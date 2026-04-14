use std::path::{Path, PathBuf};
use rayon::prelude::*;
use anyhow::{bail, Result};
use serde::Serialize;

use crate::resolver::SharedResolver;
use super::manifest::{ProjectManifest, KnowledgeFile};
use crate::markdown;
use crate::utils;

#[derive(Clone)]
pub struct KnowledgeStore {
    resolver: SharedResolver,
}

#[derive(Serialize)]
pub struct ProjectSummary {
    pub name: String,
    pub root: String,
}

#[derive(Serialize)]
pub struct FileSummary {
    pub key: String,
    pub path: String,
    pub summary: String,
    pub last_updated: Option<String>,
    pub when_to_read: Option<String>,
}

/// A single line match within a knowledge file search.
/// Carries structured data — XML formatting happens in the tool layer.
#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub line: String,
    pub section_heading: Option<String>,
}

/// All matches found within a single knowledge file.
#[derive(Debug, Serialize)]
pub struct FileSearchResult {
    pub file_key: String,
    pub matches: Vec<SearchMatch>,
}


impl KnowledgeStore {
    pub fn new(resolver: SharedResolver) -> Self {
        Self { resolver }
    }

    fn resolve(&self, project: &str) -> Result<PathBuf> {
        self.resolver.read().unwrap().resolve(project)
    }

    fn resolve_writable(&self, project: &str) -> Result<PathBuf> {
        self.resolver.read().unwrap().resolve_writable(project)
    }

    fn resolve_file(&self, project_root: &Path, project_name: &str, file_key: &str) -> Result<(PathBuf, PathBuf)> {
        let manifest = ProjectManifest::load(project_root)?;
        let entry = manifest.files
            .get(file_key)
            .ok_or_else(|| anyhow::anyhow!("Key '{}' not found. Use list_knowledge_files(project=\"{}\") to see registered keys.", file_key, project_name))?;

        let file_path = project_root.join(&entry.path);
        utils::assert_path_within(&file_path, project_root)?;

        let parent = file_path.parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid file path: no parent directory"))?;
        std::fs::create_dir_all(parent)?;

        let canonical_root = project_root.canonicalize()?;
        let canonical_file = parent.canonicalize()?.join(
            file_path.file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid file path: no file name"))?
        );

        Ok((canonical_root, canonical_file))
    }

    pub fn list_files(&self, project: &str) -> Result<Vec<FileSummary>> {
        let project_root = self.resolve(project)?;
        let manifest = ProjectManifest::load(&project_root)?;
        Ok(manifest.files
            .into_iter()
            .map(|(key, file)| FileSummary {
                key,
                path: file.path,
                summary: file.summary,
                last_updated: file.last_updated,
                when_to_read: file.when_to_read,
            })
            .collect())
    }

    pub fn read_file(&self, project: &str, file_key: &str) -> Result<String> {
        let project_root = self.resolve(project)?;
        let (_root, file_path) = self.resolve_file(&project_root, project, file_key)?;
        let content = std::fs::read_to_string(file_path)?;
        Ok(content)
    }

    fn post_write_update(&self, project: &str, file_key: &str, written_size: usize) -> Result<Option<String>> {
        let root = self.resolve(project)?;
        let mut manifest = ProjectManifest::load(&root)?;
        let mut max_size = None;
        if let Some(file) = manifest.files.get_mut(file_key) {
            file.last_updated = Some(chrono::Local::now().format("%Y-%m-%d").to_string());
            max_size = file.max_size;
        }
        manifest.save(&root)?;

        if let Some(max_size) = max_size {
            if written_size > max_size {
                return Ok(Some(format!(
                    "File '{}' is now {} characters (budget: {}). Consider splitting large sections into separate files or moving procedural material into skills.",
                    file_key, written_size, max_size
                )));
            }
        }
        Ok(None)
    }

    pub fn write_file(&self, project: &str, file_key: &str, content: &str) -> Result<Option<String>> {
        let project_root = self.resolve_writable(project)?;
        let (_root, file_path) = self.resolve_file(&project_root, project, file_key)?;
        std::fs::write(file_path, content)?;
        self.post_write_update(project, file_key, content.len())
    }

