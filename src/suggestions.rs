/// Returns an XML block of suggested next tool calls, given the tool that just
/// executed and a context hint about the result (e.g. "no_results").
///
/// Returns an empty string when no suggestions apply — callers can skip
/// appending in that case.
pub fn get_suggested_next_xml(tool: &str, context: &str) -> String {
    let suggestions: &[(&str, &str, &str)] = match (tool, context) {
        ("search_knowledge", "no_results") => &[
            ("search_graph", "project=<project>, query=<same query>", "Check for matching graph entities"),
            ("search_knowledge", "project=<project>, query=<shorter query>", "Search again with a shorter query (1-2 words)"),
        ],
        ("search_knowledge", _) => &[
            ("read_knowledge_section", "project=<project>, file_key=<key with top match>, heading=<section heading to read>", "Targeted context from the top matching file. Prefer over read_knowledge_file"),
            ("read_knowledge_file", "project=<project>, file_key=<key with top match>", "Full context for the top matching file. Prefer when full domain knowledge is needed for the task"),
            ("search_graph", "project=<project>, query=<same query>", "Check for related graph entities"),
        ],
        ("gather_project_context", _) => &[
            ("read_knowledge_section", "project=<project>, file_key=<relevant file's key>, heading=<section heading to read>", "Targeted context from a relevant file. Prefer over read_knowledge_file"),
            ("read_knowledge_file", "project=<project>, file_key=<relevant file's key>", "Full context for a file relevant to the task"),
            ("read_entities", "project=<project>, names=[<relevant entities>]", "Full observations and relations entities relevant to the task"),
        ],
        ("read_knowledge_file", _) => &[
            ("read_knowledge_section", "project=<project>, file_key=<related file>, heading=<relevant section>", "Targeted context from a related file"),
            ("search_knowledge", "project=<project>, query=<topic>", "Search for related sections across other knowledge files"),
            ("search_graph", "project=<project>, query=<topic>", "Check for graph entities related to this file's content"),
        ],
        ("read_knowledge_section", _) => &[
            ("read_knowledge_section", "project=<project>, file_key=<related file>, heading=<relevant section>", "Targeted context from the same file, or a related file. Prefer over read_knowledge_file"),
            ("search_knowledge", "project=<project>, query=<topic>", "Search for related sections across other knowledge files"),
            ("search_graph", "project=<project>, query=<topic>", "Search for related entities in the graph"),
        ],
        ("read_entities", "no_results") => &[
            ("search_graph", "project=<project>, query=<1-2 word keyword>", "Find entities by keyword — the name you used may not match exactly"),
            ("read_entities", "project=<project>, names=[\"Index:<domain>\"]", "Read the relevant Index node to see all entities in the domain"),
            ("list_entities", "project=<project>", "List all entity names and types to find the right name"),
        ],
        ("read_entities", "has_relations") => &[
            ("read_entities", "project=<project>, names=[<entity from a relation>]", "Follow a relation to read a connected entity"),
            ("search_knowledge", "project=<project>, query=<topic>", "Find knowledge file sections related to these entities"),
        ],
        ("search_graph", "no_results") => &[
            ("search_graph", "project=<project>, query=<same query>", "Search again with a shorter query (1-2 words)"),
            ("search_knowledge", "project=<project>, query=<shorter query>", "Check for relevant sections in knowledge files"),
        ],
        ("search_graph", _) => &[
            ("read_entities", "project=<project>, names=[<matched entity>]", "Full observations for the top matching entity"),
        ],
        ("create_entities", "type_warning") => &[
            ("update_graph_schema", "project=<project>, new_entity_types=[\"<unknown type>\"]", "Add the unrecognized entity type to the project schema, then retry create_entities"),
            ("validate_graph", "project=<project>", "Run a full schema audit to see all type violations across existing entities"),
        ],
        ("create_entities", "tag_warning") => &[
            ("update_graph_schema", "project=<project>, new_tags=[\"<unknown tag>\"]", "Add the unrecognized tag to the project schema, then retry create_entities"),
            ("read_entities", "project=<project>, names=[<entity name>]", "Re-read the entity to see existing observations and their tag prefixes — an existing tag may already fit"),
        ],
        ("create_entities", "missing_tag") => &[
            ("update_graph_schema", "project=<project>", "View or add allowed tags — check the schema to see what tag prefixes are valid for this project"),
            ("read_entities", "project=<project>, names=[\"Index:<domain>\"]", "Read a relevant Index node to see how existing entities are tagged — use the same tag prefixes"),
        ],
        ("add_observations", "schema_warning") => &[
            ("update_graph_schema", "project=<project>, new_tags=[\"<unknown tag>\"]", "Add the unrecognized tag to the project schema, then retry add_observations"),
            ("read_entities", "project=<project>, names=[<entity name>]", "Re-read the entity to see existing observations and their tag prefixes — an existing tag may already fit"),
        ],
        ("add_observations", "missing_entity") => &[
            ("create_entities", "project=<project>, entities=[{name: \"<entity name>\", entityType: \"<type>\", observations: [...]}]", "Entity does not exist yet — create it first, then observations can be added"),
            ("list_entities", "project=<project>", "Verify entity names — the entity you targeted may exist under a different name"),
        ],
        ("create_relations", "schema_warning") => &[
            ("update_graph_schema", "project=<project>, new_relationship_types=[\"<unknown type>\"]", "Add the unrecognized relation type to the project schema, then retry create_relations"),
            ("validate_graph", "project=<project>", "Run a full schema audit to see all relation type violations in the graph"),
        ],

        _ => return String::new(),
    };

    let items: Vec<String> = suggestions
        .iter()
        .map(|(name, args, reason)| {
            format!("<tool name=\"{name}\" args=\"{args}\" reason=\"{reason}\"/>")
        })
        .collect();
    format!("<suggested_next>\n{}\n</suggested_next>", items.join("\n"))
}