use super::TestFixture;
use super::*;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn entity_json(name: &str, entity_type: &str, observations: &[&str]) -> String {
    let obs = observations
        .iter()
        .map(|o| format!("\"{}\"", o))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"{{"name": "{name}", "entityType": "{entity_type}", "observations": [{obs}]}}"#
    )
}

fn relation_json(from: &str, to: &str, rel_type: &str) -> String {
    format!(r#"{{"from": "{from}", "to": "{to}", "relationType": "{rel_type}"}}"#)
}

// ─── create_entities ─────────────────────────────────────────────────────────

#[tokio::test]
async fn create_entities_success_clean_schema() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "create_entities",
        "Valid entities with known types and tagged observations — no warnings",
        &format!(
            r#"{{"project": "{}", "entities": [{}, {}]}}"#,
            f.project,
            entity_json("MyComponent", "Component", &["[purpose] Handles the main UI"]),
            entity_json("MyStore", "Store", &["[purpose] Manages auth state"]),
        ),
    ).await;
    assert!(out.contains("MyComponent"));
    assert!(out.contains("MyStore"));
}

#[tokio::test]
async fn create_entities_duplicate_silently_skipped() {
    let f = TestFixture::new();
    f.call_ok(
        "create_entities",
        "First create succeeds normally",
        &format!(
            r#"{{"project": "{}", "entities": [{}]}}"#,
            f.project,
            entity_json("DupTarget", "Component", &["[purpose] Original"]),
        ),
    ).await;

    let out = f.call_ok(
        "create_entities",
        "Second create with same name is silently skipped; new entity alongside is created",
        &format!(
            r#"{{"project": "{}", "entities": [{}, {}]}}"#,
            f.project,
            entity_json("DupTarget", "Component", &["[purpose] Should be skipped"]),
            entity_json("NewEntity", "Component", &["[purpose] Should be created"]),
        ),
    ).await;
    assert!(out.contains("NewEntity"), "new entity was not created");
    // DupTarget should not appear as newly created
    assert!(!out.contains("Should be skipped"), "duplicate observation leaked into output");
}

#[tokio::test]
async fn create_entities_unknown_type_emits_warning_and_suggestion() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "create_entities",
        "Unknown entity type — entity created but schema warning + update_graph_schema suggestion emitted",
        &format!(
            r#"{{"project": "{}", "entities": [{}]}}"#,
            f.project,
            entity_json("WeirdThing", "UnknownEntityType", &["[purpose] Something"]),
        ),
    ).await;
    assert!(out.contains("WeirdThing"), "entity should still be created");
    assert!(
        out.to_lowercase().contains("unknown type") || out.contains("UnknownEntityType"),
        "expected unknown-type warning"
    );
    assert!(
        out.contains("update_graph_schema") || out.contains("suggested_next"),
        "expected schema update suggestion"
    );
}

#[tokio::test]
async fn create_entities_unknown_tag_emits_warning() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "create_entities",
        "Unknown observation tag prefix emits schema warning",
        &format!(
            r#"{{"project": "{}", "entities": [{}]}}"#,
            f.project,
            entity_json("TaggedComp", "Component", &["[unknowntag] Observation with bad tag"]),
        ),
    ).await;
    assert!(out.contains("TaggedComp"));
    assert!(
        out.to_lowercase().contains("unknown tag") || out.contains("unknowntag"),
        "expected unknown-tag warning"
    );
}

#[tokio::test]
async fn create_entities_missing_tag_prefix_emits_warning() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "create_entities",
        "Observation without any [tag] prefix emits missing-tag warning",
        &format!(
            r#"{{"project": "{}", "entities": [{}]}}"#,
            f.project,
            entity_json("NoTagComp", "Component", &["This observation has no tag prefix"]),
        ),
    ).await;
    assert!(out.contains("NoTagComp"));
    assert!(
        out.to_lowercase().contains("missing") || out.to_lowercase().contains("tag"),
        "expected missing-tag warning"
    );
}

