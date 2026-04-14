/// Integration tests for all MCP tools.
///
/// Each test gets its own isolated TestFixture (a real temp directory with
/// a live OrchestratorServer). Tests call tools via cli::dispatch_tool,
/// which is the same code path used by the CLI test subcommand.
///
/// Every call is logged to `test-output.log` in the project root so you
/// can inspect exactly what an agent would see for each situation.
///
/// To run:   cargo test
/// To view:  cat test-output.log

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use tempfile::TempDir;

use crate::config::ProjectEntry;
use crate::graph::store::GraphStore;
use crate::knowledge::store::KnowledgeStore;
use crate::resolver::ProjectResolver;
use crate::skills::store::SkillStore;
use crate::OrchestratorServer;

pub mod knowledge_tests;
pub mod graph_tests;
pub mod skill_tests;

// ─── Log writer ──────────────────────────────────────────────────────────────

static LOG_MUTEX: Mutex<()> = Mutex::new(());

pub fn log_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-output.log")
}

pub fn write_log(tool: &str, situation: &str, params: &str, output: &str) {
    let _guard = LOG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
        .expect("Failed to open test-output.log");

    writeln!(file, "{}", "=".repeat(80)).unwrap();
    writeln!(file, "TOOL:      {tool}").unwrap();
    writeln!(file, "SITUATION: {situation}").unwrap();
    writeln!(file, "PARAMS:    {params}").unwrap();
    writeln!(file, "{}", "-".repeat(80)).unwrap();
    writeln!(file, "OUTPUT:").unwrap();
    writeln!(file, "{output}").unwrap();
    writeln!(file, "{}", "=".repeat(80)).unwrap();
    writeln!(file).unwrap();
}

// ─── Content extraction ───────────────────────────────────────────────────────

