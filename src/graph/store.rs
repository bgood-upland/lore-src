use std::collections::HashSet;
use std::path::{Path, PathBuf};
use crate::resolver::SharedResolver;
use crate::graph::schema::GraphSchema;
use crate::graph::validation;
use crate::graph::types::{Entity, KnowledgeGraph, ObservationDeletion, EntitySummary, ObservationInput, Relation, ValidationWarning, WriteResult};

/// Stateless coordinator for all knowledge graph operations
#[derive(Clone)]
pub struct GraphStore {
    resolver: SharedResolver,
    defaults_dir: PathBuf,
}

impl GraphStore {
    pub fn new(resolver: SharedResolver, defaults_dir: PathBuf) -> Self {
        Self { resolver, defaults_dir }
    }

    fn resolve(&self, project: &str) -> anyhow::Result<PathBuf> {
        self.resolver.read().unwrap().resolve(project)
    }

    fn resolve_writable(&self, project: &str) -> anyhow::Result<PathBuf> {
        self.resolver.read().unwrap().resolve_writable(project)
    }

    fn graph_path(root: &Path) -> PathBuf {
        root.join(".lore/graph/memory.jsonl")
    }

    fn load_graph(root: &Path) -> anyhow::Result<KnowledgeGraph> {
        let path = Self::graph_path(root);
        if !path.exists() {
            return Ok(KnowledgeGraph::default());
        }
        let content = std::fs::read_to_string(path)?;
        Ok(KnowledgeGraph::from_jsonl(&content)?)
    }

