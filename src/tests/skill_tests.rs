use super::TestFixture;
use super::*;

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Minimal valid SKILL.md with correct YAML frontmatter.
const VALID_SKILL: &str = "---\nname: My Skill\ndescription: Use when doing skill-related tasks\n---\n\n# My Skill\n\n## Overview\n\nThis skill handles important tasks.\n\n## Steps\n\n1. Read context.\n2. Apply pattern.\n";

/// Skill content with an extra section used by update_skill_section tests.
const MULTI_SECTION_SKILL: &str = "---\nname: Multi Section\ndescription: Skill with multiple sections\n---\n\n# Multi Section\n\n## Overview\n\nOriginal overview content.\n\n## Steps\n\nOriginal steps content.\n";

// ─── list_skills ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_skills_returns_pre_existing_skill() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "list_skills",
        "Returns all available skills including the pre-existing test-skill",
        &f.p(),
    ).await;
    assert!(out.contains("test-skill") || out.contains("Test Skill"));
    assert!(out.contains("testing skill operations"));
}

#[tokio::test]
async fn list_skills_unknown_project() {
    let f = TestFixture::new();
    let err = f.call_err(
        "list_skills",
        "Unknown project returns error",
        r#"{"project": "nonexistent"}"#,
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("nonexistent"));
}

#[tokio::test]
async fn list_skills_shows_newly_created_skill() {
    let f = TestFixture::new();
    f.call_ok("create_skill", "Create a new skill",
        &format!(r#"{{"project": "{}", "skill_name": "fresh-skill", "content": {}, "scope": "project"}}"#,
            f.project,
            TestFixture::json_str(VALID_SKILL),
        ),
    ).await;

    let out = f.call_ok(
        "list_skills",
        "Newly created skill appears in listing",
        &f.p(),
    ).await;
    assert!(out.contains("fresh-skill") || out.contains("My Skill"));
}

// ─── read_skill ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn read_skill_reads_skill_md_by_default() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "read_skill",
        "No file_path given — reads SKILL.md by default",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill"}}"#, f.project),
    ).await;
    assert!(out.contains("Test Skill"));
    assert!(out.contains("testing skill operations"));
    assert!(out.contains("Usage"));
}

#[tokio::test]
async fn read_skill_reads_specific_file() {
    let f = TestFixture::new();
    // Write a supplementary file first
    f.call_ok("write_skill_file", "Setup: write a reference file in skill dir",
        &format!(
            r###"{{"project": "{}", "skill_key": "test-skill", "file_path": "references/guide.md", "content": "# Reference Guide\n\nThis is reference content.\n"}}"###,
            f.project
        ),
    ).await;

    let out = f.call_ok(
        "read_skill",
        "file_path provided — reads the specified file, not SKILL.md",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "references/guide.md"}}"#, f.project),
    ).await;
    assert!(out.contains("Reference Guide"), "specific file content missing");
    assert!(out.contains("reference content"));
    // Should not contain SKILL.md content
    assert!(!out.contains("testing skill operations"), "should not have read SKILL.md");
}

#[tokio::test]
async fn read_skill_unknown_key() {
    let f = TestFixture::new();
    let err = f.call_err(
        "read_skill",
        "Non-existent skill key returns error",
        &format!(r#"{{"project": "{}", "skill_key": "ghost-skill"}}"#, f.project),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("ghost-skill"));
}

#[tokio::test]
async fn read_skill_unknown_file_path() {
    let f = TestFixture::new();
    let err = f.call_err(
        "read_skill",
        "Valid skill but non-existent file_path returns error",
        &format!(
            r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "references/ghost.md"}}"#,
            f.project
        ),
    ).await;
    assert!(
        err.to_lowercase().contains("not found")
            || err.to_lowercase().contains("failed to read")
            || err.contains("ghost.md")
            || err.to_lowercase().contains("no such file"),  // ← add this
    );
}

// ─── list_skill_files ─────────────────────────────────────────────────────────

