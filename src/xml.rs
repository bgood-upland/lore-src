use crate::graph::types::ValidationWarning;
use crate::knowledge::store::{FileSearchResult, SearchMatch};

pub fn wrap_xml(tag: &str, content: &str) -> String {
    let escaped = content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let tag_name = tag.split_whitespace().next().unwrap_or(tag);
    format!("<{tag}>\n{escaped}\n</{tag_name}>")
}

// ─── Graph entity formatting ────────────────────────────────────

pub fn format_entity_xml(entity: &crate::graph::types::Entity) -> String {
    if entity.observations.is_empty() {
        return format!("<entity name=\"{}\" type=\"{}\" />", entity.name, entity.entity_type);
    }
    let observations = entity.observations.iter()
        .map(|o| format!("  <observation>{}</observation>", o))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<entity name=\"{}\" type=\"{}\">\n{}\n</entity>",
        entity.name, entity.entity_type, observations
    )
}

pub fn format_graph_result_xml(
    graph: &crate::graph::types::KnowledgeGraph,
    project: &str,
    query: Option<&str>,
) -> String {
    let query_attr = match query {
        Some(q) => format!(" query=\"{}\"", q),
        None => String::new(),
    };

    if graph.entities.is_empty() && graph.relations.is_empty() {
        return format!(
            "<graph_results project=\"{}\"{} entities=\"0\" relations=\"0\" />",
            project, query_attr
        );
    }

    let entities_xml = graph.entities.iter()
        .map(format_entity_xml)
        .collect::<Vec<_>>()
        .join("\n");
    let relations_xml = graph.relations.iter()
        .map(|r| format!(
            "  <relation from=\"{}\" type=\"{}\" to=\"{}\" />",
            r.from, r.relation_type, r.to
        ))
        .collect::<Vec<_>>()
        .join("\n");
    let relations_block = if relations_xml.is_empty() {
        String::new()
    } else {
        format!("<relations>\n{}\n</relations>", relations_xml)
    };

    // Build inner content conditionally to avoid a trailing blank line
    // when relations_block is empty.
    let inner = if relations_block.is_empty() {
        entities_xml
    } else {
        format!("{}\n{}", entities_xml, relations_block)
    };

    format!(
        "<graph_results project=\"{}\"{} entities=\"{}\" relations=\"{}\">\n{}\n</graph_results>",
        project,
        query_attr,
        graph.entities.len(),
        graph.relations.len(),
        inner,
    )
}

pub fn format_entity_list_xml(
    entities: &[crate::graph::types::EntitySummary],
    project: &str,
) -> String {
    let items_xml = entities.iter()
        .map(|e| format!("<entity name=\"{}\" type=\"{}\" />", e.name, e.entity_type))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<entity_list project=\"{}\" count=\"{}\">\n{}\n</entity_list>",
        project, entities.len(), items_xml
    )
}

pub fn format_entities_xml(
    entities: &[crate::graph::types::Entity],
    project: &str,
) -> String {
    let entities_xml = entities.iter()
        .map(format_entity_xml)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<entities project=\"{}\" count=\"{}\">\n{}\n</entities>",
        project, entities.len(), entities_xml
    )
}

pub fn format_relations_xml(relations: &[crate::graph::types::Relation], project: &str) -> String {
    if relations.is_empty() {
        return format!("<relations project=\"{}\" count=\"0\" />", project);
    }
    let items = relations.iter()
        .map(|r| format!(
            "  <relation from=\"{}\" type=\"{}\" to=\"{}\" />",
            r.from, r.relation_type, r.to
        ))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<relations project=\"{}\" count=\"{}\">\n{}\n</relations>",
        project, relations.len(), items
    )
}