    fn save_graph(root: &Path, graph: &KnowledgeGraph) -> anyhow::Result<()> {
        let path = Self::graph_path(root);
        let content = graph.to_jsonl()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Add new entities to the graph, skipping any whose name already exists
    pub fn create_entities(&self, project: &str, entities: Vec<Entity>) -> anyhow::Result<WriteResult<Vec<Entity>>> {
        let root = self.resolve_writable(project)?;
        let mut graph = Self::load_graph(&root)?;

        let existing: HashSet<String> = graph.entities.iter().map(|e| e.name.clone()).collect();
        let new_entities: Vec<Entity> = entities
            .into_iter()
            .filter(|e| !existing.contains(&e.name))
            .collect();

        let mut warnings: Vec<ValidationWarning> = Vec::new();
        if let Some(ref schema) = GraphSchema::load(&self.defaults_dir, &root)? {
            warnings.extend(validation::validate_entities(&new_entities, schema));
        }

        graph.entities.extend(new_entities.clone());
        Self::save_graph(&root, &graph)?;
        Ok(WriteResult { data: new_entities, warnings })
    }

    /// Add observations to existing entities
    pub fn add_observations(&self, project: &str, inputs: Vec<ObservationInput>) -> anyhow::Result<WriteResult<usize>> {
        let root = self.resolve_writable(project)?;
        let mut graph = Self::load_graph(&root)?;

        let mut warnings: Vec<ValidationWarning> = Vec::new();
        let schema = GraphSchema::load(&self.defaults_dir, &root)?;

        if let Some(ref schema) = schema {
            warnings.extend(validation::validate_observations(&inputs, &graph.entities, schema));
        }

        let mut total_added = 0usize;
        for input in &inputs {
            if let Some(entity) = graph.entities.iter_mut().find(|e| e.name == input.entity_name) {
                if let Some(ref _schema) = schema {
                    warnings.extend(validation::check_duplicate_observations(&input.contents, &entity.observations));
                    if let Some(w) = validation::check_observation_count(
                        &input.entity_name,
                        entity.observations.len(),
                        input.contents.len(),
                        25,
                    ) {
                        warnings.push(w);
                    }
                }
                entity.observations.extend(input.contents.clone());
                total_added += input.contents.len();
            }
        }

        Self::save_graph(&root, &graph)?;
        Ok(WriteResult { data: total_added, warnings })
    }

    /// Remove entities by name. Also removes any relations that reference them
    pub fn delete_entities(&self, project: &str, names: Vec<String>) -> anyhow::Result<()> {
        let root = self.resolve_writable(project)?;
        let mut graph = Self::load_graph(&root)?;
        let name_set: HashSet<&String> = names.iter().collect();
        graph.entities.retain(|e| !name_set.contains(&e.name));
        graph.relations.retain(|r| !name_set.contains(&r.from) && !name_set.contains(&r.to));
        Self::save_graph(&root, &graph)?;
        Ok(())
    }

    /// Remove specific observations from entities.
    pub fn delete_observations(&self, project: &str, deletions: Vec<ObservationDeletion>) -> anyhow::Result<Vec<String>> {
        let root = self.resolve_writable(project)?;
        let mut graph = Self::load_graph(&root)?;
        let mut found: Vec<String> = Vec::new();
        for deletion in &deletions {
            if let Some(entity) = graph.entities.iter_mut().find(|e| e.name == deletion.entity_name) {
                let to_remove: HashSet<&String> = deletion.observations.iter().collect();
                entity.observations.retain(|o| !to_remove.contains(o));
                found.push(deletion.entity_name.clone());
            }
        }
        Self::save_graph(&root, &graph)?;
        Ok(found)
    }

    /// Add new relations to the graph, skipping duplicates.
    pub fn create_relations(&self, project: &str, relations: Vec<Relation>) -> anyhow::Result<WriteResult<Vec<Relation>>> {
        let root = self.resolve_writable(project)?;
        let mut graph = Self::load_graph(&root)?;

        let existing: HashSet<(&str, &str, &str)> = graph.relations
            .iter()
            .map(|r| (r.from.as_str(), r.to.as_str(), r.relation_type.as_str()))
            .collect();
        let new_relations: Vec<Relation> = relations
            .into_iter()
            .filter(|r| !existing.contains(&(r.from.as_str(), r.to.as_str(), r.relation_type.as_str())))
            .collect();

        let mut warnings: Vec<ValidationWarning> = Vec::new();
        if let Some(ref schema) = GraphSchema::load(&self.defaults_dir, &root)? {
            warnings.extend(validation::validate_relations(&new_relations, &graph.entities, schema));
        }

        graph.relations.extend(new_relations.clone());
        Self::save_graph(&root, &graph)?;
        Ok(WriteResult { data: new_relations, warnings })
    }

    /// Remove specific relations from the graph.
    pub fn delete_relations(&self, project: &str, relations: Vec<Relation>) -> anyhow::Result<usize> {
        let root = self.resolve_writable(project)?;
        let mut graph = Self::load_graph(&root)?;
        let before = graph.relations.len();
        let delete_set: HashSet<(&str, &str, &str)> = relations
            .iter()
            .map(|r| (r.from.as_str(), r.to.as_str(), r.relation_type.as_str()))
            .collect();
        graph.relations.retain(|r| {
            !delete_set.contains(&(r.from.as_str(), r.to.as_str(), r.relation_type.as_str()))
        });
        let removed = before - graph.relations.len();
        Self::save_graph(&root, &graph)?;
        Ok(removed)
    }

    /// Search the graph for a query string.
    pub fn search_graph(&self, project: &str, query: &str) -> anyhow::Result<KnowledgeGraph> {
        let root = self.resolve(project)?;
        let graph = Self::load_graph(&root)?;
        let query = query.to_lowercase();
        let mut matches: HashSet<String> = HashSet::new();
        for entity in &graph.entities {
            if entity.name.to_lowercase().contains(&query)
                || entity.entity_type.to_lowercase().contains(&query)
                || entity.observations.iter().any(|o| o.to_lowercase().contains(&query))
            {
                matches.insert(entity.name.clone());
            }
        }
        let entities = graph.entities.iter().filter(|e| matches.contains(&e.name)).cloned().collect();
        let relations = graph.relations.iter().filter(|r| matches.contains(&r.from) && matches.contains(&r.to)).cloned().collect();
        Ok(KnowledgeGraph { entities, relations })
    }

    /// Return the full graph for a project.
    pub fn read_graph(&self, project: &str) -> anyhow::Result<KnowledgeGraph> {
        let root = self.resolve(project)?;
        Self::load_graph(&root)
    }

    pub fn list_entities(&self, project: &str) -> anyhow::Result<Vec<EntitySummary>> {
        let root = self.resolve(project)?;
        let graph = Self::load_graph(&root)?;
        Ok(graph.entities.iter()
            .map(|e| EntitySummary {
                name: e.name.clone(),
                entity_type: e.entity_type.clone()
            })
            .collect())
    }

    pub fn read_entities(&self, project: &str, names: Vec<String>) -> anyhow::Result<KnowledgeGraph> {
        let root = self.resolve(project)?;
        let graph = Self::load_graph(&root)?;
        let names_set: HashSet<&String> = names.iter().collect();
        let entities = graph.entities.into_iter()
            .filter(|e| names_set.contains(&e.name))
            .collect();
        let relations = graph.relations.into_iter()
            .filter(|r| names_set.contains(&r.from) || names_set.contains(&r.to))
            .collect();
        Ok(KnowledgeGraph { entities, relations })
    }

    pub fn validate_graph(&self, project: &str) -> anyhow::Result<Option<Vec<ValidationWarning>>> {
        let root = self.resolve(project)?;
        let graph = Self::load_graph(&root)?;
        let schema = GraphSchema::load(&self.defaults_dir, &root)?;

        let Some(schema) = schema else {
            return Ok(None);
        };

        let mut warnings: Vec<ValidationWarning> = Vec::new();

        warnings.extend(validation::validate_entities(&graph.entities, &schema));

        for entity in &graph.entities {
            warnings.extend(
                validation::check_duplicate_observations(&entity.observations, &[])
            );
            if let Some(w) = validation::check_observation_count(
                &entity.name,
                entity.observations.len(),
                0,
                25,
            ) {
                warnings.push(w);
            }
        }

        warnings.extend(validation::validate_relations(&graph.relations, &graph.entities, &schema));

        Ok(Some(warnings))
    }

    pub fn update_project_schema(
        &self,
        project: &str,
        new_tags: &[String],
        new_entity_types: &[String],
        new_relationship_types: &[String],
        new_index_nodes: &[String],
    ) -> anyhow::Result<String> {
        let root = self.resolve_writable(project)?;

        let mut schema = GraphSchema::load(&self.defaults_dir, &root)?
            .unwrap_or_else(|| GraphSchema::empty());

        schema.update_project_schema(
            &root,
            new_tags,
            new_entity_types,
            new_relationship_types,
            new_index_nodes,
        )
    }

    pub fn get_schema(&self, project: &str) -> anyhow::Result<Option<GraphSchema>> {
        let root = self.resolve(project)?;
        GraphSchema::load(&self.defaults_dir, &root)
    }
}