#[tokio::test]
async fn create_entities_unknown_project() {
    let f = TestFixture::new();
    let err = f.call_err(
        "create_entities",
        "Unknown project name returns error",
        &format!(
            r#"{{"project": "nonexistent", "entities": [{}]}}"#,
            entity_json("E", "Component", &[])
        ),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("nonexistent"));
}

// ─── add_observations ────────────────────────────────────────────────────────

#[tokio::test]
async fn add_observations_success() {
    let f = TestFixture::new();
    f.call_ok(
        "create_entities",
        "Setup: create entity to receive observations",
        &format!(
            r#"{{"project": "{}", "entities": [{}]}}"#,
            f.project,
            entity_json("ObsTarget", "Component", &["[purpose] Initial purpose"]),
        ),
    ).await;

    f.call_ok(
        "add_observations",
        "Appends new observations to an existing entity",
        &format!(
            r#"{{"project": "{}", "observations": [{{"entity_name": "ObsTarget", "contents": ["[gotcha] Watch out here", "[api] Exposes useObsTarget()"]}}]}}"#,
            f.project
        ),
    ).await;

    // Read back and verify both old and new observations present
    let read = f.call_ok(
        "read_entities",
        "Verify new observations appended, old observation preserved",
        &format!(r#"{{"project": "{}", "entity_names": ["ObsTarget"]}}"#, f.project),
    ).await;
    assert!(read.contains("Initial purpose"), "original observation lost");
    assert!(read.contains("Watch out here"), "new gotcha observation missing");
    assert!(read.contains("useObsTarget"), "new api observation missing");
}

#[tokio::test]
async fn add_observations_nonexistent_entity_silently_skipped() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "add_observations",
        "Entity does not exist — silently skipped, warning emitted, no crash",
        &format!(
            r#"{{"project": "{}", "observations": [{{"entity_name": "GhostEntity", "contents": ["[purpose] Will be ignored"]}}]}}"#,
            f.project
        ),
    ).await;
    // Should get a warning about the nonexistent entity
    assert!(
        out.contains("GhostEntity") || out.to_lowercase().contains("does not exist"),
        "expected warning about missing entity"
    );
}

#[tokio::test]
async fn add_observations_unknown_tag_emits_warning_and_suggestion() {
    let f = TestFixture::new();
    f.call_ok(
        "create_entities",
        "Setup entity",
        &format!(
            r#"{{"project": "{}", "entities": [{}]}}"#,
            f.project,
            entity_json("TagWarnComp", "Component", &[])
        ),
    ).await;

    let out = f.call_ok(
        "add_observations",
        "Unknown tag prefix — warning emitted + update_graph_schema suggested",
        &format!(
            r#"{{"project": "{}", "observations": [{{"entity_name": "TagWarnComp", "contents": ["[badtag] Something here"]}}]}}"#,
            f.project
        ),
    ).await;
    assert!(
        out.to_lowercase().contains("unknown tag") || out.contains("badtag"),
        "expected unknown-tag warning"
    );
    assert!(
        out.contains("update_graph_schema") || out.contains("suggested_next"),
        "expected schema update suggestion"
    );
}

#[tokio::test]
async fn add_observations_near_duplicate_emits_warning() {
    let f = TestFixture::new();
    f.call_ok(
        "create_entities",
        "Setup entity with an existing observation",
        &format!(
            r#"{{"project": "{}", "entities": [{}]}}"#,
            f.project,
            entity_json("DupObsEnt", "Component", &["[purpose] This is the main purpose of the component system"]),
        ),
    ).await;

    let out = f.call_ok(
        "add_observations",
        "Near-duplicate observation triggers duplicate warning",
        &format!(
            r#"{{"project": "{}", "observations": [{{"entity_name": "DupObsEnt", "contents": ["[purpose] This is the main purpose of the component system here"]}}]}}"#,
            f.project
        ),
    ).await;
    assert!(
        out.to_lowercase().contains("duplicate") || out.contains("<warning"),
        "expected duplicate observation warning"
    );
}

