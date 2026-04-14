use std::path::{Path, PathBuf};
use anyhow::{Result, bail};

const DEFAULT_CONFIG: &str = include_str!("../defaults/config.toml");
const SESSION_UPDATE_SKILL: &str = include_str!("../defaults/skills/session-knowledge-update/SKILL.md");
const PROJECT_INIT_SKILL: &str = include_str!("../defaults/skills/project-init/SKILL.md");
const DEFAULT_SCHEMA: &str = include_str!("../defaults/schema.toml");

pub fn data_dir() -> Result<PathBuf> {
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".lore"));
    };
    bail!("Home directory not found. Cannot initialize project.")
}

pub fn ensure_initialized(data_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(data_dir)?;
    write_if_missing(&data_dir.join("config.toml"), DEFAULT_CONFIG)?;
    write_if_missing(&data_dir.join("skills/session-knowledge-update/SKILL.md"), SESSION_UPDATE_SKILL)?;
    write_if_missing(&data_dir.join("skills/project-init/SKILL.md"), PROJECT_INIT_SKILL)?;
    write_if_missing(&data_dir.join("defaults/schema.toml"), DEFAULT_SCHEMA)?;
    Ok(())
}

fn write_if_missing(path: &Path, content: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}