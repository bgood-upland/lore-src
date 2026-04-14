use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use anyhow::{bail, Result};
use crate::config::{ProjectEntry, ProjectMode};

/// Shared project resolution for all stores.
/// Handles name→root lookup and enforces write restrictions on Autopilot projects.
pub struct ProjectResolver {
    projects: Vec<ProjectEntry>,
    data_dir: PathBuf,
}

/// A cheaply-cloneable handle. All stores and the server hold one of these.
pub type SharedResolver = Arc<RwLock<ProjectResolver>>;

impl ProjectResolver {
    pub fn new(projects: Vec<ProjectEntry>, data_dir: PathBuf) -> SharedResolver {
        Arc::new(RwLock::new(Self { projects, data_dir }))
    }

    /// Resolve a project name to its effective root
    pub fn resolve(&self, name: &str) -> Result<PathBuf> {
        let entry = self.find(name)?;
        Ok(entry.effective_root(&self.data_dir))
    }

    /// Resolve a project name to its effective root, rejecting Autopilot projects
    pub fn resolve_writable(&self, name: &str) -> Result<PathBuf> {
        let entry = self.find(name)?;
        if entry.mode == ProjectMode::Autopilot {
            bail!(
                "Project '{}' is read-only (Autopilot mode). \
                 Changes must be committed to the upstream repository.",
                name
            );
        }
        Ok(entry.effective_root(&self.data_dir))
    }

    /// Look up a project entry by name. Used internally and by callers
    pub fn find(&self, name: &str) -> Result<&ProjectEntry> {
        self.projects.iter()
            .find(|p| p.name == name)
            .ok_or_else(|| anyhow::anyhow!("Unknown project: {}", name))
    }

    /// Register a new project at runtime
    pub fn register(&mut self, entry: ProjectEntry) {
        self.projects.push(entry);
    }

    /// Return all projects. Used by list_all_projects.
    pub fn list(&self) -> &[ProjectEntry] {
        &self.projects
    }

    /// Return the data directory. Used by background sync.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}