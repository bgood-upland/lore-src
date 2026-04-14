mod commands;

use std::{io::{self, Write}, path::Path};
use anyhow::Result;

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorCode, ErrorData},
};
use crate::{OrchestratorServer, config::OrchestratorConfig, tools::graph::UpdateGraphSchemaParams};
use crate::tools::knowledge::{
    GatherProjectContextParams, InitProjectParams, SyncProjectParams,
    ListFilesParams, ListProjectsParams, ReadFileParams, WriteFileParams,
    UpdateSectionParams, RegisterFileParams, UnregisterFileParams,
    ReadSectionParams, SearchKnowledgeParams,
};
use crate::tools::skills::{
    ListSkillsParams, ReadSkillParams, ListSkillFilesParams, CreateSkillParams,
    WriteSkillFileParams, DeleteSkillParams, DeleteSkillFileParams, UpdateSkillSectionParams,
};
use crate::tools::graph::{
    CreateEntitiesParams, AddObservationsParams, DeleteEntitiesParams,
    DeleteObservationsParams, CreateRelationsParams, DeleteRelationsParams,
    SearchGraphParams, ReadGraphParams, ListEntitiesParams, ReadEntitiesParams,
    ValidateGraphParams,
};

pub fn run_cli(config: &mut OrchestratorConfig, config_path: &Path, data_dir: &Path) -> anyhow::Result<()> {
    println!("Lore CLI. Type 'help' for commands, 'exit' to quit.");

    let mut input = String::new();

    loop {
        print!("> ");

        io::stdout().flush()?;
        input.clear();

        let bytes_read = io::stdin().read_line(&mut input)?;
        if bytes_read == 0 {
            break;
        }

        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let command = parts[0];
        let args = &parts[1..];

        match command {
            "help" => {
                if args.is_empty() {
                    commands::print_help();
                } else {
                    commands::print_command_help(args[0]);
                }
            },
            "exit" | "quit" => break,
            _ => {
                if let Err(e) = commands::find_and_run(command, config, args, config_path, data_dir) {
                    println!("Error: {}", e);
                }
            }
        }
    }

    println!("Goodbye.");
    Ok(())
}

/// Test a tool using a CLI command
pub async fn run_test(server: OrchestratorServer, tool: &str, params_json: &str) -> Result<()> {
    println!("🔧 Tool:   {tool}");
    println!("📥 Params: {params_json}");
    println!("---");

    let start = std::time::Instant::now();
    let result = dispatch_tool(&server, tool, params_json).await;
    let elapsed = start.elapsed();

    match result {
        Ok(call_result) => {
            for content in &call_result.content {
                match serde_json::to_string_pretty(content) {
                    Ok(s) => println!("{s}"),
                    Err(_) => println!("{content:?}"),
                }
            }
            println!("---");
            println!("✅  OK  ({elapsed:.2?})");
        }
        Err(e) => {
            println!("❌  Error: {}", e.message);
            if let Some(data) = &e.data {
                println!("    Data:  {data}");
            }
            println!("---");
            println!("⏱   ({elapsed:.2?})");
            anyhow::bail!("Tool returned an error");
        }
    }

    Ok(())
}

/// Benchmark a tool's execution time from the CLI
pub async fn run_bench(
    server: OrchestratorServer,
    tool: &str,
    params_json: &str,
    iterations: u32,
) -> Result<()> {
    println!("⏱  Benchmarking '{tool}' × {iterations}");
    println!("---");

    let mut timings: Vec<std::time::Duration> = Vec::with_capacity(iterations as usize);

    for i in 1..=iterations {
        let start = std::time::Instant::now();
        let result = dispatch_tool(&server, tool, params_json).await;
        let elapsed = start.elapsed();

        if let Err(e) = result {
            anyhow::bail!("Run {i} failed: {}", e.message);
        }

        timings.push(elapsed);
    }

    timings.sort();

    let min = timings.first().unwrap();
    let max = timings.last().unwrap();
    let mean = timings.iter().sum::<std::time::Duration>() / iterations;
    let median = timings[timings.len() / 2];
    let p95 = timings[(timings.len() as f64 * 0.95) as usize];

    println!("  min:    {min:.2?}");
    println!("  median: {median:.2?}");
    println!("  p95:    {p95:.2?}");
    println!("  max:    {max:.2?}");
    println!("  mean:   {mean:.2?}");

    Ok(())
}