/// Pull the text string out of a successful CallToolResult.
pub fn extract_text(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .map(|c| {
            if let Ok(v) = serde_json::to_value(c) {
                if let Some(text) = v.get("text").and_then(|t| t.as_str()) {
                    return text.to_string();
                }
            }
            format!("{c:?}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ─── TestFixture ─────────────────────────────────────────────────────────────

/// A fully isolated test environment. Creates a real temp directory with:
///   data/
///     config.toml
///     skills/          ← global skills dir (empty)
///     defaults/
///       schema.toml    ← global graph schema with universal tags + base types
///   project/
///     .lore/
///       knowledge.toml ← two pre-registered files
///       docs/
///         project-instructions.md
///         architecture.md
///       graph/
///         schema.toml  ← project-level overrides (extra types, tags)
///       skills/
///         test-skill/
///           SKILL.md   ← a pre-existing skill for read/list/update tests
///
/// Dropped (and temp dir deleted) at the end of each test.
pub struct TestFixture {
    pub _tmpdir: TempDir,
    pub server: OrchestratorServer,
    pub project: String,
}

impl TestFixture {
    pub fn new() -> Self {
        let tmpdir = tempfile::tempdir().expect("Failed to create tempdir");
        let root = tmpdir.path();

        let data_dir = root.join("data");
        let project_root = root.join("project");

        // ── Directory scaffold ──────────────────────────────────────
        for dir in [
            data_dir.join("skills"),
            data_dir.join("defaults"),
            project_root.join(".lore/docs"),
            project_root.join(".lore/graph"),
            project_root.join(".lore/skills/test-skill"),
        ] {
            fs::create_dir_all(&dir).unwrap();
        }

        // ── Global schema defaults ──────────────────────────────────
        fs::write(
            data_dir.join("defaults/schema.toml"),
            r#"
[tags]
universal = ["file", "purpose", "api", "gotcha", "decision", "convention"]

[entity_types]
allowed = ["Component", "Utility", "Store", "Index"]

[relation_types]
allowed = ["creates", "renders", "depends_on"]
"#,
        )
        .unwrap();

        // ── Project-level schema ────────────────────────────────────
        fs::write(
            project_root.join(".lore/graph/schema.toml"),
            r#"
[tags]
project = ["auth", "routing"]

[entity_types]
allowed = ["Page", "Plugin"]

[relation_types]
allowed = ["navigates_to", "registers_with"]

[index_nodes]
required = ["Index:Core"]
"#,
        )
        .unwrap();

        // ── Knowledge manifest ──────────────────────────────────────
        fs::write(
            project_root.join(".lore/knowledge.toml"),
            r#"
[project]
name = "Test Project"

[files.project-instructions]
path = ".lore/docs/project-instructions.md"
summary = "Core project instructions and conventions"
when_to_read = "Read at the start of every session"

[files.architecture]
path = ".lore/docs/architecture.md"
summary = "System architecture overview"
when_to_read = "When working on system design"
"#,
        )
        .unwrap();

        // ── Knowledge files ─────────────────────────────────────────
        fs::write(
            project_root.join(".lore/docs/project-instructions.md"),
            "# Project Instructions\n\n## Overview\n\nThis is the test project overview.\n\n## Conventions\n\nUse snake_case for variables.\n",
        )
        .unwrap();

        fs::write(
            project_root.join(".lore/docs/architecture.md"),
            "# Architecture\n\n## Components\n\nThe system has several components.\n\n## Data Flow\n\nData flows from top to bottom.\n",
        )
        .unwrap();

        // ── Pre-existing skill ──────────────────────────────────────
        fs::write(
            project_root.join(".lore/skills/test-skill/SKILL.md"),
            "---\nname: Test Skill\ndescription: Use when testing skill operations\n---\n\n# Test Skill\n\n## Usage\n\nThis skill is used for testing.\n",
        )
        .unwrap();

        // ── Config file ─────────────────────────────────────────────
        let config_path = data_dir.join("config.toml");
        fs::write(
            &config_path,
            &format!(
                "[[projects]]\nname = \"test-project\"\nmode = \"manual\"\nroot = \"{}\"\n",
                project_root.display()
            ),
        )
        .unwrap();

        // ── Build server ────────────────────────────────────────────
        let project_name = "test-project".to_string();
        let projects = vec![ProjectEntry {
            name: project_name.clone(),
            root: Some(project_root.clone()),
            mode: crate::config::ProjectMode::Manual,
            repo: None,
            branch: None,
        }];

        let resolver = ProjectResolver::new(projects, data_dir.clone());
        let knowledge = KnowledgeStore::new(resolver.clone());
        let skills = SkillStore::new(resolver.clone(), Some(data_dir.join("skills")));
        let graph = GraphStore::new(resolver.clone(), data_dir.join("defaults"));
        let server = OrchestratorServer::new(knowledge, skills, graph, resolver, config_path, data_dir);

        Self {
            _tmpdir: tmpdir,
            server,
            project: project_name,
        }
    }

    // ── Call helpers ─────────────────────────────────────────────────

    /// Raw dispatch: returns Ok(text) or Err(message).
    pub async fn call(&self, tool: &str, params: &str) -> Result<String, String> {
        match crate::cli::dispatch_tool(&self.server, tool, params).await {
            Ok(result) => Ok(extract_text(&result)),
            Err(e) => Err(e.message.to_string()),
        }
    }

    /// Expects success. Panics with the error if it fails. Logs the output.
    pub async fn call_ok(&self, tool: &str, situation: &str, params: &str) -> String {
        let output = self.call(tool, params).await.unwrap_or_else(|e| {
            panic!("Tool '{tool}' failed unexpectedly\n  situation: {situation}\n  params: {params}\n  error: {e}")
        });
        write_log(tool, situation, params, &output);
        output
    }

    /// Expects failure. Panics if it succeeds. Logs the error output.
    pub async fn call_err(&self, tool: &str, situation: &str, params: &str) -> String {
        let err = self.call(tool, params).await.err().unwrap_or_else(|| {
            panic!("Tool '{tool}' should have failed but succeeded\n  situation: {situation}\n  params: {params}")
        });
        write_log(tool, situation, params, &format!("[ERROR] {err}"));
        err
    }

    /// Convenience: `{"project": "<project>"}` JSON.
    pub fn p(&self) -> String {
        format!(r#"{{"project": "{}"}}"#, self.project)
    }

    /// Convenience: serialize a value and embed in a larger JSON string.
    pub fn json_str(s: &str) -> String {
        serde_json::to_string(s).unwrap()
    }
}