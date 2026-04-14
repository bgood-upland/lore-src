use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use dialoguer::{MultiSelect, theme::ColorfulTheme};

// ─── Types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppId {
    ClaudeDesktop,
    ClaudeCode,
    Codex,
}

pub struct AppTarget {
    pub id: AppId,
    pub display_name: &'static str,
    pub cli_name: &'static str,
}

pub struct AppStatus {
    pub target: &'static AppTarget,
    pub detected: bool,
    pub config_path: Option<PathBuf>,
    pub already_configured: bool,
}

// ─── App Registry ──────────────────────────────────────────────

const APPS: &[AppTarget] = &[
    AppTarget {
        id: AppId::ClaudeDesktop,
        display_name: "Claude Desktop",
        cli_name: "claude-desktop",
    },
    AppTarget {
        id: AppId::ClaudeCode,
        display_name: "Claude Code",
        cli_name: "claude-code",
    },
    AppTarget {
        id: AppId::Codex,
        display_name: "OpenAI Codex",
        cli_name: "codex",
    },
];

// ─── Detection ─────────────────────────────────────────────────

/// Returns the config file path for this app, or None if not installed.
fn detect_app(app: &AppTarget) -> Option<PathBuf> {
    match app.id {
        AppId::ClaudeDesktop => detect_claude_desktop(),
        AppId::ClaudeCode => detect_claude_code(),
        AppId::Codex => detect_codex(),
    }
}

/// Claude Desktop config dir:
///   macOS:   ~/Library/Application Support/Claude/
///   Windows: %APPDATA%\Claude\
/// Both resolve via dirs::config_dir().join("Claude").
fn detect_claude_desktop() -> Option<PathBuf> {
    let config_dir = dirs::config_dir()?.join("Claude");
    if config_dir.is_dir() {
        Some(config_dir.join("claude_desktop_config.json"))
    } else {
        None
    }
}

/// Claude Code detection: ~/.claude/ directory exists.
/// Config file: ~/.claude.json (note: file is at home root, not inside .claude/).
fn detect_claude_code() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    if home.join(".claude").is_dir() {
        Some(home.join(".claude.json"))
    } else {
        None
    }
}

/// Codex detection: ~/.codex/ directory exists.
/// Config file: ~/.codex/config.toml.
fn detect_codex() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let codex_dir = home.join(".codex");
    if codex_dir.is_dir() {
        Some(codex_dir.join("config.toml"))
    } else {
        None
    }
}

/// Check whether a "lore" entry already exists in the app's config.
fn is_configured(app: &AppTarget, config_path: &Path) -> bool {
    match app.id {
        AppId::ClaudeDesktop | AppId::ClaudeCode => {
            fs::read_to_string(config_path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| v.get("mcpServers")?.get("lore").cloned())
                .is_some()
        }
        AppId::Codex => {
            fs::read_to_string(config_path)
                .ok()
                .and_then(|s| s.parse::<toml::Value>().ok())
                .and_then(|v| v.get("mcp_servers")?.get("lore").cloned())
                .is_some()
        }
    }
}

/// Check all known apps: detect installation and existing configuration.
fn check_all_apps() -> Vec<AppStatus> {
    APPS.iter()
        .map(|app| match detect_app(app) {
            Some(config_path) => {
                let already_configured = is_configured(app, &config_path);
                AppStatus {
                    target: app,
                    detected: true,
                    config_path: Some(config_path),
                    already_configured,
                }
            }
            None => AppStatus {
                target: app,
                detected: false,
                config_path: None,
                already_configured: false,
            },
        })
        .collect()
}

// ─── Launcher Path ─────────────────────────────────────────────

/// Returns the platform-appropriate launcher path inside data_dir.
fn launcher_path(data_dir: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        data_dir.join("bin").join("launch.cmd")
    }
    #[cfg(not(target_os = "windows"))]
    {
        data_dir.join("bin").join("launch.sh")
    }
}

// ─── Config Patching ───────────────────────────────────────────

