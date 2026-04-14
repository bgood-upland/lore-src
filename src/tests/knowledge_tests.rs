use super::TestFixture;
use super::*;

// ─── list_all_projects ────────────────────────────────────────────────────────

#[tokio::test]
async fn list_all_projects_returns_registered_projects() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "list_all_projects",
        "Returns all registered projects with names and root paths",
        "{}",
    ).await;
    assert!(out.contains("test-project"), "project name missing from output");
}

// ─── gather_project_context ───────────────────────────────────────────────────

#[tokio::test]
async fn gather_project_context_without_overview() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "gather_project_context",
        "include_overview: false — returns file index, skills, graph summary; no instructions body",
        &format!(r#"{{"project": "{}", "include_overview": false}}"#, f.project),
    ).await;
    assert!(out.contains("<knowledge_files>"));
    assert!(out.contains("project-instructions"));
    assert!(out.contains("architecture"));
    assert!(out.contains("<skills>"));
    assert!(out.contains("<graph_summary>"));
    assert!(out.contains("<suggested_next>"));
    // Full instructions body should NOT be included
    assert!(!out.contains("test project overview"));
}

#[tokio::test]
async fn gather_project_context_with_overview() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "gather_project_context",
        "include_overview: true — includes full project-instructions body",
        &format!(r#"{{"project": "{}", "include_overview": true}}"#, f.project),
    ).await;
    assert!(out.contains("<project_instructions>"));
    assert!(out.contains("test project overview"));
}

#[tokio::test]
async fn gather_project_context_when_to_read_attribute_present() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "gather_project_context",
        "when_to_read attribute appears on files that have it set in manifest",
        &format!(r#"{{"project": "{}", "include_overview": false}}"#, f.project),
    ).await;
    assert!(out.contains("when_to_read="), "when_to_read attribute missing");
}

#[tokio::test]
async fn gather_project_context_unknown_project() {
    let f = TestFixture::new();
    let err = f.call_err(
        "gather_project_context",
        "Unknown project name returns an error",
        r#"{"project": "nonexistent-project", "include_overview": false}"#,
    ).await;
    assert!(
        err.to_lowercase().contains("not found") || err.contains("nonexistent-project"),
        "unexpected error text: {err}"
    );
}

// ─── list_knowledge_files ─────────────────────────────────────────────────────

#[tokio::test]
async fn list_knowledge_files_returns_manifest_entries() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "list_knowledge_files",
        "Returns all registered files with keys, paths, summaries",
        &f.p(),
    ).await;
    assert!(out.contains("project-instructions"));
    assert!(out.contains("architecture"));
}

#[tokio::test]
async fn list_knowledge_files_unknown_project() {
    let f = TestFixture::new();
    let err = f.call_err(
        "list_knowledge_files",
        "Unknown project returns error",
        r#"{"project": "unknown"}"#,
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("unknown"));
}

// ─── read_knowledge_file ──────────────────────────────────────────────────────

#[tokio::test]
async fn read_knowledge_file_returns_full_content() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "read_knowledge_file",
        "Reads full markdown file content by manifest key",
        &format!(r#"{{"project": "{}", "file_key": "project-instructions"}}"#, f.project),
    ).await;
    assert!(out.contains("Project Instructions"));
    assert!(out.contains("test project overview"));
    assert!(out.contains("snake_case"));
    assert!(out.contains("<suggested_next>"));
}

#[tokio::test]
async fn read_knowledge_file_unknown_key() {
    let f = TestFixture::new();
    let err = f.call_err(
        "read_knowledge_file",
        "Unregistered file key returns error",
        &format!(r#"{{"project": "{}", "file_key": "ghost-key"}}"#, f.project),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("ghost-key"));
}

#[tokio::test]
async fn read_knowledge_file_unknown_project() {
    let f = TestFixture::new();
    let err = f.call_err(
        "read_knowledge_file",
        "Unknown project returns error",
        r#"{"project": "unknown", "file_key": "project-instructions"}"#,
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("unknown"));
}

// ─── read_knowledge_section ───────────────────────────────────────────────────

#[tokio::test]
async fn read_knowledge_section_returns_only_that_section() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "read_knowledge_section",
        "Returns only the targeted section, not adjacent sections",
        &format!(
            r###"{{"project": "{}", "file_key": "project-instructions", "heading": "## Conventions"}}"###,
            f.project
        ),
    ).await;
    assert!(out.contains("snake_case"), "section content missing");
    // Sibling section content should NOT be present
    assert!(!out.contains("test project overview"), "adjacent section leaked into result");
    assert!(out.contains("<suggested_next>"));
}