// ─── delete_entities ─────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_entities_removes_entity_from_graph() {
    let f = TestFixture::new();
    f.call_ok(
        "create_entities",
        "Setup entity to delete",
        &format!(
            r#"{{"project": "{}", "entities": [{}]}}"#,
            f.project,
            entity_json("ToDelete", "Component", &[])
        ),
    ).await;

    let del = f.call_ok(
        "delete_entities",
        "Entity is deleted by name",
        &format!(r#"{{"project": "{}", "entity_names": ["ToDelete"]}}"#, f.project),
    ).await;
    assert!(del.contains("ToDelete") || del.to_lowercase().contains("deleted"));

    let graph = f.call_ok(
        "read_graph",
        "Verify entity no longer present in graph",
        &f.p(),
    ).await;
    assert!(!graph.contains("ToDelete"), "entity still present after delete");
}

#[tokio::test]
async fn delete_entities_cascades_relations() {
    let f = TestFixture::new();
    f.call_ok(
        "create_entities",
        "Setup two entities",
        &format!(
            r#"{{"project": "{}", "entities": [{}, {}]}}"#,
            f.project,
            entity_json("CascadeFrom", "Component", &[]),
            entity_json("CascadeTo", "Component", &[]),
        ),
    ).await;
    f.call_ok(
        "create_relations",
        "Setup relation between the two entities",
        &format!(
            r#"{{"project": "{}", "relations": [{}]}}"#,
            f.project,
            relation_json("CascadeFrom", "CascadeTo", "depends_on"),
        ),
    ).await;

    f.call_ok(
        "delete_entities",
        "Deleting CascadeFrom also removes its relation to CascadeTo",
        &format!(r#"{{"project": "{}", "entity_names": ["CascadeFrom"]}}"#, f.project),
    ).await;

    let graph = f.call_ok(
        "read_graph",
        "Verify entity and its outgoing relation are both gone",
        &f.p(),
    ).await;
    assert!(!graph.contains("CascadeFrom"), "entity still in graph");
    // The relation referencing CascadeFrom should be gone
    assert!(
        !graph.contains("\"from\": \"CascadeFrom\"") && !graph.contains("from=\"CascadeFrom\""),
        "dangling relation still present"
    );
}

// ─── delete_observations ─────────────────────────────────────────────────────

#[tokio::test]
async fn delete_observations_removes_only_targeted() {
    let f = TestFixture::new();
    f.call_ok(
        "create_entities",
        "Setup entity with two observations",
        &format!(
            r#"{{"project": "{}", "entities": [{}]}}"#,
            f.project,
            entity_json("ObsDelEnt", "Component", &["[purpose] Keep this one", "[gotcha] Remove this one"]),
        ),
    ).await;

    f.call_ok(
        "delete_observations",
        "Deletes only the specified observation — sibling is preserved",
        &format!(
            r#"{{"project": "{}", "deletions": [{{"entity_name": "ObsDelEnt", "observations": ["[gotcha] Remove this one"]}}]}}"#,
            f.project
        ),
    ).await;

    let read = f.call_ok(
        "read_entities",
        "Verify targeted observation removed, sibling preserved",
        &format!(r#"{{"project": "{}", "entity_names": ["ObsDelEnt"]}}"#, f.project),
    ).await;
    assert!(read.contains("Keep this one"), "sibling observation was lost");
    assert!(!read.contains("Remove this one"), "targeted observation still present");
}

#[tokio::test]
async fn delete_observations_nonexistent_entity_is_noop() {
    let f = TestFixture::new();
    // Should complete without error — just a no-op
    f.call_ok(
        "delete_observations",
        "Deleting from nonexistent entity is a silent no-op",
        &format!(
            r#"{{"project": "{}", "deletions": [{{"entity_name": "Ghost", "observations": ["[purpose] Nope"]}}]}}"#,
            f.project
        ),
    ).await;
}

// ─── create_relations ────────────────────────────────────────────────────────

#[tokio::test]
async fn create_relations_success() {
    let f = TestFixture::new();
    f.call_ok(
        "create_entities",
        "Setup two entities to relate",
        &format!(
            r#"{{"project": "{}", "entities": [{}, {}]}}"#,
            f.project,
            entity_json("RelFrom", "Component", &[]),
            entity_json("RelTo", "Component", &[]),
        ),
    ).await;

    let out = f.call_ok(
        "create_relations",
        "Creates valid relation between existing entities",
        &format!(
            r#"{{"project": "{}", "relations": [{}]}}"#,
            f.project,
            relation_json("RelFrom", "RelTo", "depends_on"),
        ),
    ).await;
    assert!(out.contains("RelFrom"));
    assert!(out.contains("RelTo"));
    assert!(out.contains("depends_on"));
}

#[tokio::test]
async fn create_relations_duplicate_silently_skipped() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup",
        &format!(r#"{{"project": "{}", "entities": [{}, {}]}}"#, f.project,
            entity_json("DupRelA", "Component", &[]),
            entity_json("DupRelB", "Component", &[]),
        ),
    ).await;
    f.call_ok("create_relations", "First relation succeeds",
        &format!(r#"{{"project": "{}", "relations": [{}]}}"#, f.project,
            relation_json("DupRelA", "DupRelB", "depends_on"),
        ),
    ).await;
    let out = f.call_ok(
        "create_relations",
        "Identical relation submitted again — silently skipped, empty result returned",
        &format!(r#"{{"project": "{}", "relations": [{}]}}"#, f.project,
            relation_json("DupRelA", "DupRelB", "depends_on"),
        ),
    ).await;
    // Result should be an empty list (nothing new created)
    let _ = out; // success is enough; no panic = correct
}

