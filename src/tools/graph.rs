use schemars::JsonSchema;
use serde::Deserialize;
use crate::graph::types::{Entity, ObservationDeletion, ObservationInput, Relation};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateEntitiesParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "Entities to create. Each needs a name (PascalCase for components, Index:DomainName for indices), entityType, and observations (each a single atomic fact with a tag prefix like [file], [purpose], [gotcha]).")]
    pub entities: Vec<Entity>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddObservationsParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "Observations to add, keyed by entity name. Each observation should be a single atomic fact with a tag prefix. Silently skips entity names that don't exist.")]
    pub observations: Vec<ObservationInput>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteEntitiesParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "Exact names of entities to delete. Also removes all relations referencing these entities. Verify names with read_entities or list_entities first.")]
    pub entity_names: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteObservationsParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "Observations to delete, keyed by entity name. Observation text must match exactly. Read the entity first to get exact observation text.")]
    pub deletions: Vec<ObservationDeletion>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateRelationsParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "Relations to create. Each needs from (source entity name), to (target entity name), and relationType (snake_case, e.g. depends_on, contains, used_by). Both entities should already exist.")]
    pub relations: Vec<Relation>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteRelationsParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "Relations to delete. The from, to, and relationType must all match exactly.")]
    pub relations: Vec<Relation>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchGraphParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "Case-insensitive substring to search for. Use 1–2 words maximum. For tagged observations, search for the tag (e.g. 'gotcha', 'decision'). For domain exploration, search for 'Index:' to find index entities.")]
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadGraphParams {
    #[schemars(description = "Project name as registered in config.toml. Warning: this loads the entire graph — prefer search_graph or read_entities for targeted access.")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListEntitiesParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadEntitiesParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "Exact entity names to read. Names are case-sensitive. Use search_graph or list_entities first if unsure of exact names.")]
    pub entity_names: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateGraphParams {
    #[schemars(description = "The project name as registered in config.toml")]
    pub project: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateGraphSchemaParams {
    #[schemars(description = "The project name to update the schema for")]
    pub project: String,
    #[schemars(description = "New project-scoped tags to add")]
    pub new_tags: Option<Vec<String>>,
    #[schemars(description = "New entity types to add to the allowed list")]
    pub new_entity_types: Option<Vec<String>>,
    #[schemars(description = "New relation types to add to the allowed list")]
    pub new_relationship_types: Option<Vec<String>>,
    #[schemars(description = "New index node names to add to the required list")]
    pub new_index_nodes: Option<Vec<String>>,
}