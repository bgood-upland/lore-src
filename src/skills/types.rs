use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// Human-readable skill name from frontmatter.
    /// Falls back to the directory name if parsing fails.
    pub name: String,

    /// Trigger description from frontmatter.
    /// Falls back to "No description available" if parsing fails.
    pub description: String,
}

/// Where a skill was discovered — affects precedence and path resolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SkillScope {
    /// Found in `<project_root>/.lore/skills/<key>/`
    Project,
    /// Found in the global skills directory from config.toml
    Global,
}

impl From<Option<String>> for SkillScope {
    fn from(s: Option<String>) -> Self {
        match s.as_deref() {
            Some("global") => SkillScope::Global,
            _ => SkillScope::Project
        }
    }
}

impl std::fmt::Display for SkillScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillScope::Global => write!(f, "global"),
            SkillScope::Project => write!(f, "project"),
        }
    }
}

/// A fully resolved skill entry ready for tool responses.
/// Combines parsed metadata with location info.
#[derive(Debug, Clone, Serialize)]
pub struct SkillEntry {
    /// Directory name — used as the identifier in tool calls (e.g., "dynamic-ui")
    pub skill_key: String,

    /// Parsed frontmatter metadata
    pub metadata: SkillMetadata,

    /// Whether this skill is project-scoped or global
    pub scope: SkillScope,

    /// Absolute path to the skill directory on disk.
    /// NOT exposed to tool callers — used internally for file operations.
    pub dir_path: std::path::PathBuf,
}

/// A single file entry within a skill's directory tree.
/// Returned by `list_skill_files`.
#[derive(Debug, Clone, Serialize)]
pub struct SkillFileEntry {
    /// Relative path from the skill directory root (e.g., "references/new-component.md")
    pub relative_path: String,

    /// File size in bytes
    pub size: u64,
}

/// The concise skill summary included in `gather_project_context` responses.
/// This is the "level 1" metadata that lets Claude decide whether to load the full skill.
#[derive(Debug, Clone, Serialize)]
pub struct SkillSummary {
    pub skill_key: String,
    pub name: String,
    pub description: String,
    pub scope: SkillScope,
}