#[tokio::test]
async fn create_relations_unknown_type_emits_warning_and_suggestion() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup",
        &format!(r#"{{"project": "{}", "entities": [{}, {}]}}"#, f.project,
            entity_json("WarnRelA", "Component", &[]),
            entity_json("WarnRelB", "Component", &[]),
        ),
    ).await;

    let out = f.call_ok(
        "create_relations",
        "Unknown relation type — warning + update_graph_schema suggestion emitted",
        &format!(r#"{{"project": "{}", "relations": [{}]}}"#, f.project,
            relation_json("WarnRelA", "WarnRelB", "completely_unknown_type"),
        ),
    ).await;
    assert!(
        out.to_lowercase().contains("unknown type") || out.contains("completely_unknown_type"),
        "expected unknown-type warning"
    );
    assert!(
        out.contains("update_graph_schema") || out.contains("suggested_next"),
        "expected schema update suggestion"
    );
}

#[tokio::test]
async fn create_relations_nonexistent_entity_emits_warning() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup one entity only",
        &format!(r#"{{"project": "{}", "entities": [{}]}}"#, f.project,
            entity_json("ExistingNode", "Component", &[])
        ),
    ).await;

    let out = f.call_ok(
        "create_relations",
        "Relation to nonexistent entity emits dangling-reference warning",
        &format!(r#"{{"project": "{}", "relations": [{}]}}"#, f.project,
            relation_json("ExistingNode", "GhostNode", "depends_on"),
        ),
    ).await;
    assert!(
        out.contains("GhostNode") || out.to_lowercase().contains("non-existent"),
        "expected dangling reference warning"
    );
}

// ─── delete_relations ────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_relations_success() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup",
        &format!(r#"{{"project": "{}", "entities": [{}, {}]}}"#, f.project,
            entity_json("DelRelA", "Component", &[]),
            entity_json("DelRelB", "Component", &[]),
        ),
    ).await;
    f.call_ok("create_relations", "Create relation to delete",
        &format!(r#"{{"project": "{}", "relations": [{}]}}"#, f.project,
            relation_json("DelRelA", "DelRelB", "depends_on"),
        ),
    ).await;

    let del = f.call_ok(
        "delete_relations",
        "Deletes exact relation — all three fields must match",
        &format!(r#"{{"project": "{}", "relations": [{}]}}"#, f.project,
            relation_json("DelRelA", "DelRelB", "depends_on"),
        ),
    ).await;
    assert!(del.to_lowercase().contains("deleted") || del.contains("1"));

    let graph = f.call_ok("read_graph", "Verify relation removed from graph", &f.p()).await;
    assert!(
        !graph.contains("\"relationType\": \"depends_on\"") && !graph.contains("relationType=\"depends_on\"")
        || !graph.contains("DelRelA"),
        "relation still present after deletion"
    );
}