pub fn format_schema_xml(schema: &crate::graph::schema::GraphSchema) -> String {
    let mut entity_types = schema.entity_types.allowed.clone();
    entity_types.sort();

    let mut relation_types = schema.relation_types.allowed.clone();
    relation_types.sort();

    let mut tags: Vec<&str> = schema.all_allowed_tags().into_iter().collect();
    tags.sort();

    if entity_types.is_empty() && relation_types.is_empty() && tags.is_empty() {
        return "<graph_schema />".to_string();
    }

    format!(
        "<graph_schema>\n<entity_types>{}</entity_types>\n<relation_types>{}</relation_types>\n<tags>{}</tags>\n</graph_schema>",
        entity_types.join(", "),
        relation_types.join(", "),
        tags.join(", "),
    )
}

// ─── Warning formatting ─────────────────────────────────────────

pub fn format_warning_xml(w: &ValidationWarning) -> String {
    let entity = w.entity_name.as_deref().unwrap_or("graph");
    format!("<warning entity=\"{}\">{}</warning>", entity, w.message)
}

pub fn format_warnings_block(warnings: &[ValidationWarning]) -> String {
    let inner = warnings.iter()
        .map(format_warning_xml)
        .collect::<Vec<_>>()
        .join("\n");
    format!("<warnings>\n{inner}\n</warnings>")
}

pub fn format_validation_report_xml(warnings: &[ValidationWarning]) -> String {
    let inner = warnings.iter()
        .map(format_warning_xml)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<validation_report warnings=\"{}\">\n{}\n</validation_report>",
        warnings.len(),
        inner
    )
}

// ─── Knowledge search formatting ────────────────────────────────

fn format_search_match_xml(m: &SearchMatch) -> String {
    let tag = match &m.section_heading {
        Some(heading) => format!("match section=\"{}\"", heading),
        None => "match".to_string(),
    };
    wrap_xml(&tag, &m.line)
}

pub fn format_search_results_xml(
    results: &[FileSearchResult],
    project: &str,
    query: &str,
) -> String {
    let files_xml: Vec<String> = results.iter()
        .map(|r| {
            let matches = r.matches.iter()
                .map(format_search_match_xml)
                .collect::<Vec<_>>()
                .join("\n");
            format!("<file key=\"{}\">\n{}\n</file>", r.file_key, matches)
        })
        .collect();
    format!(
        "<search_results query=\"{}\" project=\"{}\">\n{}\n</search_results>",
        query, project, files_xml.join("\n")
    )
}

pub fn format_file_list_xml(files: &[crate::knowledge::store::FileSummary], project: &str) -> String {
    if files.is_empty() {
        return format!("<knowledge_files project=\"{}\" count=\"0\" />", project);
    }
    let items = files.iter()
        .map(|f| {
            let mut attrs = format!("key=\"{}\" path=\"{}\"", f.key, f.path);
            if let Some(ref w) = f.when_to_read {
                attrs.push_str(&format!(" when_to_read=\"{}\"", w));
            }
            if let Some(ref u) = f.last_updated {
                attrs.push_str(&format!(" last_updated=\"{}\"", u));
            }
            format!("<file {}>{}</file>", attrs, f.summary)
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<knowledge_files project=\"{}\" count=\"{}\">\n{}\n</knowledge_files>",
        project, files.len(), items
    )
}

pub fn format_skill_list_xml(skills: &[crate::skills::types::SkillSummary], project: &str) -> String {
    if skills.is_empty() {
        return format!("<skills project=\"{}\" count=\"0\" />", project);
    }
    let items = skills.iter()
        .map(|s| format!(
            "<skill key=\"{}\" name=\"{}\" scope=\"{}\">{}</skill>",
            s.skill_key, s.name, s.scope, s.description
        ))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<skills project=\"{}\" count=\"{}\">\n{}\n</skills>",
        project, skills.len(), items
    )
}

// Skills
pub fn format_skill_files_xml(files: &[crate::skills::types::SkillFileEntry], skill_key: &str) -> String {
    if files.is_empty() {
        return format!("<skill_files skill=\"{}\" count=\"0\" />", skill_key);
    }
    let items = files.iter()
        .map(|f| format!("<file path=\"{}\" size=\"{}\" />", f.relative_path, f.size))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<skill_files skill=\"{}\" count=\"{}\">\n{}\n</skill_files>",
        skill_key, files.len(), items
    )
}