#[tokio::test]
async fn list_skill_files_includes_all_files_flat() {
    let f = TestFixture::new();
    // Add nested files
    f.call_ok("write_skill_file", "Setup: top-level extra file",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "extra.md", "content": "extra"}}"#, f.project),
    ).await;
    f.call_ok("write_skill_file", "Setup: nested file",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "examples/demo.md", "content": "demo"}}"#, f.project),
    ).await;

    let out = f.call_ok(
        "list_skill_files",
        "Lists all files including SKILL.md and nested files as flat relative paths",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill"}}"#, f.project),
    ).await;
    assert!(out.contains("SKILL.md"));
    assert!(out.contains("extra.md"));
    assert!(out.contains("examples/demo.md") || out.contains("demo.md"));
}

#[tokio::test]
async fn list_skill_files_unknown_skill() {
    let f = TestFixture::new();
    let err = f.call_err(
        "list_skill_files",
        "Unknown skill key returns error",
        &format!(r#"{{"project": "{}", "skill_key": "ghost-skill"}}"#, f.project),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("ghost-skill"));
}

// ─── create_skill ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_skill_project_scope_success() {
    let f = TestFixture::new();
    let out = f.call_ok(
        "create_skill",
        "Creates new project-scoped skill with valid frontmatter",
        &format!(r#"{{"project": "{}", "skill_name": "my-new-skill", "content": {}, "scope": "project"}}"#,
            f.project,
            TestFixture::json_str(VALID_SKILL),
        ),
    ).await;
    assert!(out.contains("my-new-skill"));

    // Should appear in listing
    let list = f.call_ok("list_skills", "New skill appears in listing", &f.p()).await;
    assert!(list.contains("my-new-skill") || list.contains("My Skill"));

    // SKILL.md should be readable
    let read = f.call_ok("read_skill", "SKILL.md content readable after creation",
        &format!(r###"{{"project": "{}", "skill_key": "my-new-skill"}}"###, f.project),
    ).await;
    assert!(read.contains("My Skill"));
}

#[tokio::test]
async fn create_skill_invalid_frontmatter_returns_error() {
    let f = TestFixture::new();
    let err = f.call_err(
        "create_skill",
        "Content without YAML frontmatter is rejected",
        &format!(r###"{{"project": "{}", "skill_name": "bad-skill", "content": "# No Frontmatter\n\nJust content.\n", "scope": "project"}}"###, f.project),
    ).await;
    assert!(
        err.to_lowercase().contains("frontmatter")
            || err.to_lowercase().contains("yaml")
            || err.to_lowercase().contains("invalid"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn create_skill_duplicate_name_returns_error() {
    let f = TestFixture::new();
    f.call_ok("create_skill", "Create initial skill",
        &format!(r#"{{"project": "{}", "skill_name": "dup-skill", "content": {}, "scope": "project"}}"#,
            f.project, TestFixture::json_str(VALID_SKILL),
        ),
    ).await;

    let err = f.call_err(
        "create_skill",
        "Creating a skill with a name that already exists returns error",
        &format!(r#"{{"project": "{}", "skill_name": "dup-skill", "content": {}, "scope": "project"}}"#,
            f.project, TestFixture::json_str(VALID_SKILL),
        ),
    ).await;
    assert!(
        err.to_lowercase().contains("already exists") || err.contains("dup-skill"),
        "unexpected error: {err}"
    );
}

// ─── write_skill_file ─────────────────────────────────────────────────────────

#[tokio::test]
async fn write_skill_file_creates_file_with_intermediate_dirs() {
    let f = TestFixture::new();
    f.call_ok(
        "write_skill_file",
        "Creates file in deep nested path — intermediate directories auto-created",
        &format!(r###"{{"project": "{}", "skill_key": "test-skill", "file_path": "deep/nested/dir/file.md", "content": "# Deep\n\nDeep content here.\n"}}"###, f.project),
    ).await;

    let read = f.call_ok("read_skill", "Deeply nested file is readable",
        &format!(r###"{{"project": "{}", "skill_key": "test-skill", "file_path": "deep/nested/dir/file.md"}}"###, f.project),
    ).await;
    assert!(read.contains("Deep content here"));
}

#[tokio::test]
async fn write_skill_file_overwrites_existing() {
    let f = TestFixture::new();
    f.call_ok("write_skill_file", "Create initial file",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "overwrite.md", "content": "original content"}}"#, f.project),
    ).await;
    f.call_ok("write_skill_file", "Overwrite with new content",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "overwrite.md", "content": "replaced content"}}"#, f.project),
    ).await;

    let out = f.call_ok("read_skill", "Verify overwrite — old content gone",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "overwrite.md"}}"#, f.project),
    ).await;
    assert!(out.contains("replaced content"));
    assert!(!out.contains("original content"), "old content survived overwrite");
}

#[tokio::test]
async fn write_skill_file_unknown_skill_returns_error() {
    let f = TestFixture::new();
    let err = f.call_err(
        "write_skill_file",
        "Writing to a skill that does not exist returns error",
        &format!(r#"{{"project": "{}", "skill_key": "ghost-skill", "file_path": "file.md", "content": "content"}}"#, f.project),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("ghost-skill"));
}

// ─── update_skill_section ────────────────────────────────────────────────────

#[tokio::test]
async fn update_skill_section_in_skill_md() {
    let f = TestFixture::new();
    f.call_ok(
        "update_skill_section",
        "Updates one section in SKILL.md, preserves other sections",
        &format!(
            r###"{{"project": "{}", "skill_key": "test-skill", "heading": "## Usage", "content": "Updated usage instructions here.\n"}}"###,
            f.project
        ),
    ).await;

    let out = f.call_ok("read_skill", "Verify targeted section updated, others preserved",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill"}}"#, f.project),
    ).await;
    assert!(out.contains("Updated usage instructions here"), "new section content missing");
    assert!(out.contains("Test Skill"), "skill name/frontmatter was lost");
    assert!(!out.contains("This skill is used for testing"), "old section content not replaced");
}

#[tokio::test]
async fn update_skill_section_in_explicit_file() {
    let f = TestFixture::new();
    // Create a file with multiple sections
    f.call_ok("write_skill_file", "Setup: multi-section file",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "guide.md", "content": {}}}"#,
            f.project,
            TestFixture::json_str(MULTI_SECTION_SKILL),
        ),
    ).await;

    f.call_ok(
        "update_skill_section",
        "Updates ## Steps in guide.md — ## Overview is untouched",
        &format!(
            r###"{{"project": "{}", "skill_key": "test-skill", "file_path": "guide.md", "heading": "## Steps", "content": "New steps content.\n"}}"###,
            f.project
        ),
    ).await;

    let out = f.call_ok("read_skill", "Verify targeted section updated, sibling preserved",
        &format!(r###"{{"project": "{}", "skill_key": "test-skill", "file_path": "guide.md"}}"###, f.project),
    ).await;
    assert!(out.contains("New steps content"), "new section content missing");
    assert!(out.contains("Original overview content"), "sibling section was modified");
    assert!(!out.contains("Original steps content"), "old section content not replaced");
}

#[tokio::test]
async fn update_skill_section_heading_not_found() {
    let f = TestFixture::new();
    let err = f.call_err(
        "update_skill_section",
        "Non-existent heading in SKILL.md returns error",
        &format!(
            r###"{{"project": "{}", "skill_key": "test-skill", "heading": "## Ghost Section", "content": "stuff\n"}}"###,
            f.project
        ),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("Ghost Section"));
}

#[tokio::test]
async fn update_skill_section_unknown_skill() {
    let f = TestFixture::new();
    let err = f.call_err(
        "update_skill_section",
        "Unknown skill key returns error",
        &format!(
            r###"{{"project": "{}", "skill_key": "ghost-skill", "heading": "## Usage", "content": "stuff\n"}}"###,
            f.project
        ),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("ghost-skill"));
}

// ─── delete_skill ────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_skill_removes_entire_directory() {
    let f = TestFixture::new();
    f.call_ok("create_skill", "Create skill to delete",
        &format!(r#"{{"project": "{}", "skill_name": "deletable-skill", "content": {}, "scope": "project"}}"#,
            f.project, TestFixture::json_str(VALID_SKILL),
        ),
    ).await;
    // Add a nested file so we verify the whole dir is removed
    f.call_ok("write_skill_file", "Add nested file to skill",
        &format!(r#"{{"project": "{}", "skill_key": "deletable-skill", "file_path": "extras/data.md", "content": "data"}}"#, f.project),
    ).await;

    let del = f.call_ok(
        "delete_skill",
        "Deletes skill dir and all contents (SKILL.md + nested files)",
        &format!(r#"{{"project": "{}", "skill_key": "deletable-skill", "scope": "project"}}"#, f.project),
    ).await;
    assert!(del.contains("deletable-skill"));

    // Should no longer appear in listing
    let list = f.call_ok("list_skills", "Deleted skill absent from listing", &f.p()).await;
    assert!(!list.contains("deletable-skill"), "skill still in listing after delete");

    // Reading it should now fail
    f.call_err("read_skill", "Reading deleted skill returns error",
        &format!(r#"{{"project": "{}", "skill_key": "deletable-skill"}}"#, f.project),
    ).await;
}

#[tokio::test]
async fn delete_skill_unknown_key_returns_error() {
    let f = TestFixture::new();
    let err = f.call_err(
        "delete_skill",
        "Deleting a skill that does not exist returns error",
        &format!(r#"{{"project": "{}", "skill_key": "ghost-skill", "scope": "project"}}"#, f.project),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("ghost-skill"));
}

// ─── delete_skill_file ───────────────────────────────────────────────────────

#[tokio::test]
async fn delete_skill_file_removes_single_file() {
    let f = TestFixture::new();
    f.call_ok("write_skill_file", "Create a file to delete",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "temp.md", "content": "temporary"}}"#, f.project),
    ).await;

    let del = f.call_ok(
        "delete_skill_file",
        "Deletes a single file — SKILL.md and other files unaffected",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "temp.md"}}"#, f.project),
    ).await;
    assert!(del.contains("temp.md"));

    // SKILL.md should still exist and be readable
    let skill = f.call_ok("read_skill", "SKILL.md still readable after sibling file deleted",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill"}}"#, f.project),
    ).await;
    assert!(skill.contains("Test Skill"), "SKILL.md was affected by delete_skill_file");

    // Deleted file should no longer be readable
    f.call_err("read_skill", "Deleted file returns error when read",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "temp.md"}}"#, f.project),
    ).await;
}