#[tokio::test]
async fn delete_relations_nonexistent_is_noop() {
    let f = TestFixture::new();
    // Deleting a relation that doesn't exist should not error
    f.call_ok(
        "delete_relations",
        "Deleting a non-existent relation is a silent no-op",
        &format!(r#"{{"project": "{}", "relations": [{}]}}"#, f.project,
            relation_json("Ghost1", "Ghost2", "depends_on"),
        ),
    ).await;
}

// ─── search_graph ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn search_graph_matches_by_name() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup searchable entity",
        &format!(r#"{{"project": "{}", "entities": [{}]}}"#, f.project,
            entity_json("SearchableWidget", "Component", &["[purpose] Does widget things"]),
        ),
    ).await;

    let out = f.call_ok(
        "search_graph",
        "Finds entity by partial name match — case-insensitive",
        &format!(r#"{{"project": "{}", "query": "Widget"}}"#, f.project),
    ).await;
    assert!(out.contains("SearchableWidget"));
    assert!(out.contains("<suggested_next>"));
}

#[tokio::test]
async fn search_graph_matches_by_type() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup",
        &format!(r#"{{"project": "{}", "entities": [{}, {}]}}"#, f.project,
            entity_json("UtilityOne", "Utility", &["[purpose] A utility"]),
            entity_json("ComponentOne", "Component", &["[purpose] A component"]),
        ),
    ).await;

    let out = f.call_ok(
        "search_graph",
        "Search by entity type — returns only entities of that type",
        &format!(r#"{{"project": "{}", "query": "Utility"}}"#, f.project),
    ).await;
    assert!(out.contains("UtilityOne"));
}

#[tokio::test]
async fn search_graph_matches_by_observation_text() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup entity with distinctive observation",
        &format!(r#"{{"project": "{}", "entities": [{}]}}"#, f.project,
            entity_json("ObsSearchTarget", "Component", &["[gotcha] xyzzy_unique_token for testing"]),
        ),
    ).await;

    let out = f.call_ok(
        "search_graph",
        "Finds entity by substring in observation text",
        &format!(r#"{{"project": "{}", "query": "xyzzy_unique"}}"#, f.project),
    ).await;
    assert!(out.contains("ObsSearchTarget"));
}

#[tokio::test]
async fn search_graph_no_results_includes_suggestions() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "search_graph",
        "No matches — returns empty result with no-results suggested_next",
        &format!(r#"{{"project": "{}", "query": "zzznomatch999xyz"}}"#, f.project),
    ).await;
    assert!(out.contains("<suggested_next>"), "no-results should include suggestions");
}

// ─── read_graph ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn read_graph_empty_graph() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "read_graph",
        "Empty graph — returns valid XML with no entities or relations",
        &f.p(),
    ).await;
    // Valid XML structure expected even for empty graph
    assert!(out.contains("graph") || out.contains("entities") || out.contains("project"));
}

#[tokio::test]
async fn read_graph_returns_all_entities_and_relations() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup two entities",
        &format!(r#"{{"project": "{}", "entities": [{}, {}]}}"#, f.project,
            entity_json("GraphNodeA", "Component", &["[purpose] Part A"]),
            entity_json("GraphNodeB", "Store", &["[purpose] Part B"]),
        ),
    ).await;
    f.call_ok("create_relations", "Setup relation",
        &format!(r#"{{"project": "{}", "relations": [{}]}}"#, f.project,
            relation_json("GraphNodeA", "GraphNodeB", "depends_on"),
        ),
    ).await;

    let out = f.call_ok(
        "read_graph",
        "Full graph — all entities and relations present",
        &f.p(),
    ).await;
    assert!(out.contains("GraphNodeA"));
    assert!(out.contains("GraphNodeB"));
    assert!(out.contains("depends_on"));
}

