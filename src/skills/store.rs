use std::collections::HashSet;
use std::path::{Path, PathBuf};
use anyhow::{bail, Context, Result};

use crate::resolver::SharedResolver;
use crate::markdown;
use crate::utils;
use super::types::{SkillEntry, SkillFileEntry, SkillScope, SkillSummary};
use super::frontmatter;

/// Stateless coordinator for all skill operations.
/// Same design as KnowledgeStore — no caching, reads from disk on every call.
#[derive(Clone)]
pub struct SkillStore {
    resolver: SharedResolver,
    global_skills_dir: Option<PathBuf>,
}

impl SkillStore {
    pub fn new(resolver: SharedResolver, global_skills_dir: Option<PathBuf>) -> Self {
        Self {
            resolver,
            global_skills_dir,
        }
    }

    // ─── Internal helpers ────────────────────────────────────────────

    fn resolve(&self, project: &str) -> Result<PathBuf> {
        self.resolver.read().unwrap().resolve(project)
    }

    fn resolve_writable(&self, project: &str) -> Result<PathBuf> {
        self.resolver.read().unwrap().resolve_writable(project)
    }

    fn project_skills_dir(root: &Path) -> PathBuf {
        root.join(".lore").join("skills")
    }

    /// Scans a directory for skill subdirectories containing SKILL.md.
    /// Returns entries tagged with the given scope.
    ///
    /// Each immediate subdirectory that contains a SKILL.md file becomes a skill.
    /// The subdirectory name becomes the skill_key.
    /// If the directory doesn't exist, returns an empty Vec (not an error).
    fn scan_skills_dir(&self, dir: &Path, scope: SkillScope) -> Result<Vec<SkillEntry>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for entry_result in std::fs::read_dir(dir)? {
            let entry = entry_result?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }

            let dir_name = entry.file_name().to_string_lossy().to_string();
            if dir_name.starts_with('.') {
                continue;
            }