#[tokio::test]
async fn delete_skill_file_rejects_skill_md() {
    let f = TestFixture::new();
    let err = f.call_err(
        "delete_skill_file",
        "Attempting to delete SKILL.md via delete_skill_file is rejected",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "SKILL.md"}}"#, f.project),
    ).await;
    assert!(
        err.to_lowercase().contains("cannot delete")
            || err.to_lowercase().contains("skill.md")
            || err.to_lowercase().contains("use delete_skill"),
        "unexpected error message: {err}"
    );
}

#[tokio::test]
async fn delete_skill_file_unknown_file_returns_error() {
    let f = TestFixture::new();
    let err = f.call_err(
        "delete_skill_file",
        "File does not exist in skill directory — returns error",
        &format!(r#"{{"project": "{}", "skill_key": "test-skill", "file_path": "ghost.md"}}"#, f.project),
    ).await;
    assert!(err.to_lowercase().contains("not found") 
        || err.contains("ghost.md")
        || err.to_lowercase().contains("no such file")
    );
}

#[tokio::test]
async fn delete_skill_file_unknown_skill_returns_error() {
    let f = TestFixture::new();
    let err = f.call_err(
        "delete_skill_file",
        "Skill does not exist — returns not-found error",
        &format!(r#"{{"project": "{}", "skill_key": "ghost-skill", "file_path": "file.md"}}"#, f.project),
    ).await;
    assert!(err.to_lowercase().contains("not found") || err.contains("ghost-skill"));
}