use std::collections::HashSet;

use crate::graph::schema::GraphSchema;
use crate::graph::types::{Entity, ObservationInput, Relation, ValidationWarning};

fn validate_observation_tags(entity_name: &str, observations: &[String], allowed_tags: &HashSet<&str>) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();
    for observation in observations {
        if !observation.starts_with('[') {
            warnings.push(ValidationWarning {
                entity_name: Some(entity_name.to_string()),
                message: format!("Observation on '{}' is missing a [tag] prefix: '{}'", entity_name, observation.chars().take(50).collect::<String>()),
            });
        } else if let Some(close) = observation.find(']') {
            let tag = &observation[1..close];
            if !allowed_tags.contains(tag) {
                warnings.push(ValidationWarning {
                    entity_name: Some(entity_name.to_string()),
                    message: format!("Observation on '{}' uses unknown tag '[{}]'. Allowed tags: [{}]", entity_name, tag, allowed_tags.iter().cloned().collect::<Vec<_>>().join(", ")),
                });
            }
        } else {
            warnings.push(ValidationWarning {
                entity_name: Some(entity_name.to_string()),
                message: format!("Observation on '{}' is missing a closing bracket for tag: '{}'", entity_name, observation.chars().take(50).collect::<String>()),
            });
        }
    }
    warnings
}

pub fn validate_entities(entities: &[Entity], schema: &GraphSchema) -> Vec<ValidationWarning> {
    let mut warnings: Vec<ValidationWarning> = Vec::new();
    let allowed_tags = schema.all_allowed_tags();
    for entity in entities {
        if !schema.entity_types.allowed.contains(&entity.entity_type) {
            warnings.push(ValidationWarning { entity_name: Some(entity.name.clone()), message: format!("Entity '{}' has unknown type '{}'. Allowed types: [{}]", entity.name, entity.entity_type, schema.entity_types.allowed.join(",")) });
        }
        if schema.tags.require_prefix && !entity.name.starts_with("Index:") {
            warnings.extend(validate_observation_tags(&entity.name, &entity.observations, &allowed_tags));
        }
    }
    warnings
}

pub fn validate_observations(inputs: &[ObservationInput], existing_entities: &[Entity], schema: &GraphSchema,) -> Vec<ValidationWarning> {
    let mut warnings: Vec<ValidationWarning> = Vec::new();
    let allowed_tags = schema.all_allowed_tags();
    let existing_names: HashSet<&str> = existing_entities.iter().map(|e| e.name.as_str()).collect();
    for input in inputs {
        if !existing_names.contains(input.entity_name.as_str()) {
            warnings.push(ValidationWarning { entity_name: Some(input.entity_name.clone()), message: format!("Entity '{}' does not exist in the graph. Observation will be skipped.", input.entity_name) });
            continue;
        }
        if schema.tags.require_prefix && !input.entity_name.starts_with("Index:") {
            warnings.extend(validate_observation_tags(&input.entity_name, &input.contents, &allowed_tags));
        }
    }
    warnings
}

pub fn validate_relations(relations: &[Relation], existing_entities: &[Entity], schema: &GraphSchema) -> Vec<ValidationWarning> {
    let mut warnings: Vec<ValidationWarning> = Vec::new();
    let existing_names: HashSet<&str> = existing_entities.iter().map(|e| e.name.as_str()).collect();
    for relation in relations {
        if !schema.relation_types.allowed.contains(&relation.relation_type) {
            warnings.push(ValidationWarning { entity_name: None, message: format!("Relation '{}' → '{}' has unknown type '{}'. Allowed types: [{}]", relation.from, relation.to, relation.relation_type, schema.relation_types.allowed.join(",")) });
        }
        if !existing_names.contains(relation.from.as_str()) {
            warnings.push(ValidationWarning { entity_name: None, message: format!("Relation references non-existent entity '{}'", relation.from) });
        }
        if !existing_names.contains(relation.to.as_str()) {
            warnings.push(ValidationWarning { entity_name: None, message: format!("Relation references non-existent entity '{}'", relation.to) });
        }
    }
    warnings
}

pub fn check_duplicate_observations(new_observations: &[String], existing_observations: &[String]) -> Vec<ValidationWarning> {
    let mut warnings: Vec<ValidationWarning> = Vec::new();
    let existing_tokens: Vec<HashSet<String>> = existing_observations.iter().map(|o| o.split_ascii_whitespace().map(|w| w.to_lowercase()).collect()).collect();
    let new_tokens: Vec<HashSet<String>> = new_observations.iter().map(|o| o.split_ascii_whitespace().map(|w| w.to_lowercase()).collect()).collect();
    for new_obs in &new_tokens {
        for (i, existing_obs) in existing_tokens.iter().enumerate() {
            let intersection = existing_obs.intersection(new_obs).count();
            let union = existing_obs.union(new_obs).count();
            if union > 0 && intersection as f64 / union as f64 > 0.7 {
                let preview = existing_observations[i].chars().take(60).collect::<String>();
                warnings.push(ValidationWarning {
                    entity_name: None,
                    message: format!("New observation may duplicate existing: '{}'", preview),
                });
                break;
            }
        }
    }
    warnings
}

pub fn check_observation_count(
    entity_name: &str,
    current_count: usize,
    adding_count: usize,
    threshold: usize,
) -> Option<ValidationWarning> {
    let total = current_count + adding_count;
    if total > threshold {
        Some(ValidationWarning {
            entity_name: Some(entity_name.to_string()),
            message: format!(
                "Entity '{}' will have {} observations (threshold: {}). Consider consolidating or splitting.",
                entity_name, total, threshold
            ),
        })
    } else {
        None
    }
}

pub fn check_index_node_coverage(
    new_entity: &Entity,
    schema: &GraphSchema,
) -> Option<ValidationWarning> {
    if new_entity.entity_type == "Index" {
        return None;
    }
    if schema.index_nodes.required.is_empty() {
        return None;
    }
    Some(ValidationWarning {
        entity_name: Some(new_entity.name.clone()),
        message: format!(
            "New entity '{}' (type: {}) created. Consider adding it to the relevant Index node.",
            new_entity.name, new_entity.entity_type
        ),
    })
}