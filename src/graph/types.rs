use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObservationInput {
    pub entity_name: String,
    pub contents: Vec<String>,
}

/// Input for delete_observations — which observations to remove from an entity  
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObservationDeletion {
    pub entity_name: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntitySummary {
    pub name: String,
    pub entity_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationWarning {
    pub entity_name: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct WriteResult<T> {
    pub data: T,
    pub warnings: Vec<ValidationWarning>,
}

/// A single entity in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Entity {
    pub name: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub observations: Vec<String>,

}

/// A directed relationship between two entities
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Relation {
    pub from: String,
    pub to: String,
    #[serde(rename = "relationType")]
    pub relation_type: String,
}

/// Used internally to parse a single JSONL line as either an Entity or Relation
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum GraphLine {
    Entity(Entity),
    Relation(Relation),
}

/// The full in-memory representation of a project's knowledge graph
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>
}

impl KnowledgeGraph {
    /// Parse the full contents of a memory.jsonl file into a KnowledgeGraph
    pub fn from_jsonl(content: &str) -> anyhow::Result<Self> {
        let mut entities: Vec<Entity> = Vec::new();
        let mut relations: Vec<Relation> = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let graph_line = serde_json::from_str::<GraphLine>(line)?;
            match graph_line {
                GraphLine::Entity(entity) => entities.push(entity),
                GraphLine::Relation(relation) => relations.push(relation)
            }
        }

        Ok(Self {entities, relations})


    }

    /// Serialize the graph back to JSONL format for writing to disk
    pub fn to_jsonl(&self) -> anyhow::Result<String> {
        let mut json_lines: Vec<String> = Vec::new();
        for entity in &self.entities {
            let line = GraphLine::Entity(entity.clone());
            json_lines.push(serde_json::to_string(&line)?);
        }
        for relation in &self.relations {
            let line = GraphLine::Relation(relation.clone());
            json_lines.push(serde_json::to_string(&line)?);
        }
        Ok(json_lines.join("\n").trim().to_string())
    }
}