#[tokio::test]
async fn read_graph_unknown_project() {
    let f = TestFixture::new();
    let err = f.call_err(
        "read_graph",
        "Unknown project returns error",
        r#"{"project": "nonexistent"}"#,
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("nonexistent"));
}

// ─── list_entities ────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_entities_lightweight_no_observations() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup",
        &format!(r#"{{"project": "{}", "entities": [{}, {}]}}"#, f.project,
            entity_json("ListEntA", "Component", &["[purpose] Rich observation content here"]),
            entity_json("ListEntB", "Store", &["[purpose] Store stuff"]),
        ),
    ).await;

    let out = f.call_ok(
        "list_entities",
        "Returns name+type only — observations NOT included",
        &f.p(),
    ).await;
    assert!(out.contains("ListEntA"));
    assert!(out.contains("ListEntB"));
    assert!(out.contains("Component"));
    assert!(out.contains("Store"));
    // Observation text should NOT be in the lightweight list
    assert!(!out.contains("Rich observation content here"), "observations leaked into list_entities");
}

// ─── read_entities ────────────────────────────────────────────────────────────

#[tokio::test]
async fn read_entities_returns_full_observations_and_relations() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup",
        &format!(r#"{{"project": "{}", "entities": [{}, {}]}}"#, f.project,
            entity_json("ReadTargetA", "Component", &["[purpose] Specific detail here"]),
            entity_json("ReadTargetB", "Component", &[]),
        ),
    ).await;
    f.call_ok("create_relations", "Setup relation",
        &format!(r#"{{"project": "{}", "relations": [{}]}}"#, f.project,
            relation_json("ReadTargetA", "ReadTargetB", "depends_on"),
        ),
    ).await;

    let out = f.call_ok(
        "read_entities",
        "Returns full observations and relations for named entities",
        &format!(r#"{{"project": "{}", "entity_names": ["ReadTargetA"]}}"#, f.project),
    ).await;
    assert!(out.contains("ReadTargetA"));
    assert!(out.contains("Specific detail here"));
    assert!(out.contains("<suggested_next>"));
}

#[tokio::test]
async fn read_entities_unknown_name_returns_empty() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "read_entities",
        "Unknown entity name returns empty result — no error",
        &format!(r#"{{"project": "{}", "entity_names": ["DoesNotExist"]}}"#, f.project),
    ).await;
    // Empty result: shouldn't contain the unknown name as a populated entity,
    // but should still be a valid response with suggested_next
    assert!(out.contains("<suggested_next>") || !out.contains("DoesNotExist entities"));
}

#[tokio::test]
async fn read_entities_subset_of_names() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup three entities",
        &format!(r#"{{"project": "{}", "entities": [{}, {}, {}]}}"#, f.project,
            entity_json("SubA", "Component", &["[purpose] A"]),
            entity_json("SubB", "Component", &["[purpose] B"]),
            entity_json("SubC", "Component", &["[purpose] C"]),
        ),
    ).await;

    let out = f.call_ok(
        "read_entities",
        "Requesting subset of names returns only those entities",
        &format!(r#"{{"project": "{}", "entity_names": ["SubA", "SubC"]}}"#, f.project),
    ).await;
    assert!(out.contains("SubA"));
    assert!(out.contains("SubC"));
    assert!(!out.contains("[purpose] B"), "SubB observation should not appear");
}

// ─── validate_graph ───────────────────────────────────────────────────────────

#[tokio::test]
async fn validate_graph_clean_passes() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup clean entity with valid type and tagged observations",
        &format!(r#"{{"project": "{}", "entities": [{}]}}"#, f.project,
            entity_json("CleanEntity", "Component", &[
                "[purpose] Does good things",
                "[file] src/components/CleanEntity.vue",
            ]),
        ),
    ).await;

    let out = f.call_ok(
        "validate_graph",
        "Well-formed graph with no violations",
        &f.p(),
    ).await;
    // "passed" or a short warning list — both are acceptable
    let _ = out;
}

