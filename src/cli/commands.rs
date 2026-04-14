use std::path::{Path, PathBuf};
use std::io::{self, Write};

use crate::git::{self};
use crate::{config::{OrchestratorConfig, ProjectEntry, ProjectMode}, scaffold};

struct Command {
    name: &'static str,
    usage: &'static str,
    description: &'static str,
    handler: fn(&mut OrchestratorConfig, &[&str], &Path, &Path) -> anyhow::Result<()>,
}

const COMMANDS: &[Command] = &[
    Command {
        name: "add-project",
        usage: "add-project <name> --root <path> | --repo <url> [--branch <branch>] --autopilot",
        description: "Register a new project in the config",
        handler: handle_add_project,
    },
    Command {
        name: "configure",
        usage: "configure [--list | --all | --app <name>]",
        description: "Configure Lore for MCP client apps (Claude Desktop, Claude Code, Codex)",
        handler: handle_configure,
    },
    Command {
        name: "init-project",
        usage: "init-project <name> --root <path> | --repo <url> [--branch <branch>]",
        description: "Scaffold (Manual) or clone (Autopilot) a project's knowledge base",
        handler: handle_init_project,
    },
    Command {
        name: "list-projects",
        usage: "list-projects",
        description: "Show all registered projects",
        handler: handle_list_projects,
    },
    Command {
        name: "remove-project",
        usage: "remove-project <name>",
        description: "Remove a project from the config",
        handler: handle_remove_project,
    },
    Command {
        name: "sync-project",
        usage: "sync-project <name>",
        description: "Force-sync an Autopilot project's clone to the latest remote state",
        handler: handle_sync_project,
    },
];

pub fn print_help() {
    println!("Available commands:\n");
    for cmd in COMMANDS {
        println!("  {:20} {}", cmd.name, cmd.description);
    }
    println!("  {:20} {}", "help", "Show this help message");
    println!("  {:20} {}", "exit", "Quit the CLI");
    println!("\nFor usage details, type: help <command>");
}

pub fn print_command_help(name: &str) {
    match COMMANDS.iter().find(|c| c.name == name) {
        Some(cmd) => {
            println!("  Usage: {}", cmd.usage);
            println!("  {}", cmd.description);
        }
        None => println!("Unknown command: '{}'", name),
    }
}

pub fn find_and_run(command: &str, config: &mut OrchestratorConfig, args: &[&str], config_path: &Path, data_dir: &Path) -> anyhow::Result<()> {
    if let Some(cmd) = COMMANDS.iter().find(|c| c.name == command) {
        (cmd.handler)(config, args, config_path, data_dir)
    } else {
        anyhow::bail!("Command {} not found. Type 'help' to view available commands.", command)
    }
}

pub fn handle_add_project(config: &mut OrchestratorConfig, args: &[&str], config_path: &Path, data_dir: &Path) -> anyhow::Result<()> {
    let command = COMMANDS.iter().find(|c| c.name == "add-project").unwrap();

    let name = args.get(0).ok_or_else(|| anyhow::anyhow!("Usage: {}", command.usage))?;
    let flag = args.get(1).ok_or_else(|| anyhow::anyhow!("Usage: {}", command.usage))?;

    if config.projects.iter().any(|p| p.name == *name) {
        anyhow::bail!("Project '{}' already exists", name);
    }

    let entry = match *flag {
        "--root" => {
            let path = args.get(2).ok_or_else(|| anyhow::anyhow!("--root requires a path"))?;
            ProjectEntry {
                name: name.to_string(),
                mode: crate::config::ProjectMode::Manual,
                root: Some(PathBuf::from(path)),
                repo: None,
                branch: None,
            }
        },
        "--repo" => {
            let url = args.get(2).ok_or_else(|| anyhow::anyhow!("--repo requires a URL"))?;

            let mut branch: Option<String> = None;
            let mut i = 3;
            while i < args.len() {
                match args[i] {
                    "--branch" => {
                        branch = Some(
                            args.get(i + 1)
                                .ok_or_else(|| anyhow::anyhow!("--branch requires a value"))?
                                .to_string()
                        );
                        i += 2;
                    },
                    _ => {
                        anyhow::bail!("Unknown flag: '{}'", args[i]);
                    }
                }
            }
            ProjectEntry {
                name: name.to_string(),
                mode: crate::config::ProjectMode::Autopilot,
                root: None,
                repo: Some(url.to_string()),
                branch: branch.or_else(|| Some("main".to_string())),
            }
        },
        _ => {
            anyhow::bail!("Expected --root <path> or --repo <url>. Type 'help add-project' for usage.");
        }
    };

    let root = entry.effective_root(data_dir);
    entry.validate()?;

    match entry.mode {
        ProjectMode::Manual => {
            // Register because root directory is confirmed to exist by validate()
            config.projects.push(entry);
            config.save(config_path)?;
            println!("Added project '{}'", name);
            if !scaffold::is_scaffolded(&root) {
                print!("Project '{}' has no .lore/ scaffolding. Initialize now? [Y/n] ", name);
                io::stdout().flush()?;
                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;
                let answer = answer.trim().to_lowercase();
                if answer.is_empty() || answer == "y" || answer == "yes" {
                    let result = scaffold::scaffold_project(&root, name)?;
                    println!("{}", result.summary());
                }
            }
        }
        ProjectMode::Autopilot => {
            // Clone then register if successful
            println!("Syncing repository...");
            let repo = entry.repo.as_ref().unwrap();
            let branch = entry.branch.as_deref().unwrap_or("main");
            let outcome = git::sync_project(repo, branch, &root)?;
            // If sync_project() returned Ok()
            config.projects.push(entry);
            config.save(config_path)?;
            println!("Added project '{}'", name);
            println!("{}", outcome.summary());
            if scaffold::is_scaffolded(&root) {
                println!("Knowledge base detected in clone.");
            } else {
                println!(
                    "Note: upstream repo has no .lore/ scaffolding.\n\
                    A developer should scaffold this project locally and push."
                );
            }
        }
    }

    Ok(())
}