/// Deserializes params and calls the matching tool method directly on the server
pub(crate) async fn dispatch_tool(
    server: &OrchestratorServer,
    tool: &str,
    params_json: &str,
) -> Result<CallToolResult, ErrorData> {
    match tool {
        // ----------------------------------------
        // GENERAL
        // ----------------------------------------

        "gather_project_context" => {
            let p: GatherProjectContextParams = parse(params_json)?;
            server.gather_project_context(Parameters(p)).await
        }
        "init_project" => {
            let p: InitProjectParams = parse(params_json)?;
            server.init_project(Parameters(p)).await
        }
        "sync_project" => {
            let p: SyncProjectParams = parse(params_json)?;

            // Not an MCP tool on the server (test-only dispatch)
            // Find the project, sync it, return the outcome as text.
            let config = OrchestratorConfig::load(&server.config_path())
                .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;
            let entry = config.projects.iter()
                .find(|e| e.name == p.project)
                .ok_or_else(|| ErrorData::new(
                    ErrorCode::INVALID_PARAMS,
                    format!("Unknown project: {}", p.project),
                    None,
                ))?;
            match entry.mode {
                crate::config::ProjectMode::Manual => {
                    return Err(ErrorData::new(
                        ErrorCode::INVALID_PARAMS,
                        format!("Project '{}' is Manual mode — nothing to sync", p.project),
                        None,
                    ));
                }
                crate::config::ProjectMode::Autopilot => {
                    let repo = entry.repo.as_ref().unwrap();
                    let branch = entry.branch.as_deref().unwrap_or("main");
                    let outcome = crate::git::sync_project(repo, branch, &entry.effective_root(server.data_dir()))
                        .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;
                    Ok(CallToolResult::success(vec![
                        rmcp::model::Content::text(outcome.summary())
                    ]))
                }
            }
        }

        // ----------------------------------------
        // PROJECT KNOWLEDGE
        // ----------------------------------------

        "list_all_projects" => {
            let p: ListProjectsParams = parse(params_json)?;
            server.list_all_projects(Parameters(p)).await
        }
        "list_knowledge_files" => {
            let p: ListFilesParams = parse(params_json)?;
            server.list_knowledge_files(Parameters(p)).await
        }
        "read_knowledge_file" => {
            let p: ReadFileParams = parse(params_json)?;
            server.read_knowledge_file(Parameters(p)).await
        }
        "write_knowledge_file" => {
            let p: WriteFileParams = parse(params_json)?;
            server.write_knowledge_file(Parameters(p)).await
        }
        "update_knowledge_section" => {
            let p: UpdateSectionParams = parse(params_json)?;
            server.update_knowledge_section(Parameters(p)).await
        }
        "register_knowledge_file" => {
            let p: RegisterFileParams = parse(params_json)?;
            server.register_knowledge_file(Parameters(p)).await
        }
        "unregister_knowledge_file" => {
            let p: UnregisterFileParams = parse(params_json)?;
            server.unregister_knowledge_file(Parameters(p)).await
        }
        "search_knowledge" => {
            let p: SearchKnowledgeParams = parse(params_json)?;
            server.search_knowledge(Parameters(p)).await
        }
        "read_knowledge_section" => {
            let p: ReadSectionParams = parse(params_json)?;
            server.read_knowledge_section(Parameters(p)).await
        }

        // ----------------------------------------
        // SKILLS
        // ----------------------------------------

        "list_skills" => {
            let p: ListSkillsParams = parse(params_json)?;
            server.list_skills(Parameters(p)).await
        }
        "read_skill" => {
            let p: ReadSkillParams = parse(params_json)?;
            server.read_skill(Parameters(p)).await
        }
        "list_skill_files" => {
            let p: ListSkillFilesParams = parse(params_json)?;
            server.list_skill_files(Parameters(p)).await
        }
        "create_skill" => {
            let p: CreateSkillParams = parse(params_json)?;
            server.create_skill(Parameters(p)).await
        }
        "write_skill_file" => {
            let p: WriteSkillFileParams = parse(params_json)?;
            server.write_skill_file(Parameters(p)).await
        }
        "update_skill_section" => {
            let p: UpdateSkillSectionParams = parse(params_json)?;
            server.update_skill_section(Parameters(p)).await
        }
        "delete_skill" => {
            let p: DeleteSkillParams = parse(params_json)?;
            server.delete_skill(Parameters(p)).await
        }
        "delete_skill_file" => {
            let p: DeleteSkillFileParams = parse(params_json)?;
            server.delete_skill_file(Parameters(p)).await
        }

        // ----------------------------------------
        // KNOWLEDGE GRAPH
        // ----------------------------------------

        "create_entities" => {
            let p: CreateEntitiesParams = parse(params_json)?;
            server.create_entities(Parameters(p)).await
        }
        "add_observations" => {
            let p: AddObservationsParams = parse(params_json)?;
            server.add_observations(Parameters(p)).await
        }
        "delete_entities" => {
            let p: DeleteEntitiesParams = parse(params_json)?;
            server.delete_entities(Parameters(p)).await
        }
        "delete_observations" => {
            let p: DeleteObservationsParams = parse(params_json)?;
            server.delete_observations(Parameters(p)).await
        }
        "create_relations" => {
            let p: CreateRelationsParams = parse(params_json)?;
            server.create_relations(Parameters(p)).await
        }
        "delete_relations" => {
            let p: DeleteRelationsParams = parse(params_json)?;
            server.delete_relations(Parameters(p)).await
        }
        "search_graph" => {
            let p: SearchGraphParams = parse(params_json)?;
            server.search_graph(Parameters(p)).await
        }
        "read_graph" => {
            let p: ReadGraphParams = parse(params_json)?;
            server.read_graph(Parameters(p)).await
        }
        "list_entities" => {
            let p: ListEntitiesParams = parse(params_json)?;
            server.list_entities(Parameters(p)).await
        }
        "read_entities" => {
            let p: ReadEntitiesParams = parse(params_json)?;
            server.read_entities(Parameters(p)).await
        }
        "validate_graph" => {
            let p: ValidateGraphParams = parse(params_json)?;
            server.validate_graph(Parameters(p)).await
        }
        "update_graph_schema" => {
            let p: UpdateGraphSchemaParams = parse(params_json)?;
            server.update_graph_schema(Parameters(p)).await
        }

        // ----------------------------------------
        // Unknown tool handling
        // ----------------------------------------
        other => {
            let available = [
                // general
                "gather_project_context", "init_project", "sync-project",
                // knowledge
                "list_all_projects", "list_knowledge_files", "read_knowledge_file",
                "write_knowledge_file", "update_knowledge_section", "register_knowledge_file",
                "unregister_knowledge_file", "search_knowledge", "read_knowledge_section",
                // skills
                "list_skills", "read_skill", "list_skill_files", "create_skill",
                "write_skill_file", "update_skill_section", "delete_skill", "delete_skill_file",
                // graph
                "create_entities", "add_observations", "delete_entities", "delete_observations",
                "create_relations", "delete_relations", "search_graph", "read_graph",
                "list_entities", "read_entities", "validate_graph", "update_graph_schema",
            ];
            Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("Unknown tool: '{other}'\n\nAvailable tools:\n  {}", available.join("\n  ")),
                None,
            ))
        }
    }
}

/// Deserialize a JSON string into T
fn parse<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, ErrorData> {
    serde_json::from_str(json).map_err(|e| {
        ErrorData::new(
            ErrorCode::INVALID_PARAMS,
            format!("Failed to parse params: {e}"),
            None,
        )
    })
}