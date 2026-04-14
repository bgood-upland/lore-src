use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
struct PartialSchema {
    pub tags: Option<PartialTagConfig>,
    pub entity_types: Option<PartialEntityTypeConfig>,
    pub relation_types: Option<PartialRelationTypeConfig>,
    pub index_nodes: Option<PartialIndexNodeConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PartialTagConfig {
    pub universal: Option<Vec<String>>,
    pub project: Option<Vec<String>>,
    pub require_prefix: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PartialEntityTypeConfig {
    pub allowed: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PartialRelationTypeConfig {
    pub allowed: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PartialIndexNodeConfig {
    pub required: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct TagConfig {
    pub universal: Vec<String>,
    pub project: Vec<String>,
    pub require_prefix: bool,
}

#[derive(Debug, Clone)]
pub struct EntityTypeConfig {
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RelationTypeConfig {
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IndexNodeConfig {
    pub required: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GraphSchema {
    pub tags: TagConfig,
    pub entity_types: EntityTypeConfig,
    pub relation_types: RelationTypeConfig,
    pub index_nodes: IndexNodeConfig,
}

impl GraphSchema {
    /// Load the merged schema: global defaults from defaults_dir + project overrides.
    /// Returns None only if neither file exists.
    pub fn load(defaults_dir: &Path, project_root: &Path) -> Result<Option<Self>> {
        let global_path = defaults_dir.join("schema.toml");
        let project_path = project_root.join(".lore/graph/schema.toml");

        let global = Self::load_partial(&global_path)?;
        let project = Self::load_partial(&project_path)?;

        match (global, project) {
            (None, None) => Ok(None),
            (Some(g), None) => Ok(Some(Self::from_partial(g))),
            (None, Some(p)) => Ok(Some(Self::from_partial(p))),
            (Some(g), Some(p)) => Ok(Some(Self::merge(g, p))),
        }
    }

    fn load_partial(path: &Path) -> Result<Option<PartialSchema>> {
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path)?;
        let partial: PartialSchema = toml::from_str(&content)?;
        Ok(Some(partial))
    }

    fn from_partial(partial: PartialSchema) -> Self {
        let tags = partial.tags.unwrap_or(PartialTagConfig {
            universal: None,
            project: None,
            require_prefix: None,
        });
        Self {
            tags: TagConfig {
                universal: tags.universal.unwrap_or_default(),
                project: tags.project.unwrap_or_default(),
                require_prefix: tags.require_prefix.unwrap_or(true),
            },
            entity_types: EntityTypeConfig {
                allowed: partial.entity_types.and_then(|e| e.allowed).unwrap_or_default(),
            },
            relation_types: RelationTypeConfig {
                allowed: partial.relation_types.and_then(|r| r.allowed).unwrap_or_default(),
            },
            index_nodes: IndexNodeConfig {
                required: partial.index_nodes.and_then(|i| i.required).unwrap_or_default(),
            },
        }
    }

    pub fn empty() -> Self {
        Self {
            tags: TagConfig { universal: vec![], project: vec![], require_prefix: true },
            entity_types: EntityTypeConfig { allowed: vec![] },
            relation_types: RelationTypeConfig { allowed: vec![] },
            index_nodes: IndexNodeConfig { required: vec![] },
        }
    }

    fn merge(global: PartialSchema, project: PartialSchema) -> Self {
        let mut base = Self::from_partial(global);

        if let Some(tags) = project.tags {
            if let Some(universal) = tags.universal {
                base.tags.universal.extend(universal);
            }
            if let Some(project_tags) = tags.project {
                base.tags.project.extend(project_tags);
            }
            if let Some(require_prefix) = tags.require_prefix {
                base.tags.require_prefix = require_prefix;
            }
        }

        if let Some(entity_types) = project.entity_types {
            if let Some(allowed) = entity_types.allowed {
                base.entity_types.allowed.extend(allowed);
            }
        }

        if let Some(relation_types) = project.relation_types {
            if let Some(allowed) = relation_types.allowed {
                base.relation_types.allowed.extend(allowed);
            }
        }

        // Index nodes: project replaces global entirely
        if let Some(index_nodes) = project.index_nodes {
            if let Some(required) = index_nodes.required {
                base.index_nodes.required = required;
            }
        }

        Self::dedup(&mut base.tags.universal);
        Self::dedup(&mut base.tags.project);
        Self::dedup(&mut base.entity_types.allowed);
        Self::dedup(&mut base.relation_types.allowed);
        Self::dedup(&mut base.index_nodes.required);

        base
    }

    fn dedup(vec: &mut Vec<String>) {
        let mut seen = HashSet::new();
        vec.retain(|item| seen.insert(item.clone()));
    }

    pub fn all_allowed_tags(&self) -> HashSet<&str> {
        let mut tags: HashSet<&str> = self.tags.project.iter().map(|t| t.as_str()).collect();
        tags.extend(self.tags.universal.iter().map(|t| t.as_str()));
        tags
    }

    pub fn update_project_schema(&mut self, project_root: &Path, new_tags: &[String], new_entity_types: &[String], new_relationship_types: &[String], new_index_nodes: &[String]) -> Result<String> {
        let project_path = project_root.join(".lore/graph/schema.toml");
        
        let mut project = Self::load_partial(&project_path)?.unwrap_or(PartialSchema {
            tags: None,
            entity_types: None,
            relation_types: None,
            index_nodes: None,
        });

        let tags_before = project.tags.as_ref()
            .and_then(|t| t.project.as_ref())
            .map(|v| v.len())
            .unwrap_or(0);
        let entity_types_before = project.entity_types.as_ref()
            .and_then(|e| e.allowed.as_ref())
            .map(|v| v.len())
            .unwrap_or(0);
        let relation_types_before = project.relation_types.as_ref()
            .and_then(|r| r.allowed.as_ref())
            .map(|v| v.len())
            .unwrap_or(0);
        let index_nodes_before = project.index_nodes.as_ref()
            .and_then(|i| i.required.as_ref())
            .map(|v| v.len())
            .unwrap_or(0);

        if !new_tags.is_empty() {
            project.tags
                .get_or_insert_with(|| PartialTagConfig { 
                    universal: None, 
                    project: None, 
                    require_prefix: None 
                })
                .project
                .get_or_insert_with(Vec::new)
                .extend(new_tags.iter().cloned());
        }

        if !new_entity_types.is_empty() {
            project
                .entity_types
                .get_or_insert_with(|| PartialEntityTypeConfig { allowed: None })
                .allowed
                .get_or_insert_with(Vec::new)
                .extend(new_entity_types.iter().cloned());
        }

        if !new_relationship_types.is_empty() {
            project
                .relation_types
                .get_or_insert_with(|| PartialRelationTypeConfig { allowed: None })
                .allowed
                .get_or_insert_with(Vec::new)
                .extend(new_relationship_types.iter().cloned());
        }

        if !new_index_nodes.is_empty() {
            project
                .index_nodes
                .get_or_insert_with(|| PartialIndexNodeConfig { required: None })
                .required
                .get_or_insert_with(Vec::new)
                .extend(new_index_nodes.iter().cloned());
        }

        if let Some(ref mut tags) = project.tags {
            if let Some(ref mut v) = tags.project { Self::dedup(v); }
            if let Some(ref mut v) = tags.universal { Self::dedup(v); }
        }
        if let Some(ref mut et) = project.entity_types {
            if let Some(ref mut v) = et.allowed { Self::dedup(v); }
        }
        if let Some(ref mut rt) = project.relation_types {
            if let Some(ref mut v) = rt.allowed { Self::dedup(v); }
        }
        if let Some(ref mut in_) = project.index_nodes {
            if let Some(ref mut v) = in_.required { Self::dedup(v); }
        }

        let tags_added = project.tags.as_ref()
            .and_then(|t| t.project.as_ref())
            .map(|v| v.len())
            .unwrap_or(0)
            .saturating_sub(tags_before);
        let entity_types_added = project.entity_types.as_ref()
            .and_then(|e| e.allowed.as_ref())
            .map(|v| v.len())
            .unwrap_or(0)
            .saturating_sub(entity_types_before);
        let relation_types_added = project.relation_types.as_ref()
            .and_then(|r| r.allowed.as_ref())
            .map(|v| v.len())
            .unwrap_or(0)
            .saturating_sub(relation_types_before);
        let index_nodes_added = project.index_nodes.as_ref()
            .and_then(|i| i.required.as_ref())
            .map(|v| v.len())
            .unwrap_or(0)
            .saturating_sub(index_nodes_before);

        let toml_string = toml::to_string(&project)?;
        if let Some(parent) = project_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&project_path, toml_string)?;

        // Sync self so callers see the updated state.
        self.tags.project.extend(new_tags.iter().cloned());
        self.entity_types.allowed.extend(new_entity_types.iter().cloned());
        self.relation_types.allowed.extend(new_relationship_types.iter().cloned());
        self.index_nodes.required.extend(new_index_nodes.iter().cloned());

        Self::dedup(&mut self.tags.project);
        Self::dedup(&mut self.entity_types.allowed);
        Self::dedup(&mut self.relation_types.allowed);
        Self::dedup(&mut self.index_nodes.required);

        Ok(format!(
            "Schema updated: +{} tags, +{} entity types, +{} relation types, +{} index nodes",
            tags_added,
            entity_types_added,
            relation_types_added,
            index_nodes_added,
        ))
    } 
}