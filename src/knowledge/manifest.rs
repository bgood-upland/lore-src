use std::collections::BTreeMap;
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct ProjectManifest {
    pub project: ProjectInfo,
    pub files: BTreeMap<String, KnowledgeFile>,
}

#[derive(Deserialize, Serialize)]
pub struct ProjectInfo {
    pub name: String,
}

#[derive(Deserialize, Serialize)]
pub struct KnowledgeFile {
    pub path: String,
    pub summary: String,
    pub max_size: Option<usize>,
    pub last_updated: Option<String>,
    pub when_to_read: Option<String>,
}

impl ProjectManifest {
    pub fn load(project_root: &Path) -> anyhow::Result<Self> {
        let manifest_path = project_root.join(".lore/knowledge.toml");
        let content = std::fs::read_to_string(&manifest_path)?;
        let manifest: ProjectManifest = toml::from_str(&content)?;
        Ok(manifest)
    }

    pub fn save(&self, project_root: &Path) -> anyhow::Result<()> {
        let manifest_path = project_root.join(".lore/knowledge.toml");
        let content_string = toml::to_string_pretty(self)?;
        std::fs::write(manifest_path, content_string)?;
        Ok(())
    }
}