fn patch_config(app: &AppTarget, config_path: &Path, data_dir: &Path) -> Result<()> {
    let launcher = launcher_path(data_dir);
    match app.id {
        AppId::ClaudeDesktop => patch_claude_desktop(config_path, &launcher),
        AppId::ClaudeCode => patch_claude_code(config_path, &launcher),
        AppId::Codex => patch_codex(config_path, &launcher),
    }
}

/// Claude Desktop: JSON with mcpServers.lore entry.
/// {"command": "<launcher>", "args": [], "env": {"RUST_BACKTRACE": "1"}}
fn patch_claude_desktop(config_path: &Path, launcher: &Path) -> Result<()> {
    let launcher_str = launcher.to_string_lossy();
    let entry = serde_json::json!({
        "command": launcher_str,
        "args": [],
        "env": { "RUST_BACKTRACE": "1" }
    });
    patch_json_mcp_config(config_path, entry)
        .context("Failed to patch Claude Desktop config")
}

/// Claude Code: JSON at ~/.claude.json with mcpServers.lore entry.
/// Includes "type": "stdio" field. File may contain OAuth tokens, preferences,
/// project settings — all must be preserved.
/// {"type": "stdio", "command": "<launcher>", "args": []}
fn patch_claude_code(config_path: &Path, launcher: &Path) -> Result<()> {
    let launcher_str = launcher.to_string_lossy();
    let entry = serde_json::json!({
        "type": "stdio",
        "command": launcher_str,
        "args": []
    });
    patch_json_mcp_config(config_path, entry)
        .context("Failed to patch Claude Code config")
}

/// Shared JSON read-modify-write for Claude Desktop and Claude Code.
/// Both store MCP servers under a top-level "mcpServers" object.
/// Creates the file and parent directories if they don't exist.
/// Preserves all keys we don't own.
fn patch_json_mcp_config(config_path: &Path, entry: serde_json::Value) -> Result<()> {
    let mut config: serde_json::Value = if config_path.exists() {
        let content = fs::read_to_string(config_path)
            .context("Failed to read config file")?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        serde_json::json!({})
    };

    let obj = config
        .as_object_mut()
        .context("Config file root is not a JSON object")?;

    if !obj.contains_key("mcpServers") {
        obj.insert("mcpServers".to_string(), serde_json::json!({}));
    }

    let servers = obj
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
        .context("mcpServers is not a JSON object")?;

    servers.insert("lore".to_string(), entry);

    let json = serde_json::to_string_pretty(&config)?;
    // fs::write produces raw UTF-8 — no BOM (important for Windows/Claude Desktop)
    fs::write(config_path, format!("{}\n", json))?;

    Ok(())
}

/// Codex: TOML at ~/.codex/config.toml with [mcp_servers.lore] table.
/// File may contain other Codex config sections that must be preserved.
fn patch_codex(config_path: &Path, launcher: &Path) -> Result<()> {
    let launcher_str = launcher.to_string_lossy().to_string();

    let mut config: toml::Value = if config_path.exists() {
        let content = fs::read_to_string(config_path)
            .context("Failed to read Codex config")?;
        content
            .parse()
            .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()))
    } else {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        toml::Value::Table(toml::map::Map::new())
    };

    let root = config
        .as_table_mut()
        .context("Codex config root is not a TOML table")?;

    if !root.contains_key("mcp_servers") {
        root.insert(
            "mcp_servers".to_string(),
            toml::Value::Table(toml::map::Map::new()),
        );
    }

    let servers = root
        .get_mut("mcp_servers")
        .and_then(|v| v.as_table_mut())
        .context("mcp_servers is not a TOML table")?;

    let mut lore_table = toml::map::Map::new();
    lore_table.insert("command".to_string(), toml::Value::String(launcher_str));
    lore_table.insert("args".to_string(), toml::Value::Array(vec![]));

    let mut env_table = toml::map::Map::new();
    env_table.insert(
        "RUST_BACKTRACE".to_string(),
        toml::Value::String("1".to_string()),
    );
    lore_table.insert("env".to_string(), toml::Value::Table(env_table));

    servers.insert("lore".to_string(), toml::Value::Table(lore_table));

    let toml_str =
        toml::to_string_pretty(&config).context("Failed to serialize Codex config")?;
    fs::write(config_path, toml_str)?;

    Ok(())
}