pub fn handle_init_project(config: &mut OrchestratorConfig, args: &[&str], config_path: &Path, data_dir: &Path) -> anyhow::Result<()> {
    let command = COMMANDS.iter().find(|c| c.name == "init-project").unwrap();
    let name = args.get(0).ok_or_else(|| anyhow::anyhow!("Usage: {}", command.usage))?;

    // Parse flags
    let mut root: Option<PathBuf> = None;
    let mut repo: Option<String> = None;
    let mut branch: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i] {
            "--root" => {
                root = Some(PathBuf::from(
                    args.get(i + 1).ok_or_else(|| anyhow::anyhow!("--root requires a path"))?
                ));
                i += 2;
            }
            "--repo" => {
                repo = Some(
                    args.get(i + 1).ok_or_else(|| anyhow::anyhow!("--repo requires a URL"))?
                        .to_string()
                );
                i += 2;
            }
            "--branch" => {
                branch = Some(
                    args.get(i + 1).ok_or_else(|| anyhow::anyhow!("--branch requires a value"))?
                        .to_string()
                );
                i += 2;
            }
            other => anyhow::bail!("Unknown flag: '{}'\nUsage: {}", other, command.usage),
        }
    }

    if root.is_some() && repo.is_some() {
        anyhow::bail!("Provide --root (Manual) or --repo (Autopilot), not both.");
    }

    // Already registered
    if config.is_registered(name) {
        let entry = config.projects.iter().find(|e| e.name == *name).unwrap();
        let effective_root = entry.effective_root(data_dir);

        match entry.mode {
            ProjectMode::Manual => {
                let result = scaffold::scaffold_project(&effective_root, &entry.name)?;
                println!("{}", result.summary());
            }
            ProjectMode::Autopilot => {
                let repo = entry.repo.as_ref().unwrap();
                let branch = entry.branch.as_deref().unwrap_or("main");
                let outcome = git::sync_project(repo, branch, &effective_root)?;
                println!("{}", outcome.summary());
                if scaffold::is_scaffolded(&effective_root) {
                    println!("Knowledge base detected in clone.");
                } else {
                    println!(
                        "Note: upstream repo has no .lore/ scaffolding.\n\
                         A developer should scaffold this project locally and push."
                    );
                }
            }
        }
        return Ok(());
    }

    // Manual mode + not registered
    if let Some(root_path) = root {
        let result = scaffold::scaffold_project(&root_path, name)?;
        println!("{}", result.summary());
        let entry = ProjectEntry {
            name: name.to_string(),
            mode: ProjectMode::Manual,
            root: Some(root_path),
            repo: None,
            branch: None,
        };
        config.register_project(entry, config_path)?;
        println!("Registered project '{}'", name);
        return Ok(());
    }

    // Autopilot mode + not registered
    if let Some(repo_url) = repo {
        let branch_str = branch.as_deref().unwrap_or("main");
        let entry = ProjectEntry {
            name: name.to_string(),
            mode: ProjectMode::Autopilot,
            root: None,
            repo: Some(repo_url.clone()),
            branch: Some(branch_str.to_string()),
        };
        let effective_root = entry.effective_root(data_dir);
        let outcome = git::sync_project(&repo_url, branch_str, &effective_root)?;
        config.register_project(entry, config_path)?;
        println!("Registered project '{}'", name);
        println!("{}", outcome.summary());
        if scaffold::is_scaffolded(&effective_root) {
            println!("Knowledge base detected in clone.");
        } else {
            println!(
                "Note: upstream repo has no .lore/ scaffolding.\n\
                 A developer should scaffold this project locally and push."
            );
        }
        return Ok(());
    }

    // Neither provided + not registered
    anyhow::bail!(
        "Project '{}' is not registered. Provide --root <path> or --repo <url>. Usage: {}",
        name, command.usage
    );
}