            let full_path = entry.path().join("SKILL.md");
            if !full_path.exists() {
                continue;
            }
            let skill_content = std::fs::read_to_string(full_path)?;
            let frontmatter = frontmatter::parse_frontmatter_or_fallback(&skill_content, &dir_name);
            results.push(SkillEntry {
                skill_key: dir_name,
                metadata: frontmatter,
                scope: scope.clone(),
                dir_path: entry.path(),
            })
        }
        Ok(results)
    }

    /// Resolves a skill_key to its directory path by checking project scope first,
    /// then global scope.
    ///
    /// This is the skill equivalent of KnowledgeStore::resolve_file — it finds
    /// where the skill lives on disk.
    fn resolve_skill_dir(&self, root: &Path, skill_key: &str) -> Result<(PathBuf, SkillScope)> {
        let project_dir = Self::project_skills_dir(root).join(skill_key);
        if project_dir.join("SKILL.md").exists() {
            return Ok((project_dir, SkillScope::Project))
        }

        if let Some(ref global_dir) = self.global_skills_dir {
            let global_skill_dir = global_dir.join(skill_key);
            if global_skill_dir.join("SKILL.md").exists() {
                return Ok((global_skill_dir, SkillScope::Global))
            }
        }

        bail!("Skill not found: {}", skill_key)
    }

    // ─── Discovery ───────────────────────────────────────────────────

    /// Lists all skills available to a project (project-scoped + global).
    /// Project skills take precedence over global skills with the same key.
    pub fn list_skills(&self, project: &str) -> Result<Vec<SkillEntry>> {
        let root = self.resolve(project)?;
        let project_skills = self.scan_skills_dir(&Self::project_skills_dir(&root), SkillScope::Project)?;
        let skill_keys: HashSet<String> = project_skills
            .iter()
            .map(|s| s.skill_key.clone())
            .collect();

        let mut all_skills = project_skills;

        if let Some(ref global_dir) = self.global_skills_dir {
            let global_skills = self.scan_skills_dir(global_dir, SkillScope::Global)?;
            for skill in global_skills {
                if !skill_keys.contains(&skill.skill_key) {
                    all_skills.push(skill);
                }
            }
        }
        Ok(all_skills)
    }

    /// Returns concise summaries for gather_project_context.
    /// Maps each SkillEntry down to just the fields Claude needs for triggering.
    pub fn list_skill_summaries(&self, project: &str) -> Result<Vec<SkillSummary>> {
        let skill_list = self.list_skills(project)?
            .into_iter()
            .map(|s| SkillSummary {
                skill_key: s.skill_key,
                name: s.metadata.name,
                description: s.metadata.description,
                scope: s.scope
            })
            .collect();
        Ok(skill_list)
    }

    // ─── Reading ─────────────────────────────────────────────────────

    /// Reads a file from within a skill directory.
    /// If file_path is None, reads SKILL.md. Otherwise reads the specified file.
    pub fn read_skill_file(
        &self,
        project: &str,
        skill_key: &str,
        file_path: Option<&str>,
    ) -> Result<String> {
        let root = self.resolve(project)?;
        let (skill_dir, _scope) = self.resolve_skill_dir(&root, skill_key)?;
        let path = file_path.unwrap_or("SKILL.md");
        let target = skill_dir.join(path);
        utils::assert_path_within(&target, &skill_dir)?;

        if !target.exists() {
            bail!(
                "File '{}' not found in skill '{}'. Use list_skill_files to see available files.",
                path,
                skill_key
            );
        }

        std::fs::read_to_string(&target)
            .with_context(|| format!("Failed to read: {}", target.display()))
    }

    fn explore_directory(&self, path: &Path, skill_dir: &Path) -> Result<Vec<SkillFileEntry>> {
        let mut skill_files = Vec::new();
        for entry_result in std::fs::read_dir(path)? {
            let entry = entry_result?;
            let file_type = entry.file_type()?;

            if file_type.is_dir() {
                let mut nested_files = self.explore_directory(&entry.path(), skill_dir)?;
                skill_files.append(&mut nested_files);
                continue;
            }

            let absolute_path = entry.path();
            let relative_path = absolute_path
                .strip_prefix(skill_dir)
                .context("Failed to strip skill dir prefix")?
                .to_string_lossy()
                .into_owned();

            skill_files.push(SkillFileEntry {
                relative_path,
                size: entry.metadata()?.len(),
            });
        }
        Ok(skill_files)
    }

    /// Lists all files within a skill directory as a flat list of relative paths.
    pub fn list_skill_files(
        &self,
        project: &str,
        skill_key: &str,
    ) -> Result<Vec<SkillFileEntry>> {
        let root = self.resolve(project)?;
        let (skill_dir, _scope) = self.resolve_skill_dir(&root, skill_key)?;
        let mut skill_files = self.explore_directory(&skill_dir, &skill_dir)?;
        skill_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(skill_files)
    }

    // ─── Writing ─────────────────────────────────────────────────────

    /// Creates a new skill directory with a SKILL.md file.
    pub fn create_skill(
        &self,
        project: &str,
        skill_name: &str,
        content: &str,
        scope: SkillScope,
    ) -> Result<()> {
        let parent_dir = match scope {
            SkillScope::Project => {
                let root = self.resolve_writable(project)?;
                Self::project_skills_dir(&root)
            }
            SkillScope::Global => self.global_skills_dir.as_ref()
                .ok_or_else(|| anyhow::anyhow!("No global skills directory configured"))?
                .clone(),
        };

        let skill_dir = parent_dir.join(skill_name);
        if skill_dir.exists() {
            bail!("Skill already exists: {}", skill_name);
        }
        frontmatter::parse_frontmatter(content)
               .context("SKILL.md content must have valid YAML frontmatter")?;

        std::fs::create_dir_all(&skill_dir)?;
        std::fs::write(skill_dir.join("SKILL.md"), content)?;

        Ok(())
    }

    /// Writes or overwrites a file within an existing skill directory.
    /// Creates intermediate directories as needed (e.g., references/).
    pub fn write_skill_file(
        &self,
        project: &str,
        skill_key: &str,
        file_path: &str,
        content: &str,
    ) -> Result<()> {
        let root = self.resolve_writable(project)?;
        let (skill_dir, _scope) = self.resolve_skill_dir(&root, skill_key)?;
        let target = skill_dir.join(file_path);
        utils::assert_path_within(&target, &skill_dir)?;

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&target, content)?;
        Ok(())
    }

    pub fn update_skill_section(
        &self,
        project: &str,
        skill_key: &str,
        file_path: Option<&str>,
        heading: &str,
        content: &str,
    ) -> Result<()> {
        let root = self.resolve_writable(project)?;
        let (skill_dir, _scope) = self.resolve_skill_dir(&root, skill_key)?;
        let target = skill_dir.join(file_path.unwrap_or("SKILL.md"));
        utils::assert_path_within(&target, &skill_dir)?;
        let document = self.read_skill_file(project, skill_key, file_path)?;
        let updated_content = markdown::replace_section(&document, heading, content)?;
        std::fs::write(target, updated_content)?;
        Ok(())
    }

    /// Deletes an entire skill directory and all its contents.
    pub fn delete_skill(
        &self,
        project: &str,
        skill_key: &str,
        scope: SkillScope,
    ) -> Result<()> {
        let parent_dir = match scope {
            SkillScope::Project => {
                let root = self.resolve_writable(project)?;
                Self::project_skills_dir(&root)
            }
            SkillScope::Global => self.global_skills_dir.as_ref()
                .ok_or_else(|| anyhow::anyhow!("No global skills directory configured"))?
                .clone(),
        };

        let skill_dir = parent_dir.join(skill_key);
        if !skill_dir.exists() {
            bail!("Skill not found: {}", skill_key)
        }

        utils::assert_path_within(&skill_dir, &parent_dir)?;
        std::fs::remove_dir_all(&skill_dir)?;
        Ok(())
    }

    pub fn delete_skill_file(
        &self,
        project: &str,
        skill_key: &str,
        file_path: &str,
    ) -> Result<()> {
        let root = self.resolve_writable(project)?;
        let (skill_dir, _scope) = self.resolve_skill_dir(&root, skill_key)?;
        let target = skill_dir.join(file_path);

        utils::assert_path_within(&target, &skill_dir)?;
        if file_path == "SKILL.md" {
            bail!("Cannot delete SKILL.md directly. Use delete_skill to remove an entire skill.");
        }
        if !target.exists() {
            bail!(
                "File '{}' not found in skill '{}'. Use list_skill_files to see available files.",
                file_path, skill_key
            );
        }

        std::fs::remove_file(&target)?;
        Ok(())
    }
}