#[tokio::test]
async fn read_knowledge_section_heading_not_found() {
    let f = TestFixture::new();
    let err = f.call_err(
        "read_knowledge_section",
        "Non-existent heading returns error",
        &format!(
            r###"{{"project": "{}", "file_key": "project-instructions", "heading": "## Ghost Section"}}"###,
            f.project
        ),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("Ghost Section"));
}

#[tokio::test]
async fn read_knowledge_section_missing_hash_prefix_fails() {
    let f = TestFixture::new();
    // "Conventions" without ## does not match "## Conventions"
    let err = f.call_err(
        "read_knowledge_section",
        "Heading without # prefix does not match and returns error",
        &format!(
            r#"{{"project": "{}", "file_key": "project-instructions", "heading": "Conventions"}}"#,
            f.project
        ),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("Conventions"));
}

// ─── write_knowledge_file ─────────────────────────────────────────────────────

#[tokio::test]
async fn write_knowledge_file_replaces_entire_content() {
    let f = TestFixture::new();
    let new_content = "# Project Instructions\n\n## Overview\n\nCompletely new content.\n\n## Conventions\n\nNew conventions here.\n";
    f.call_ok(
        "write_knowledge_file",
        "Overwrites entire file — old content gone, new content written",
        &format!(
            r#"{{"project": "{}", "file_key": "project-instructions", "content": {}}}"#,
            f.project,
            TestFixture::json_str(new_content)
        ),
    ).await;

    // Read back and verify
    let read_out = f.call_ok(
        "read_knowledge_file",
        "Verify write persisted — old content gone, new content present",
        &format!(r#"{{"project": "{}", "file_key": "project-instructions"}}"#, f.project),
    ).await;
    assert!(read_out.contains("Completely new content"));
    assert!(!read_out.contains("test project overview"), "old content survived overwrite");
}

#[tokio::test]
async fn write_knowledge_file_unknown_key() {
    let f = TestFixture::new();
    let err = f.call_err(
        "write_knowledge_file",
        "Writing to an unregistered key returns error",
        &format!(
            r#"{{"project": "{}", "file_key": "not-registered", "content": "content"}}"#,
            f.project
        ),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("not-registered"));
}

// ─── update_knowledge_section ─────────────────────────────────────────────────

#[tokio::test]
async fn update_knowledge_section_replaces_section_preserves_others() {
    let f = TestFixture::new();
    f.call_ok(
        "update_knowledge_section",
        "Replaces targeted section; adjacent sections are untouched",
        &format!(
            r###"{{"project": "{}", "file_key": "project-instructions", "heading": "## Conventions", "content": "Use camelCase everywhere.\n"}}"###,
            f.project
        ),
    ).await;

    let full = f.call_ok(
        "read_knowledge_file",
        "Verify section replaced and siblings preserved",
        &format!(r#"{{"project": "{}", "file_key": "project-instructions"}}"#, f.project),
    ).await;
    assert!(full.contains("camelCase"), "new section content missing");
    assert!(!full.contains("snake_case"), "old section content not replaced");
    assert!(full.contains("test project overview"), "sibling Overview section was lost");
}

#[tokio::test]
async fn update_knowledge_section_heading_not_found() {
    let f = TestFixture::new();
    let err = f.call_err(
        "update_knowledge_section",
        "Non-existent heading returns error, file unchanged",
        &format!(
            r###"{{"project": "{}", "file_key": "project-instructions", "heading": "## Does Not Exist", "content": "nope\n"}}"###,
            f.project
        ),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("Does Not Exist"));
}

#[tokio::test]
async fn update_knowledge_section_unknown_key() {
    let f = TestFixture::new();
    let err = f.call_err(
        "update_knowledge_section",
        "Unknown file key returns error",
        &format!(
            r###"{{"project": "{}", "file_key": "ghost", "heading": "## Overview", "content": "nope\n"}}"###,
            f.project
        ),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("ghost"));
}

// ─── search_knowledge ─────────────────────────────────────────────────────────

#[tokio::test]
async fn search_knowledge_returns_match_with_section_context() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "search_knowledge",
        "Query matches file — returns file key and section heading",
        &format!(r#"{{"project": "{}", "query": "snake_case"}}"#, f.project),
    ).await;
    assert!(out.contains("project-instructions"));
    assert!(out.contains("snake_case"));
    assert!(out.contains("<suggested_next>"));
}

#[tokio::test]
async fn search_knowledge_is_case_insensitive() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "search_knowledge",
        "Search is case-insensitive — uppercase query matches lowercase text",
        &format!(r#"{{"project": "{}", "query": "SNAKE_CASE"}}"#, f.project),
    ).await;
    assert!(out.contains("project-instructions"));
}

