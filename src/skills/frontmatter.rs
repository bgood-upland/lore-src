use anyhow::{Result, Context};
use super::types::SkillMetadata;

/// Extracts YAML frontmatter from a SKILL.md file's contents
pub fn parse_frontmatter(content: &str) -> Result<SkillMetadata> {
    let mut lines = content.lines();
    let first_line = lines
        .next()
        .context("SKILL.md is empty")?;
    if first_line.trim() != "---" {
        anyhow::bail!("SKILL.md does not start with frontmatter delimiter '---'")
    }

    let mut yaml_block = String::new();
    let mut found_closing = false;

    for line in lines {
        if line.trim() == "---" {
            found_closing = true;
            break;
        }

        yaml_block.push_str(line);
        yaml_block.push('\n');
    }

    if !found_closing {
        anyhow::bail!("SKILL.md frontmatter has opening '---' but no closing '---'");
    }

    let metadata: SkillMetadata = serde_yaml::from_str(&yaml_block)
        .context("Failed to parse SKILL.md frontmatter as valid YAML")?;

    Ok(metadata)
}

/// Attempts to parse frontmatter, returning a fallback SkillMetadata
pub fn parse_frontmatter_or_fallback(content: &str, dir_name: &str) -> SkillMetadata {
    match parse_frontmatter(content) {
        Ok(metadata) => metadata,
        Err(_) => SkillMetadata { name:dir_name.to_string(), description: "No description available".to_string() }
    }
}