    pub fn update_section(
        &self,
        project: &str,
        file_key: &str,
        heading: &str,
        content: &str,
    ) -> Result<Option<String>> {
        let project_root = self.resolve_writable(project)?;
        let (_root, file_path) = self.resolve_file(&project_root, project, file_key)?;
        let document = std::fs::read_to_string(&file_path)?;
        let updated_document = markdown::replace_section(&document, heading, content)?;
        let written_size = updated_document.len();
        std::fs::write(&file_path, updated_document)?;
        self.post_write_update(project, file_key, written_size)
    }

    pub fn register_file(
        &self,
        project: &str,
        key: &str,
        path: &str,
        summary: &str,
    ) -> Result<()> {
        let root = self.resolve_writable(project)?;
        let mut manifest = ProjectManifest::load(&root)?;
        
        if manifest.files.contains_key(key) {
            bail!("Key already exists: {}", key);
        }
        if path.starts_with("/") {
            bail!("Path must be relative to project root, got: {}", path);
        }

        manifest.files.insert(key.to_string(), KnowledgeFile {
            path: path.to_string(),
            summary: summary.to_string(),
            max_size: None,
            last_updated: None,
            when_to_read: None,
        });

        manifest.save(&root)?;
        Ok(())
    }

    pub fn unregister_file(&self, project: &str, file_key: &str) -> Result<()> {
        let root = self.resolve_writable(project)?;
        let mut manifest = ProjectManifest::load(&root)?;

        if !manifest.files.contains_key(file_key) {
            bail!("Key does not exist: {}", file_key);
        }
        manifest.files.remove(file_key);
        manifest.save(&root)?;

        Ok(())
    }

    pub fn search_files(&self, project: &str, query: &str) -> Result<Vec<FileSearchResult>> {
        let root = self.resolve(project)?;
        let files = self.list_files(project)?;
        let query_lower = query.to_lowercase();
        let file_search_results: Vec<FileSearchResult> = files
            .par_iter()
            .filter_map(|file| {
                let mut file_search = FileSearchResult {
                    file_key: file.key.clone(),
                    matches: Vec::new()
                };
                let Ok(content) = std::fs::read_to_string(root.join(&file.path)) else {
                    return None;
                };
                let lines: Vec<&str> = content.lines().collect();
                for (index, line) in lines.iter().enumerate() {
                    if line.to_lowercase().contains(&query_lower) {
                        let section_heading = lines[..index]
                            .iter()
                            .rev()
                            .find(|l| l.starts_with("#"))
                            .map(|l| l.to_string());
                        file_search.matches.push(SearchMatch {
                            line: line.to_string(),
                            section_heading,
                        });
                    }
                }
                if file_search.matches.is_empty() {
                    None
                } else {
                    Some(file_search)
                }
            }) 
            .collect();
        Ok(file_search_results)
    }

    pub fn read_section(&self, project: &str, file_key: &str, heading: &str) -> Result<String> {
        let root = self.resolve(project)?;
        let manifest = ProjectManifest::load(&root)?;
        if let Some(file) = manifest.files.get(file_key) {
            let content = std::fs::read_to_string(root.join(&file.path))?;
            return Ok(markdown::extract_section(&content, heading)?);
        }
        bail!("File not found for key: {}", file_key)
    }

    pub fn list_file_headings(&self, project: &str, file_key: &str) -> Result<Vec<String>> {
        let root = self.resolve(project)?;
        let manifest = ProjectManifest::load(&root)?;
        let file = manifest.files.get(file_key)
            .ok_or_else(|| anyhow::anyhow!("File not found for key: {}", file_key))?;
        let content = std::fs::read_to_string(root.join(&file.path))?;
        Ok(markdown::extract_section_headings(&content))
    }

    pub fn list_file_headings_with_size(&self, project: &str, file_key: &str) -> Result<Vec<(String, usize)>> {
        let root = self.resolve(project)?;
        let manifest = ProjectManifest::load(&root)?;
        let file = manifest.files.get(file_key)
            .ok_or_else(|| anyhow::anyhow!("File not found for key: {}", file_key))?;
        let content = std::fs::read_to_string(root.join(&file.path))?;
        Ok(markdown::section_sizes(&content))
    }
}