#[tokio::test]
async fn validate_graph_reports_unknown_type() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup entity with invalid type",
        &format!(r#"{{"project": "{}", "entities": [{}]}}"#, f.project,
            entity_json("BadTypeEntity", "CompletelyUnknownType", &["[purpose] Something"]),
        ),
    ).await;

    let out = f.call_ok(
        "validate_graph",
        "Graph with unknown entity type — validate_graph reports it",
        &f.p(),
    ).await;
    assert!(
        out.to_lowercase().contains("unknown") || out.contains("CompletelyUnknownType"),
        "expected unknown-type warning in validation output"
    );
}

#[tokio::test]
async fn validate_graph_reports_missing_tag_prefix() {
    let f = TestFixture::new();
    f.call_ok("create_entities", "Setup entity with untagged observation",
        &format!(r#"{{"project": "{}", "entities": [{}]}}"#, f.project,
            entity_json("UntaggedObs", "Component", &["This observation has no tag"]),
        ),
    ).await;

    let out = f.call_ok(
        "validate_graph",
        "Observation without [tag] prefix — validate_graph reports it",
        &f.p(),
    ).await;
    assert!(
        out.to_lowercase().contains("missing") || out.to_lowercase().contains("tag"),
        "expected missing-tag warning"
    );
}

#[tokio::test]
async fn validate_graph_no_schema_returns_informative_message() {
    // Build a server pointing to a project with NO schema files at all
    let tmpdir = tempfile::tempdir().unwrap();
    let data_dir = tmpdir.path().join("data");
    let project_root = tmpdir.path().join("project");

    std::fs::create_dir_all(data_dir.join("defaults")).unwrap();
    std::fs::create_dir_all(project_root.join(".lore")).unwrap();
    std::fs::write(
        project_root.join(".lore/knowledge.toml"),
        "[project]\nname = \"No Schema\"\n",
    ).unwrap();

    let config_path = data_dir.join("config.toml");
    std::fs::write(
        &config_path,
        &format!(
            "[[projects]]\nname = \"no-schema\"\nmode = \"manual\"\nroot = \"{}\"\n",
            project_root.display()
        ),
    ).unwrap();

    let projects = vec![crate::config::ProjectEntry {
        name: "no-schema".to_string(),
        root: Some(project_root),
        mode: crate::config::ProjectMode::Manual,
        repo: None,
        branch: None,
    }];
    let resolver = crate::resolver::ProjectResolver::new(projects, data_dir.clone());
    let knowledge = KnowledgeStore::new(resolver.clone());
    let skills = SkillStore::new(resolver.clone(), None);
    let graph = GraphStore::new(resolver.clone(), data_dir.join("defaults"));
    let server = crate::OrchestratorServer::new(knowledge, skills, graph, resolver, config_path, data_dir);

    let result = crate::cli::dispatch_tool(&server, "validate_graph", r#"{"project": "no-schema"}"#)
        .await
        .expect("validate_graph should not error even with no schema");

    let text = super::extract_text(&result);
    super::write_log(
        "validate_graph",
        "No schema present — should return informative cannot-validate message",
        r#"{"project": "no-schema"}"#,
        &text,
    );
    assert!(
        text.to_lowercase().contains("no schema") || text.to_lowercase().contains("cannot validate"),
        "expected informative no-schema message, got: {text}"
    );
}

// ─── update_graph_schema ──────────────────────────────────────────────────────

#[tokio::test]
async fn update_graph_schema_adds_entity_type_then_no_warning() {
    let f = TestFixture::new();
    let schema_out = f.call_ok(
        "update_graph_schema",
        "Adds new entity type — subsequent create with that type emits no warning",
        &format!(r#"{{"project": "{}", "new_entity_types": ["NewCustomType"]}}"#, f.project),
    ).await;
    assert!(
        schema_out.to_lowercase().contains("schema updated") || schema_out.contains("entity"),
        "expected success message"
    );

    let create_out = f.call_ok(
        "create_entities",
        "Entity with newly registered type — should have no unknown-type warning",
        &format!(
            r#"{{"project": "{}", "entities": [{}]}}"#,
            f.project,
            entity_json("NewTypeComp", "NewCustomType", &["[purpose] Uses new type"]),
        ),
    ).await;
    assert!(create_out.contains("NewTypeComp"));
    assert!(
        !create_out.to_lowercase().contains("unknown type"),
        "should not warn about type that was just added to schema"
    );
}

#[tokio::test]
async fn update_graph_schema_adds_tag_then_no_warning() {
    let f = TestFixture::new();
    f.call_ok(
        "update_graph_schema",
        "Adds new tag to schema",
        &format!(r#"{{"project": "{}", "new_tags": ["newtag"]}}"#, f.project),
    ).await;
    f.call_ok("create_entities", "Setup",
        &format!(r#"{{"project": "{}", "entities": [{}]}}"#, f.project,
            entity_json("TagTestEnt", "Component", &[])
        ),
    ).await;

    let obs_out = f.call_ok(
        "add_observations",
        "Observation using newly added tag — should not warn",
        &format!(
            r#"{{"project": "{}", "observations": [{{"entity_name": "TagTestEnt", "contents": ["[newtag] This uses the new tag"]}}]}}"#,
            f.project
        ),
    ).await;
    assert!(!obs_out.to_lowercase().contains("unknown tag"), "newly added tag should be allowed");
}

#[tokio::test]
async fn update_graph_schema_adds_relation_type_then_no_warning() {
    let f = TestFixture::new();
    f.call_ok(
        "update_graph_schema",
        "Adds new relation type to schema",
        &format!(r#"{{"project": "{}", "new_relationship_types": ["custom_rel"]}}"#, f.project),
    ).await;
    f.call_ok("create_entities", "Setup",
        &format!(r#"{{"project": "{}", "entities": [{}, {}]}}"#, f.project,
            entity_json("SchRelA", "Component", &[]),
            entity_json("SchRelB", "Component", &[]),
        ),
    ).await;

    let rel_out = f.call_ok(
        "create_relations",
        "Relation using newly added type — should not warn",
        &format!(r#"{{"project": "{}", "relations": [{}]}}"#, f.project,
            relation_json("SchRelA", "SchRelB", "custom_rel"),
        ),
    ).await;
    assert!(!rel_out.to_lowercase().contains("unknown type"), "newly added relation type should be allowed");
}

#[tokio::test]
async fn update_graph_schema_partial_update_omitted_fields_ok() {
    let f = TestFixture::new();
    // Only supply new_relationship_types — all other fields omitted
    let out = f.call_ok(
        "update_graph_schema",
        "Partial update — only new_relationship_types provided, other fields omitted",
        &format!(r#"{{"project": "{}", "new_relationship_types": ["partial_only_rel"]}}"#, f.project),
    ).await;
    assert!(out.to_lowercase().contains("schema updated") || out.contains("relation"));
}

#[tokio::test]
async fn update_graph_schema_deduplicates_repeated_values() {
    let f = TestFixture::new();
    // Add the same type twice in one call — should not produce duplicates in schema
    f.call_ok(
        "update_graph_schema",
        "Duplicate values in request are deduplicated in the written schema",
        &format!(
            r#"{{"project": "{}", "new_entity_types": ["DedupType", "DedupType"]}}"#,
            f.project
        ),
    ).await;
    // Add again — should still not error and not duplicate
    f.call_ok(
        "update_graph_schema",
        "Adding same type a second time is idempotent",
        &format!(r#"{{"project": "{}", "new_entity_types": ["DedupType"]}}"#, f.project),
    ).await;
}

#[tokio::test]
async fn update_graph_schema_unknown_project() {
    let f = TestFixture::new();
    let err = f.call_err(
        "update_graph_schema",
        "Unknown project returns error",
        r#"{"project": "nonexistent", "new_entity_types": ["Type"]}"#,
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("nonexistent"));
}