pub fn handle_list_projects(config: &mut OrchestratorConfig, _args: &[&str], _config_path: &Path, _data_dir: &Path) -> anyhow::Result<()> {
    if config.projects.is_empty() {
        println!("No projects registered.");
        return Ok(());
    }

    println!("{:<20} {:<12} {}", "Name", "Mode", "Location");
    println!("{:<20} {:<12} {}", "----", "----", "--------");

    for project in &config.projects {
        let mode = match project.mode {
            crate::config::ProjectMode::Manual => "manual",
            crate::config::ProjectMode::Autopilot => "autopilot",
        };
        let location = match &project.root {
            Some(root) => root.display().to_string(),
            None => project.repo.clone().unwrap_or_default(),
        };
        println!("{:<20} {:<12} {}", project.name, mode, location);
    }

    Ok(())
}

pub fn handle_remove_project(config: &mut OrchestratorConfig, args: &[&str], config_path: &Path, _data_dir: &Path) -> anyhow::Result<()> {
    let command = COMMANDS.iter().find(|c| c.name == "remove-project").unwrap();

    let name = args.get(0).ok_or_else(|| anyhow::anyhow!("Usage: {}", command.usage))?;

    let before = config.projects.len();
    config.projects.retain(|p| p.name != **name);

    if config.projects.len() == before {
        anyhow::bail!("Project '{}' not found", name);
    }

    config.save(config_path)?;
    println!("Removed project '{}'", name);
    Ok(())
}

pub fn handle_sync_project(config: &mut OrchestratorConfig, args: &[&str], _config_path: &Path, data_dir: &Path) -> anyhow::Result<()> {
    let command = COMMANDS.iter().find(|c| c.name == "sync-project").unwrap();
    let name = args.get(0).ok_or_else(|| anyhow::anyhow!("Usage: {}", command.usage))?;
    let project = config.projects.iter()
        .find(|p| p.name == *name)
        .ok_or_else(|| anyhow::anyhow!("Project not found: {}", name))?;

    if project.mode == ProjectMode::Manual {
        anyhow::bail!("Nothing to sync. Project '{}' mode is 'Manual'", name);
    }

    let repo = project.repo.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Project '{}' is missing repo URL", name))?;
    let branch = project.branch.as_deref().unwrap_or("main");
    let effective_root = project.effective_root(data_dir);

    let outcome = git::sync_project(repo, branch, &effective_root)?;
    println!("{}", outcome.summary());
    if scaffold::is_scaffolded(&effective_root) {
        println!("Knowledge base detected.");
    } else {
        println!(
            "Note: upstream repo has no .lore/ scaffolding.\n\
             A developer should scaffold this project locally and push."
        );
    }
    Ok(())
}

pub fn handle_configure(_config: &mut OrchestratorConfig, args: &[&str], _config_path: &Path, data_dir: &Path) -> anyhow::Result<()> {
    if args.contains(&"--list") {
        crate::configure::configure_list(data_dir)
    } else if args.contains(&"--all") {
        crate::configure::configure_all(data_dir)
    } else if args.contains(&"--app") {
        let name = args.iter()
            .position(|a| *a == "--app")
            .and_then(|i| args.get(i + 1))
            .ok_or_else(|| anyhow::anyhow!(
                "--app requires a name (claude-desktop, claude-code, codex)"
            ))?;
        crate::configure::configure_app(name, data_dir)
    } else {
        crate::configure::configure_interactive(data_dir)
    }
}