#[tokio::test]
async fn search_knowledge_multi_file_results() {
    let f = TestFixture::new();
    // "the" appears in both files
    let out = f.call_ok(
        "search_knowledge",
        "Query matching multiple files returns results from all files",
        &format!(r#"{{"project": "{}", "query": "the system"}}"#, f.project),
    ).await;
    // architecture.md contains "The system has several components"
    assert!(out.contains("architecture"));
}

#[tokio::test]
async fn search_knowledge_no_results_includes_suggestions() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "search_knowledge",
        "No matches — returns no-results message with suggested_next for retrying",
        &format!(r#"{{"project": "{}", "query": "zzznomatch999xyz"}}"#, f.project),
    ).await;
    assert!(out.to_lowercase().contains("no matching") || out.contains("zzznomatch999xyz"));
    assert!(out.contains("<suggested_next>"), "no-results path should include suggestions");
}

#[tokio::test]
async fn search_knowledge_unknown_project() {
    let f = TestFixture::new();
    let err = f.call_err(
        "search_knowledge",
        "Unknown project returns error",
        r#"{"project": "unknown", "query": "anything"}"#,
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("unknown"));
}

// ─── register_knowledge_file / unregister_knowledge_file ─────────────────────

#[tokio::test]
async fn register_then_unregister_lifecycle() {
    let f = TestFixture::new();

    // Register
    let reg = f.call_ok(
        "register_knowledge_file",
        "Adds new file entry to the manifest",
        &format!(
            r#"{{"project": "{}", "key": "new-doc", "path": ".lore/docs/new-doc.md", "summary": "A brand new document"}}"#,
            f.project
        ),
    ).await;
    assert!(reg.contains("new-doc"));

    // Should now appear in listing
    let list1 = f.call_ok(
        "list_knowledge_files",
        "Newly registered file appears in manifest listing",
        &f.p(),
    ).await;
    assert!(list1.contains("new-doc"), "registered file missing from listing");

    // Unregister
    let unreg = f.call_ok(
        "unregister_knowledge_file",
        "Removes file entry from manifest without deleting the file on disk",
        &format!(r#"{{"project": "{}", "file_key": "new-doc"}}"#, f.project),
    ).await;
    assert!(unreg.contains("new-doc"));

    // Should no longer appear
    let list2 = f.call_ok(
        "list_knowledge_files",
        "Unregistered file absent from manifest listing",
        &f.p(),
    ).await;
    assert!(!list2.contains("new-doc"), "unregistered file still in listing");
}

#[tokio::test]
async fn register_duplicate_key_returns_error() {
    let f = TestFixture::new();
    let err = f.call_err(
        "register_knowledge_file",
        "Registering a key that already exists returns error",
        &format!(
            r#"{{"project": "{}", "key": "project-instructions", "path": ".lore/docs/other.md", "summary": "dup"}}"#,
            f.project
        ),
    ).await;
    assert!(
        err.to_lowercase().contains("already exists") || err.contains("project-instructions"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn register_absolute_path_rejected() {
    let f = TestFixture::new();
    let err = f.call_err(
        "register_knowledge_file",
        "Absolute path is rejected — paths must be relative to project root",
        &format!(
            r#"{{"project": "{}", "key": "bad-path", "path": "/etc/passwd", "summary": "bad"}}"#,
            f.project
        ),
    ).await;
    assert!(
        err.to_lowercase().contains("relative") || err.contains("/etc/passwd"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn unregister_unknown_key_returns_error() {
    let f = TestFixture::new();
    let err = f.call_err(
        "unregister_knowledge_file",
        "Unregistering a key that does not exist returns error",
        &format!(r#"{{"project": "{}", "file_key": "ghost-key"}}"#, f.project),
    ).await;
    assert!(
        err.to_lowercase().contains("not found")
            || err.to_lowercase().contains("does not exist")
            || err.contains("ghost-key"),
        "unexpected error: {err}"
    );
}