// ─── Public Entry Points ───────────────────────────────────────

/// Interactive mode: detect apps, show MultiSelect, patch selected ones.
pub fn configure_interactive(data_dir: &Path) -> Result<()> {
    let statuses = check_all_apps();
    let detected: Vec<&AppStatus> = statuses.iter().filter(|s| s.detected).collect();

    if detected.is_empty() {
        println!("No MCP-compatible apps detected.\n");
        println!("Looked for:");
        for app in APPS {
            println!("  - {}", app.display_name);
        }
        println!("\nInstall one of these apps, then run 'lore configure' again.");
        return Ok(());
    }

    // Build display strings and defaults for the checkbox prompt
    let items: Vec<String> = detected
        .iter()
        .map(|s| {
            if s.already_configured {
                format!("{} (already configured)", s.target.display_name)
            } else {
                s.target.display_name.to_string()
            }
        })
        .collect();

    // Pre-check apps that are detected but not yet configured
    let defaults: Vec<bool> = detected.iter().map(|s| !s.already_configured).collect();

    println!("Detected MCP-compatible apps:\n");

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select apps to configure (space = toggle, enter = confirm)")
        .items(&items)
        .defaults(&defaults)
        .interact()?;

    if selections.is_empty() {
        println!("No apps selected.");
        return Ok(());
    }

    println!();
    for &idx in &selections {
        let status = detected[idx];
        let config_path = status.config_path.as_ref().unwrap();
        print!("  Configuring {}... ", status.target.display_name);
        match patch_config(status.target, config_path, data_dir) {
            Ok(()) => println!("done"),
            Err(e) => println!("failed: {}", e),
        }
    }

    println!("\nRestart the configured app(s) to activate Lore.");
    Ok(())
}

/// Configure a single app by its CLI name.
pub fn configure_app(cli_name: &str, data_dir: &Path) -> Result<()> {
    let app = APPS
        .iter()
        .find(|a| a.cli_name == cli_name)
        .ok_or_else(|| {
            let valid: Vec<&str> = APPS.iter().map(|a| a.cli_name).collect();
            anyhow::anyhow!(
                "Unknown app: '{}'\nValid options: {}",
                cli_name,
                valid.join(", ")
            )
        })?;

    let config_path = detect_app(app).ok_or_else(|| {
        anyhow::anyhow!(
            "{} does not appear to be installed on this machine.",
            app.display_name
        )
    })?;

    patch_config(app, &config_path, data_dir)?;
    println!("Configured Lore for {}.", app.display_name);
    println!("Restart {} to activate.", app.display_name);
    Ok(())
}

/// List all apps and their detection/configuration status.
pub fn configure_list(data_dir: &Path) -> Result<()> {
    let statuses = check_all_apps();

    println!("{:<20} {}", "App", "Status");
    println!("{:<20} {}", "---", "------");

    for status in &statuses {
        let label = if !status.detected {
            "not found".to_string()
        } else if status.already_configured {
            "configured".to_string()
        } else {
            "detected (not configured)".to_string()
        };
        println!("{:<20} {}", status.target.display_name, label);
    }

    let launcher = launcher_path(data_dir);
    println!("\nLauncher: {}", launcher.display());

    Ok(())
}

/// Configure all detected apps without prompting.
pub fn configure_all(data_dir: &Path) -> Result<()> {
    let statuses = check_all_apps();
    let detected: Vec<&AppStatus> = statuses.iter().filter(|s| s.detected).collect();

    if detected.is_empty() {
        println!("No MCP-compatible apps detected.");
        return Ok(());
    }

    for status in &detected {
        let config_path = status.config_path.as_ref().unwrap();
        print!("  Configuring {}... ", status.target.display_name);
        match patch_config(status.target, config_path, data_dir) {
            Ok(()) => println!("done"),
            Err(e) => println!("failed: {}", e),
        }
    }

    println!("\nRestart the configured app(s) to activate Lore.");
    Ok(())
}