use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use anyhow;

#[derive(Deserialize, Serialize)]
pub struct OrchestratorConfig {
    #[serde(default)]
    pub projects: Vec<ProjectEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectMode {
    #[default]
    Manual,
    Autopilot
}

fn default_mode() -> ProjectMode {
    ProjectMode::Manual
}

fn default_branch() -> Option<String> {
    Some("main".to_string())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectEntry {
    pub name: String,
    #[serde(default = "default_mode")]
    pub mode: ProjectMode,
    pub root: Option<PathBuf>,
    pub repo: Option<String>,
    #[serde(default = "default_branch")]
    pub branch: Option<String>
}

impl OrchestratorConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: OrchestratorConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()>{
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Check whether a project name is already registered
    pub fn is_registered(&self, name: &str) -> bool {
        self.projects.iter().any(|p| p.name == name)
    }

    /// Register a new project. Validate, append to project list, and save
    pub fn register_project(&mut self, entry: ProjectEntry, config_path: &Path) -> anyhow::Result<()> {
        if self.is_registered(&entry.name) {
            anyhow::bail!("Project '{}' is already registered", entry.name)
        }
        entry.validate()?;
        self.projects.push(entry);
        self.save(config_path)?;
        Ok(())
    }
}

impl ProjectEntry {

    pub fn effective_root(&self, data_dir: &Path) -> PathBuf {
        match self.mode {
            ProjectMode::Manual => self.root.clone().expect("validated"),
            ProjectMode::Autopilot => data_dir.join("repos").join(&self.name),
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        match self.mode {
            ProjectMode::Manual => {
                if let Some(root) = self.root.as_ref() {
                    if !root.exists() {
                        anyhow::bail!(
                            "Project '{}': root {} does not exist",
                            self.name, root.display()
                        );
                    }
                } else {
                    anyhow::bail!(
                        "Project '{}': root is required for Manual mode",
                        self.name
                    );
                }
                if self.repo.is_some() {
                    anyhow::bail!(
                        "Project '{}': repo is unused in Manual mode and should be removed",
                        self.name
                    );
                }
                Ok(())
            },
            ProjectMode::Autopilot => {
                if self.repo.is_none() {
                    anyhow::bail!(
                        "Project '{}': repo is required for Autopilot mode",
                        self.name
                    );
                }
                if self.root.is_some() {
                    anyhow::bail!(
                        "Project '{}': root is unused in Autopilot mode and should be removed",
                        self.name
                    );
                }
                Ok(())
            }
        }
    }
}