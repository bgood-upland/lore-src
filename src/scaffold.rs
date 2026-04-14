use std::path::Path;
use anyhow::Result;
use crate::knowledge::manifest::{ProjectManifest, ProjectInfo, KnowledgeFile};
use std::collections::BTreeMap;

pub struct ScaffoldResult {
    pub created: Vec<String>,
    pub skipped: Vec<String>,
}

impl ScaffoldResult {
    pub fn summary(&self) -> String {
        format!("Scaffolded project\n  Created: {}\n  Skipped (already exist): {}", self.created.join(", "), self.skipped.join(", "))
    }
}

const PROJECT_INSTRUCTIONS_TEMPLATE: &str = r#"# {name} — Project Instructions

## Role

## Code Standards

## Tool Usage

## Communication
"#;

const PROJECT_SCHEMA_TEMPLATE: &str = r#"# Project-specific schema extensions
# Values here are merged with (and extend) the global defaults.
# See the global schema.toml for base entity types, relation types, and tags.

# [tags]
# project = []

# [entity_types]
# allowed = []

# [relation_types]
# allowed = []

# [index_nodes]
# required = []
"#;

/// Create the `.lore/` directory structure for a project.
pub fn scaffold_project(root: &Path, project_name: &str) -> Result<ScaffoldResult> {
    let manifest_content = toml::to_string_pretty(&build_manifest(project_name))?;
    let instructions_content = PROJECT_INSTRUCTIONS_TEMPLATE.replace("{name}", project_name);
    
    let to_write: Vec<(&str, &str)> = vec![
        ("knowledge.toml", &manifest_content),
        ("docs/project-instructions.md", &instructions_content),
        ("graph/schema.toml", PROJECT_SCHEMA_TEMPLATE),
    ];

    let base_dir = root.join(".lore");
    let mut scaffold_result = ScaffoldResult {
        created: Vec::new(),
        skipped: Vec::new(),
    };

    for (relative, content) in to_write {
        let full_path = base_dir.join(relative);
        let result = write_if_new(&full_path, content)?;
        if result {
            scaffold_result.created.push(relative.to_string())
        }
        else {
            scaffold_result.skipped.push(relative.to_string())
        }
    }
    Ok(scaffold_result)

}

/// Build a starter manifest with the project name and a pre-registered entry for project-instructions.md
fn build_manifest(project_name: &str) -> ProjectManifest {
    let mut files: BTreeMap<String, KnowledgeFile> = BTreeMap::new();
    files.insert(
        "project-instructions".to_string(),
        KnowledgeFile {
            path: ".lore/docs/project-instructions.md".to_string(),
            summary: "Role, architecture decisions, conventions, and development guidelines".to_string(),
            when_to_read: Some("Read at the start of every session before any other file.".to_string()),
            max_size: None,
            last_updated: None,
        }
    );
    ProjectManifest {
        project: ProjectInfo {
            name: project_name.to_string()
        },
        files,
    }
}

/// Write content to path if it doesn't already exist
fn write_if_new(path: &Path, content: &str) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(true)
}

/// Check whether a project root already has scaffolding
pub fn is_scaffolded(root: &Path) -> bool {
    root.join(".lore/knowledge.toml").exists()
}