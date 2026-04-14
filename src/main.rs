mod tools;
mod knowledge;
mod skills;
mod graph;
mod resolver;
mod utils;
mod config;
mod markdown;
mod xml;
mod cli;
mod configure;
mod init;
mod scaffold;
mod git;
mod suggestions;

use std::path::{Path, PathBuf};

use anyhow::Result;
use rmcp::{
    ServerHandler,
    ServiceExt,
    handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{
        CallToolResult,
        ErrorCode,
        Implementation,
        ProtocolVersion,
        ServerCapabilities,
        ServerInfo,
    },
    tool,
    tool_handler,
    tool_router,
    transport::stdio,
};

use config::OrchestratorConfig;
use resolver::SharedResolver;

use crate::{config::{ProjectEntry, ProjectMode}, knowledge::store::{FileSummary, KnowledgeStore}, tools::knowledge::ListFileHeadingsParams};
use crate::tools::knowledge::{
    GatherProjectContextParams,
    InitProjectParams,
    ListFilesParams,
    ReadFileParams,
    WriteFileParams,
    UpdateSectionParams,
    RegisterFileParams,
    UnregisterFileParams,
    ReadSectionParams,
    SearchKnowledgeParams
};

use skills::store::SkillStore;
use tools::skills::{
    ListSkillsParams, ReadSkillParams,
    ListSkillFilesParams, CreateSkillParams, WriteSkillFileParams,
    DeleteSkillParams, DeleteSkillFileParams, UpdateSkillSectionParams
};
use skills::types::SkillScope;

use graph::store::GraphStore;
use crate::graph::types::ValidationWarning;
use tools::graph::{
    CreateEntitiesParams, AddObservationsParams,
    DeleteEntitiesParams, DeleteObservationsParams,
    CreateRelationsParams, DeleteRelationsParams,
    SearchGraphParams, ReadGraphParams, ListEntitiesParams, 
    ReadEntitiesParams, ValidateGraphParams, UpdateGraphSchemaParams
};

/// The MCP server struct.
#[derive(Clone)]
pub(crate) struct OrchestratorServer {
    knowledge: KnowledgeStore,
    skills: SkillStore,
    graph: GraphStore,
    resolver: SharedResolver,
    config_path: PathBuf,
    data_dir: PathBuf,
    tool_router: ToolRouter<Self>,
}

/// This impl is where MCP tool definitions live
#[tool_router]
impl OrchestratorServer {
    pub(crate) fn new(knowledge: KnowledgeStore, skills: SkillStore, graph: GraphStore, resolver: SharedResolver, config_path: PathBuf, data_dir: PathBuf) -> Self {
        Self {
            knowledge,
            skills,
            graph,
            resolver,
            config_path,
            data_dir,
            tool_router: Self::tool_router(),
        }
    }

    // ─── Response helpers ────────────────────────────────────────────

    fn to_error(e: impl std::fmt::Display) -> rmcp::ErrorData {
        rmcp::ErrorData::new(
            ErrorCode::INTERNAL_ERROR,
            e.to_string(),
            None,
        )
    }

    fn text_result(text: impl Into<String>) -> Result<CallToolResult, rmcp::ErrorData> {
        Ok(CallToolResult::success(vec![rmcp::model::Content::text(text.into())]))
    }

    fn json_result<T: serde::Serialize>(val: &T) -> Result<CallToolResult, rmcp::ErrorData> {
        let json = serde_json::to_string_pretty(val).map_err(Self::to_error)?;
        Ok(CallToolResult::success(vec![rmcp::model::Content::text(json)]))
    }

    /// For create operations that return the created data + schema warnings.
    fn json_result_with_warnings<T: serde::Serialize>(
        data: &T,
        warnings: &[ValidationWarning],
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let json = serde_json::to_string_pretty(data).map_err(Self::to_error)?;
        if warnings.is_empty() {
            return Self::text_result(json);
        }
        let warnings_block = xml::format_warnings_block(warnings);
        Self::text_result(format!("{json}\n\n{warnings_block}"))
    }

    /// For mutations that return a success message + optional schema warnings.
    fn success_with_warnings(
        message: impl Into<String>,
        warnings: &[ValidationWarning],
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let msg = message.into();
        if warnings.is_empty() {
            return Self::text_result(msg);
        }
        let warnings_block = xml::format_warnings_block(warnings);
        Self::text_result(format!("{msg}\n\n{warnings_block}"))
    }

    /// For knowledge writes that return a success message + optional size budget warning.
    fn success_with_optional_warning(
        message: impl Into<String>,
        warning: Option<String>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let msg = message.into();
        match warning {
            Some(w) => Self::text_result(format!("{msg}\n\n<warning>{w}</warning>")),
            None => Self::text_result(msg),
        }
    }

    pub(crate) fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub(crate) fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    // ==================
    // GENERAL TOOLS
    // ==================

    fn gather_knowledge_file_context(&self, project: &str, file: &FileSummary) -> String {
        let headings_result = self.knowledge.list_file_headings_with_size(project, &file.key);

        let (size_attr, headings_block) = match headings_result {
            Ok(mut headings) => {
                let total = headings.iter()
                    .position(|(heading, _)| heading == "__total__")
                    .map(|i| headings.remove(i).1);
                let size_attr = match total {
                    Some(size) => format!(" size=\"{}\"", size),
                    None => String::new(),
                };
                let block = if headings.is_empty() {
                    xml::wrap_xml("headings", "No headings found")
                } else {
                    let formatted: Vec<String> = headings.iter()
                        .map(|h| format!("{} ({} chars)", h.0, h.1))
                        .collect();
                    xml::wrap_xml("headings", &formatted.join("\n"))
                };
                (size_attr, block)
            }
            Err(_) => {
                (String::new(), xml::wrap_xml("headings", "Could not read file headings"))
            }
        };

        let when_to_read_attr = match &file.when_to_read {
            Some(w) => format!(" when_to_read=\"{}\"", w),
            None => String::new(),
        };

        format!(
            "<file key=\"{}\"{size_attr}{when_to_read_attr}>\n{}\n{headings_block}\n</file>",
            file.key,
            file.summary.trim()
        )
    }

    #[tool(
        description = "**Call this first at the start of every task.** Returns the project's knowledge file index (keys, summaries, section headings with sizes), skill index, and graph entity summary. Set include_overview: true on the first call of a conversation to load the full project-instructions document. On follow-up tasks, use include_overview: false to refresh the index without re-loading instructions already in context. After results return, check the size attribute on each <file> — files over 15K characters should generally be accessed via read_knowledge_section, not read_knowledge_file. Use the when_to_read attribute to decide which files are relevant to the current task. Do NOT call this more than once per conversation unless files were registered or unregistered.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn gather_project_context(
        &self,
        params: Parameters<GatherProjectContextParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = &params.0;

        // Knowledge Files
        let files = self.knowledge
            .list_files(&p.project)
            .map_err(Self::to_error)?;
        let files_xml = files.iter()
            .map(|f| self.gather_knowledge_file_context(&p.project, f))
            .collect::<Vec<String>>()
            .join("\n");
        let files_block = format!("<knowledge_files>\n{files_xml}\n</knowledge_files>");

        // Skills
        let skills = self.skills
            .list_skill_summaries(&p.project)
            .map_err(Self::to_error)?;
        let skills_xml = skills.iter()
            .map(|s| xml::wrap_xml(
                &format!("skill key=\"{}\" name=\"{}\" scope=\"{}\"", s.skill_key, s.name, s.scope),
                s.description.trim(),
            ))
            .collect::<Vec<String>>()
            .join("\n");
        let skills_block = format!("<skills>\n{skills_xml}\n</skills>");

        // Graph
        let graph = self.graph
            .read_graph(&p.project)
            .map_err(Self::to_error)?;
        let (index_entities, non_index_entities): (Vec<_>, Vec<_>) = graph.entities
            .iter()
            .partition(|e| e.entity_type == "Index");
        let index_xml: Vec<String> = index_entities.iter()
            .map(|e| {
                let member_text = e.observations.join(", ");
                format!(
                    "<index name=\"{}\" entities=\"{}\">\n{}\n</index>",
                    e.name,
                    e.observations.len(),
                    member_text
                )
            })
            .collect();
        let non_indexed_block = if non_index_entities.is_empty() {
            String::new()
        } else {
            let names: Vec<&str> = non_index_entities.iter().map(|e| e.name.as_str()).collect();
            format!(
                "<non_indexed count=\"{}\">\n{}\n</non_indexed>",
                non_index_entities.len(),
                names.join(", ")
            )
        };
        let mut graph_parts = index_xml;
        if !non_indexed_block.is_empty() {
            graph_parts.push(non_indexed_block);
        }
        let entities_block = if graph_parts.is_empty() {
            "<graph_summary>\n</graph_summary>".to_string()
        } else {
            format!("<graph_summary>\n{}\n</graph_summary>", graph_parts.join("\n"))
        };

        // Graph Schema
        let schema_block = match self.graph.get_schema(&p.project).map_err(Self::to_error)? {
            Some(schema) => xml::format_schema_xml(&schema),
            None => "<graph_schema />".to_string(),
        };

        // Project Instructions
        let overview = if p.include_overview.unwrap_or(false) {
            let content = self.knowledge
                .read_file(&p.project, "project-instructions")
                .map_err(Self::to_error)?;
            format!("<project_instructions>\n{content}\n</project_instructions>")
        } else {
            String::new()
        };

        let mut sections = vec![files_block, skills_block, entities_block, schema_block];
        if !overview.is_empty() {
            sections.push(overview);
        }
        let suggested = suggestions::get_suggested_next_xml("gather_project_context", "");
        if !suggested.is_empty() {
            sections.push(suggested);
        }
        Self::text_result(sections.join("\n\n"))
    }

    #[tool(
        description = "Initialize a project for use with the lore. For Manual mode (developer with a local checkout): scaffolds the .lore/ knowledge base directory structure. For Autopilot mode (read-only clone): clones the repo, then registers the project. If the project is not yet registered, provide either root (Manual) or repo (Autopilot) to register it automatically. If already registered, triggers a scaffold (Manual) or sync (Autopilot) using the known configuration.",
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn init_project(
        &self,
        params: Parameters<InitProjectParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;

        let mut config = OrchestratorConfig::load(&self.config_path).map_err(Self::to_error)?;

        // Already registered
        if config.is_registered(&p.project) {
            let entry = config.projects.iter().find(|e| e.name == p.project).unwrap();
            let root = entry.effective_root(&self.data_dir);
            return match entry.mode {
                ProjectMode::Manual => {
                    let result = scaffold::scaffold_project(&root, &p.project)
                        .map_err(Self::to_error)?;
                    Self::text_result(result.summary())
                }
                ProjectMode::Autopilot => {
                    let repo = entry.repo.as_ref().unwrap();
                    let branch = entry.branch.as_deref().unwrap_or("main");
                    let outcome = git::sync_project(repo, branch, &root)
                        .map_err(Self::to_error)?;
                    let mut response = outcome.summary();
                    if scaffold::is_scaffolded(&root) {
                        response.push_str("\nKnowledge base detected in clone.");
                    } else {
                        response.push_str(
                            "\nNote: upstream repo has no .lore/ scaffolding. \
                            A developer should scaffold locally and push."
                        );
                    }
                    Self::text_result(response)
                }
            };
        }

        // Not registered: determine mode from params
        if p.root.is_some() && p.repo.is_some() {
            return Err(Self::to_error(
                "Provide either root (Manual mode) or repo (Autopilot mode), not both."
            ));
        }

        if let Some(ref root_str) = p.root {
            // Manual mode register + scaffold
            let root = PathBuf::from(root_str);
            let entry = ProjectEntry {
                name: p.project.clone(),
                mode: ProjectMode::Manual,
                root: Some(root.clone()),
                repo: None,
                branch: None,
            };
            let entry_clone = entry.clone();
            config.register_project(entry, &self.config_path).map_err(Self::to_error)?;
            self.resolver.write().unwrap().register(entry_clone);
            let result = scaffold::scaffold_project(&root, &p.project)
                .map_err(Self::to_error)?;
            let mut response = result.summary();
            response.push_str(
                "\nNew project registered"
            );
            Self::text_result(response)

        } else if let Some(ref repo_url) = p.repo {
            // Autopilot mode clone then register
            let branch_str = p.branch.as_deref().unwrap_or("main");
            let entry = ProjectEntry {
                name: p.project.clone(),
                mode: ProjectMode::Autopilot,
                root: None,
                repo: Some(repo_url.clone()),
                branch: Some(branch_str.to_string()),
            };
            let effective_root = entry.effective_root(&self.data_dir);
            let outcome = git::sync_project(repo_url, branch_str, &effective_root)
                .map_err(Self::to_error)?;
            // Clone succeeded — safe to register now
            let entry_clone = entry.clone();
            config.register_project(entry, &self.config_path).map_err(Self::to_error)?;
            self.resolver.write().unwrap().register(entry_clone);

            let mut response = outcome.summary();
            if scaffold::is_scaffolded(&effective_root) {
                response.push_str("\nKnowledge base detected in clone.");
            } else {
                response.push_str(
                    "\nNote: upstream repo has no .lore/ scaffolding. \
                    A developer should scaffold locally and push."
                );
            }
            response.push_str(
                "\nNew project registered."
            );
            Self::text_result(response)

        } else {
            Err(Self::to_error(format!(
                "Project '{}' is not registered. Provide root (Manual) or repo (Autopilot) to register it.",
                p.project
            )))
        }
    }


    // ==================
    // PROJECT KNOWLEDGE
    // ==================

    #[tool(
        description = "List all registered projects and their root paths. Use at the start of a conversation when you need to identify which project the user is working in, or when the user references a project by a name you haven't seen. Rarely needed after the first call — gather_project_context covers a specific project's full index. Do NOT call this repeatedly within a conversation — the project list does not change during a session.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn list_all_projects(
        &self,
        _params: Parameters<tools::knowledge::ListProjectsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let resolver = self.resolver.read().unwrap();
        let summaries: Vec<crate::knowledge::store::ProjectSummary> = resolver.list()
            .iter()
            .map(|p| crate::knowledge::store::ProjectSummary {
                name: p.name.clone(),
                root: p.effective_root(&self.data_dir).display().to_string(),
            })
            .collect();
        Self::json_result(&summaries)
    }

    #[tool(
        description = "List all knowledge files registered in a project's manifest, with keys, paths, and summaries. Rarely needed directly — gather_project_context already includes this index with section headings and sizes. Use only to refresh the file list mid-conversation after a register or unregister operation. Do NOT use as a substitute for gather_project_context at the start of a task — it returns less information (no headings, no sizes, no skills, no graph summary).",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn list_knowledge_files(
        &self,
        params: Parameters<ListFilesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let files = self.knowledge.list_files(&p.project)
            .map_err(Self::to_error)?;
        Self::text_result(xml::format_file_list_xml(&files, &p.project))
    }

    #[tool(
        description = "Read an entire knowledge file by its manifest key. Loads the full file into context. Check the file's size attribute from gather_project_context before calling — files over 15K characters consume significant context budget. Prefer read_knowledge_section when you only need 1–2 sections. Use this when you need comprehensive context across multiple sections of the same file, or when loading a reference doc for the first time on a complex task. Do NOT use just to check one fact or convention — read_knowledge_section or search_knowledge is cheaper. Common mistake: reading a 40K-character file to find a single convention that could have been found with search_knowledge or read_knowledge_section in a fraction of the context.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn read_knowledge_file(
        &self,
        params: Parameters<ReadFileParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let file = self.knowledge.read_file(&p.project, &p.file_key)
            .map_err(Self::to_error)?;
        let suggested = suggestions::get_suggested_next_xml("read_knowledge_file", "");
        Self::text_result(format!("{file}\n\n{suggested}"))
    }

    #[tool(
        description = "Overwrite an entire knowledge file with new content. **This replaces the entire file** — if only one section needs updating, use update_knowledge_section instead to avoid accidentally overwriting unrelated sections. You MUST read the file first (via read_knowledge_file or read_knowledge_section) before writing. Do NOT write to a file you haven't read in this session. After writing, consider re-reading the file or section to verify the change is correct. Common mistake: overwriting a file without reading it first, losing content in sections you didn't intend to change.",
        annotations(destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn write_knowledge_file(
        &self,
        params: Parameters<WriteFileParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let warning = self.knowledge.write_file(&p.project, &p.file_key, &p.content)
            .map_err(Self::to_error)?;
        Self::success_with_optional_warning(
            format!("Wrote file '{}'", p.file_key),
            warning,
        )
    }

    #[tool(
        description = "Replace the content under a specific markdown heading, preserving everything outside that section. **This is the preferred write operation** — safer than write_knowledge_file and more context efficient. The heading parameter must include the # prefix (e.g., ## Architecture). You MUST read the section or file before updating — do NOT write blind. The new content replaces everything between the heading and the next heading at the same or higher level. The heading line itself is preserved. After updating, consider calling read_knowledge_section on the same heading to verify the result. Common mistake: updating a section without reading it first, causing the new content to conflict with assumptions about the surrounding document.",
        annotations(destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn update_knowledge_section(
        &self,
        params: Parameters<UpdateSectionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let warning = self.knowledge.update_section(&p.project, &p.file_key, &p.heading, &p.content)
            .map_err(Self::to_error)?;
        Self::success_with_optional_warning(
            format!("Updated section '{}' in '{}'", p.heading, p.file_key),
            warning,
        )
    }

    #[tool(
        description = "Add a new file entry to a project's knowledge manifest. Use when creating a new reference doc that should be discoverable through gather_project_context. The file path must be relative to the project root. This only registers the file in the manifest — it does not create the file on disk. You must write the file separately via write_knowledge_file after registering it. Do NOT register files that already exist in the manifest — use list_knowledge_files or gather_project_context to check first. Common mistake: registering a file but forgetting to write its content, leaving a manifest entry pointing to a nonexistent file.",
        annotations(destructive_hint = false, idempotent_hint = false, open_world_hint = false)
    )]
    async fn register_knowledge_file(
        &self,
        params: Parameters<RegisterFileParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        self.knowledge.register_file(&p.project, &p.key, &p.path, &p.summary)
            .map_err(Self::to_error)?;
        Self::text_result(format!("Registered file '{}'", p.key))
    }

    #[tool(
        description = "Remove a file entry from a project's knowledge manifest. Use when a reference doc is no longer relevant and should stop appearing in gather_project_context results. This only removes the manifest entry — it does not delete the file from disk. Use when retiring or consolidating documentation. Do NOT use to temporarily hide a file — once unregistered, it will not appear in any tool results until re-registered. Verify the file key with gather_project_context before calling.",
        annotations(destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn unregister_knowledge_file(
        &self,
        params: Parameters<UnregisterFileParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        self.knowledge
            .unregister_file(&p.project, &p.file_key)
            .map_err(Self::to_error)?;
        Self::text_result(format!("Unregistered file '{}'", p.file_key))
    }

    #[tool(
        description = "Search for text across all registered knowledge files. Returns matching file keys, section headings, and line snippets grouped by file. The search is **case-insensitive substring matching — not semantic**. Use 1–3 word queries for best results; long phrases reduce matches. If a query returns no results, try a single distinctive keyword. Section headings are included in results — use them to follow up with read_knowledge_section for targeted access rather than loading the full file. Use this **before asking the user** where something is documented. Do NOT use when you already know the file key and heading — call read_knowledge_section directly. Do NOT use multi-word phrases when a single keyword would be more distinctive. Common mistake: searching with long queries like 'authentication gateway pattern' when 'gateway' alone would match.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn search_knowledge(
        &self,
        params: Parameters<SearchKnowledgeParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let results = self.knowledge
            .search_files(&p.project, &p.query)
            .map_err(Self::to_error)?;
        if results.is_empty() {
            let suggested = suggestions::get_suggested_next_xml("search_knowledge", "no_results");
            return Self::text_result(format!(
                "No matching files found for query: \"{}\"\n\n{suggested}", p.query
            ));
        }
        let results_xml = xml::format_search_results_xml(&results, &p.project, &p.query);
        let suggested = suggestions::get_suggested_next_xml("search_knowledge", "");
        Self::text_result(format!("{results_xml}\n\n{suggested}"))
    }

    #[tool(
        description = "Read a single markdown section by heading without loading the full file. **This is the preferred read operation** — use it whenever you need 1–2 specific sections rather than a full file. The heading parameter must include the # prefix exactly as it appears (e.g., ## Architecture, not Architecture). Use gather_project_context first to see available section headings and their sizes. Multiple section reads from different files are cheaper than one full file read. Issue parallel calls when you need sections from multiple files. Common mistake: not including the # prefix in the heading parameter, causing a 'heading not found' error.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn read_knowledge_section(
        &self,
        params: Parameters<ReadSectionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let section = self.knowledge
            .read_section(&p.project, &p.file_key, &p.heading)
            .map_err(Self::to_error)?;
        let suggested = suggestions::get_suggested_next_xml("read_knowledge_section", "");
        Self::text_result(format!("{section}\n\n{suggested}"))
    }

    #[tool(
        description = "List all markdown section headings in a knowledge file, with character counts per section. Use before read_knowledge_section when you know which file is relevant but not which heading to target. Cheaper than read_knowledge_file when you only need the structure. Do NOT use this before every read — if gather_project_context already returned headings for the file, use those.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn list_file_headings(
        &self,
        params: Parameters<ListFileHeadingsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let headings = self.knowledge
            .list_file_headings_with_size(&p.project, &p.file_key)
            .map_err(Self::to_error)?;
        
        // Strip the __total__ sentinel entry before formatting
        let headings: Vec<_> = headings.into_iter()
            .filter(|(h, _)| h != "__total__")
            .collect();
        
        let inner = headings.iter()
            .map(|(h, size)| format!("{} ({} chars)", h, size))
            .collect::<Vec<_>>()
            .join("\n");
        
        Self::text_result(xml::wrap_xml(
            &format!("headings project=\"{}\" file=\"{}\" count=\"{}\"", p.project, p.file_key, headings.len()),
            &inner,
        ))
    }


    // ==================
    // SKILLS
    // ==================

    #[tool(
        description = "List all skills available to a project (project-scoped and global) with names and trigger descriptions. Rarely needed directly — gather_project_context already includes this index. Use only to refresh the skill list mid-conversation after creating or deleting a skill. Do NOT use as a substitute for gather_project_context at the start of a task.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn list_skills(
        &self,
        params: Parameters<ListSkillsParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let summaries = self.skills
            .list_skill_summaries(&params.0.project)
            .map_err(Self::to_error)?;
        Self::text_result(xml::format_skill_list_xml(&summaries, &params.0.project))
    }

    #[tool(
        description = "Read a skill's SKILL.md file (default), or a specific file within the skill directory if file_path is provided. Use when a task matches a skill's trigger description from gather_project_context — read SKILL.md first for workflow instructions, then follow its guidance. When a skill's SKILL.md references files in a references/ subdirectory, call list_skill_files to discover them, then read the specific reference file relevant to the current task. Do NOT read all reference files — read only what the task requires. Do NOT skip reading a matching skill — skills contain task-specific procedures that improve output quality.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn read_skill(
        &self,
        params: Parameters<ReadSkillParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let content = self.skills
            .read_skill_file(&p.project, &p.skill_key, p.file_path.as_deref())
            .map_err(Self::to_error)?;
        Self::text_result(content)
    }

    #[tool(
        description = "List all files within a skill directory as relative paths and sizes. Use after reading a skill's SKILL.md when it references supplementary files (templates, examples, schemas) and you need to discover what's available. Not needed if SKILL.md is self-contained with no references to other files. Do NOT read every listed file — identify the one relevant to the current task and read only that.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn list_skill_files(
        &self,
        params: Parameters<ListSkillFilesParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let files = self.skills
            .list_skill_files(&p.project, &p.skill_key)
            .map_err(Self::to_error)?;
        Self::text_result(xml::format_skill_files_xml(&files, &p.skill_key))
    }

    #[tool(
        description = "Create a new skill directory with a SKILL.md file. Content must include valid YAML frontmatter with 'name' and 'description' fields. Use when codifying a repeatable workflow, pattern, or set of instructions that should be followed for specific task types. Defaults to project scope — set scope to 'global' for skills shared across all projects. Read existing skills first (via list_skills or gather_project_context) to avoid duplicating an existing skill's scope. The 'description' in frontmatter is the trigger text shown in gather_project_context — write it to clearly describe when this skill should be used. Do NOT create skills for one-off tasks — skills are for repeatable patterns.",
        annotations(destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn create_skill(
        &self,
        params: Parameters<CreateSkillParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let scope = SkillScope::from(p.scope);
        self.skills
            .create_skill(&p.project, &p.skill_name, &p.content, scope)
            .map_err(Self::to_error)?;
        Self::text_result(format!("Created skill '{}'", p.skill_name))
    }

    #[tool(
        description = "Create or overwrite a file within an existing skill directory. Intermediate directories are created automatically. Use for adding reference files, templates, or examples alongside a skill's SKILL.md. To update just one section of an existing file, prefer update_skill_section. You MUST read the file first if it already exists — do not overwrite without checking current content. Do NOT use this to modify SKILL.md — use update_skill_section for targeted changes or write the full content through create_skill for initial setup.",
        annotations(destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn write_skill_file(
        &self,
        params: Parameters<WriteSkillFileParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        self.skills
            .write_skill_file(&p.project, &p.skill_key, &p.file_path, &p.content)
            .map_err(Self::to_error)?;
        Self::text_result(format!("Wrote '{}' in skill '{}'", p.file_path, p.skill_key))
    }

    #[tool(
        description = "Replace content under a specific markdown heading in a skill file, preserving the rest of the document. Defaults to SKILL.md if no file_path is provided. **Prefer this over write_skill_file** when only one section needs updating — it's safer and preserves surrounding content. The heading must include the # prefix (e.g. ## Usage). You MUST read the section or file before updating. Common mistake: updating a section without reading it first, causing the new content to conflict with assumptions about adjacent sections.",
        annotations(destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn update_skill_section(
        &self,
        params: Parameters<UpdateSkillSectionParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        self.skills
            .update_skill_section(&p.project, &p.skill_key, p.file_path.as_deref(), &p.heading, &p.content)
            .map_err(Self::to_error)?;
        Self::text_result(format!("Updated section '{}' in skill '{}'", p.heading, p.skill_key))
    }

    #[tool(
        description = "Delete an entire skill directory and all its contents. This is irreversible — the skill, its SKILL.md, and all reference files are permanently removed. Use only when a skill is genuinely no longer needed. To remove a single reference file from a skill, use delete_skill_file instead. Verify the skill key and scope before calling — there is no undo. Do NOT delete a skill just because it needs updating — use update_skill_section or write_skill_file to modify it in place.",
        annotations(destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn delete_skill(
        &self,
        params: Parameters<DeleteSkillParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let scope = SkillScope::from(p.scope);
        self.skills
            .delete_skill(&p.project, &p.skill_key, scope)
            .map_err(Self::to_error)?;
        Self::text_result(format!("Deleted skill '{}'", p.skill_key))
    }

    #[tool(
        description = "Delete a single file within a skill directory. Cannot delete SKILL.md — use delete_skill to remove the entire skill. Use when cleaning up outdated reference files or examples without removing the skill itself. Verify the file path with list_skill_files before calling. Do NOT use this to remove SKILL.md — the operation will be rejected.",
        annotations(destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn delete_skill_file(
        &self,
        params: Parameters<DeleteSkillFileParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        self.skills
            .delete_skill_file(&p.project, &p.skill_key, &p.file_path)
            .map_err(Self::to_error)?;
        Self::text_result(format!("Deleted '{}' from skill '{}'", p.file_path, p.skill_key))
    }


    // ==================
    // KNOWLEDGE GRAPH
    // ==================

    #[tool(
        description = "Create new entities in the knowledge graph. Skips any entity whose name already exists (no duplicates). Use to record new architectural components, decisions, patterns, or concepts discovered during the session. Entity names should be PascalCase for components/composables/types, or Index:DomainName for index nodes. Each observation should be a single atomic fact, not a dump of multiple facts. Use tag prefixes for categorization: [file], [purpose], [api], [gotcha], [decision], [convention]. Do NOT create entities for concepts that can be easily re-derived from the codebase — only record knowledge that is hard to rediscover.",
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn create_entities(&self, params: Parameters<CreateEntitiesParams>) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let result = self.graph.create_entities(&p.project, p.entities)
            .map_err(Self::to_error)?;
        let has_type_warning = result.warnings.iter().any(|w| w.message.contains("unknown type"));
        let has_tag_warning = result.warnings.iter().any(|w| w.message.contains("unknown tag"));
        let has_missing_tag = result.warnings.iter().any(|w| w.message.contains("missing a [tag]"));
        let context = if has_type_warning {
            "type_warning"
        } else if has_tag_warning {
            "tag_warning"
        } else if has_missing_tag {
            "missing_tag"
        } else {
            ""
        };
        let suggested = suggestions::get_suggested_next_xml("create_entities", context);
        let xml = xml::format_entities_xml(&result.data, &p.project);
        let warnings_block = if result.warnings.is_empty() {
            String::new()
        } else {
            format!("\n\n{}", xml::format_warnings_block(&result.warnings))
        };
        let suggestions_block = if suggested.is_empty() { String::new() } else { format!("\n\n{suggested}") };
        Self::text_result(format!("{xml}{warnings_block}{suggestions_block}"))
    }

    #[tool(
        description = "Append observations to existing entities. Use to record new facts, decisions, or gotchas about entities that already exist. Silently skips entity names that don't exist — call list_entities or search_graph first if unsure. Each observation should be a single, atomic fact. Use tag prefixes: [file] for source locations, [purpose] for descriptions, [api] for interface details, [gotcha] for pitfalls, [decision] for architectural choices, [convention] for coding standards. Do NOT add verbose API signatures as observations — these belong in documentation. Do NOT duplicate information already present in knowledge files.",
        annotations(destructive_hint = false, idempotent_hint = false, open_world_hint = false)
    )]
    async fn add_observations(&self, params: Parameters<AddObservationsParams>) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let entity_names: Vec<&str> = p.observations.iter().map(|o| o.entity_name.as_str()).collect();
        let summary = entity_names.join(", ");
        let result = self.graph.add_observations(&p.project, p.observations)
            .map_err(Self::to_error)?;
        let context = if result.warnings.iter().any(|w| w.message.contains("does not exist")) {
            "missing_entity"
        } else if result.warnings.iter().any(|w| w.message.contains("unknown tag")) {
            "schema_warning"
        } else {
            ""
        };
        let suggested = suggestions::get_suggested_next_xml("add_observations", context);
        let msg = format!("Added {} observation(s) to: {summary}", result.data);
        let warnings_block = if result.warnings.is_empty() {
            String::new()
        } else {
            format!("\n\n{}", xml::format_warnings_block(&result.warnings))
        };
        let suggestions_block = if suggested.is_empty() { String::new() } else { format!("\n\n{suggested}") };
        Self::text_result(format!("{msg}{warnings_block}{suggestions_block}"))
    }

    #[tool(
        description = "Delete entities by name from the knowledge graph. Also removes any relations that reference the deleted entities. Use when an entity is no longer relevant — e.g., a component was removed from the codebase, or an entity was created in error. Read the entity first to confirm it's the right one before deleting. Do NOT delete entities just because they have stale observations — use delete_observations to clean up individual facts instead.",
        annotations(destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn delete_entities(&self, params: Parameters<DeleteEntitiesParams>)
        -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let summary = p.entity_names.join(", ");
        let count = p.entity_names.len();
        self.graph.delete_entities(&p.project, p.entity_names)
            .map_err(Self::to_error)?;
        Self::text_result(format!("Deleted {count} entity(ies): {summary}"))
    }

    #[tool(
        description = "Remove specific observations from entities in the knowledge graph. Use to correct outdated or inaccurate facts on an entity without deleting the entity itself. Read the entity first (via read_entities) to see current observations and identify exactly which ones to remove. The observation text must match exactly. Do NOT use this to remove all observations from an entity — if the entity itself is obsolete, use delete_entities instead.",
        annotations(destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn delete_observations(&self, params: Parameters<DeleteObservationsParams>)
        -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let all_names: Vec<String> = p.deletions.iter().map(|d| d.entity_name.clone()).collect();
        let found = self.graph.delete_observations(&p.project, p.deletions)
            .map_err(Self::to_error)?;
        if found.is_empty() {
            Self::text_result(format!(
                "Deleted 0 observations — no matching entities found: {}",
                all_names.join(", ")
            ))
        } else {
            Self::text_result(format!("Deleted observations from: {}", found.join(", ")))
        }
    }

    #[tool(
        description = "Create directed relations between entities in the knowledge graph. Skips duplicate relations. Both the 'from' and 'to' entities should already exist — relations referencing nonexistent entities are rejected. Use to record how entities connect: dependencies, composition, inheritance, usage patterns. Relation types should use snake_case (e.g. depends_on, contains, used_by). Read relevant entities first to understand the current relation graph before adding new connections. Do NOT create redundant relations — check existing relations on the entities first via read_entities.",
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn create_relations(&self, params: Parameters<CreateRelationsParams>) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let result = self.graph.create_relations(&p.project, p.relations)
            .map_err(Self::to_error)?;
        let context = if result.warnings.iter().any(|w| w.message.contains("unknown type")) {
            "schema_warning"
        } else {
            ""
        };
        let suggested = suggestions::get_suggested_next_xml("create_relations", context);
        let xml = xml::format_relations_xml(&result.data, &p.project);
        let warnings_block = if result.warnings.is_empty() {
            String::new()
        } else {
            format!("\n\n{}", xml::format_warnings_block(&result.warnings))
        };
        let suggestions_block = if suggested.is_empty() { String::new() } else { format!("\n\n{suggested}") };
        Self::text_result(format!("{xml}{warnings_block}{suggestions_block}"))
    }

    #[tool(
        description = "Remove specific relations from the knowledge graph. Use when a relationship between entities is no longer accurate — e.g., a dependency was removed, or a relation was created in error. The from, to, and relation_type must all match exactly. Read the entities involved first (via read_entities) to verify the relation exists before attempting deletion.",
        annotations(destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn delete_relations(&self, params: Parameters<DeleteRelationsParams>)
        -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let removed = self.graph.delete_relations(&p.project, p.relations)
            .map_err(Self::to_error)?;
        Self::text_result(format!("Deleted {removed} relation(s)"))
    }

    #[tool(
        description = "Search the knowledge graph by case-insensitive substring match. Use 1–2 word queries, full phrases return nothing. Matches against entity names, types, and observation text. For known entity names, prefer read_entities for exact lookup with no false negatives. For domain exploration, search for the relevant Index entity first (e.g., search 'Index:Auth' or 'Index:API') — Index entities list all entities in a domain. Observations use tag prefixes like [file], [purpose], [api], [gotcha], [decision], [convention] — you can search for these tags to find specific observation types (e.g., search 'gotcha' to find all gotchas). Do NOT use long phrases. Do NOT use this when you know the exact entity name — call read_entities directly.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn search_graph(&self, params: Parameters<SearchGraphParams>)
        -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let result = self.graph.search_graph(&p.project, &p.query)
            .map_err(Self::to_error)?;
        let xml_output = xml::format_graph_result_xml(&result, &p.project, Some(&p.query));
        let context = if result.entities.is_empty() { "no_results" } else { "" };
        let suggested = suggestions::get_suggested_next_xml("search_graph", context);
        Self::text_result(format!("{xml_output}\n\n{suggested}"))
    }

    #[tool(
        description = "Return the complete knowledge graph for a project — all entities, observations, and relations. **Use sparingly** — this can be very large and consumes significant context. Only appropriate when you need a holistic view: auditing the full graph, cross-cutting analysis, or generating a project-wide summary. For targeted access, prefer search_graph to find entities by keyword, or read_entities to load specific entities by name. Do NOT use this for routine lookups — it loads everything regardless of relevance. Common mistake: reading the full graph to find one entity when search_graph with a keyword would return it instantly.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn read_graph(&self, params: Parameters<ReadGraphParams>)
        -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let result = self.graph.read_graph(&p.project)
            .map_err(Self::to_error)?;
        let xml_output = xml::format_graph_result_xml(&result, &p.project, None);
        Self::text_result(xml_output)
    }

    #[tool(
        description = "List the name and type of every entity in a project's knowledge graph. Lightweight — returns only names and types, no observations or relations. Use to discover what exists in the graph before deciding which entities to read in full. Prefer this over read_graph when you just need an inventory. For targeted lookup by keyword, use search_graph instead. Do NOT use this when you already know the entity names you need — call read_entities directly.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn list_entities(&self, params: Parameters<ListEntitiesParams>)
        -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let result = self.graph.list_entities(&p.project)
            .map_err(Self::to_error)?;
        let xml_output = xml::format_entity_list_xml(&result, &p.project);
        Self::text_result(xml_output)
    }

    #[tool(
        description = "Read full entities by exact name, returning all observations and relations involving those entities. Use when you know which entities are relevant and need their details. Observations may use tag prefixes: [file] (source location), [purpose] (what it does), [api] (interface), [gotcha] (pitfall), [decision] (architectural choice), [convention] (coding standard). Scan these prefixes to find the observation type you need. Call search_graph or list_entities first if you're not sure of the entity name. Index entities (e.g., Index:DynamicCore) list all entities in a domain — read these first when exploring a new area.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn read_entities(&self, params: Parameters<ReadEntitiesParams>)
        -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let result = self.graph.read_entities(&p.project, p.entity_names)
            .map_err(Self::to_error)?;
        let context = if result.entities.is_empty() {
            "no_results"
        } else if !result.relations.is_empty() {
            "has_relations"
        } else {
            ""  // has entities but no relations — silent
        };
        let xml_output = xml::format_graph_result_xml(&result, &p.project, None);
        let suggested = suggestions::get_suggested_next_xml("read_entities", context);
        let output = if suggested.is_empty() {
            xml_output
        } else {
            format!("{xml_output}\n\n{suggested}")
        };
        Self::text_result(output)
    }

    #[tool(
        description = "Audit the entire knowledge graph against the project's schema. Returns all validation warnings across every entity, observation, and relation: unrecognized types, missing tag prefixes, duplicate observations, high observation counts, dangling relations. Use periodically to check graph health — after a batch of updates, at end of session, or when the user requests a graph audit. Do NOT call on every write — write-time validation already catches issues on new data. This tool catches issues in existing data that may have been written before schema rules were added.",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn validate_graph(&self, params: Parameters<ValidateGraphParams>)
        -> Result<CallToolResult, rmcp::ErrorData>
    {
        let p = params.0;
        let result = self.graph.validate_graph(&p.project)
            .map_err(Self::to_error)?;
        match result {
            None => Self::text_result(
                "No schema found — cannot validate. Add a schema.toml to enable validation."
            ),
            Some(warnings) if warnings.is_empty() => {
                Self::text_result("Graph validation passed — no warnings.")
            }
            Some(warnings) => {
                Self::text_result(xml::format_validation_report_xml(&warnings))
            }
        }
    }

    #[tool(
        description = "Add new tags, entity types, relation types, or index nodes to a project's graph schema. Use when a new component, pattern, or concept you're recording doesn't fit the existing schema — add the type first, then create the entity or relation. All fields are optional; omit any category you don't need to update. Changes are written to the project's .lore/graph/schema.toml and take effect immediately.",
        annotations(destructive_hint = false, idempotent_hint = false, open_world_hint = false)
    )]
    async fn update_graph_schema(
        &self,
        params: Parameters<UpdateGraphSchemaParams>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let p = params.0;
        let message = self.graph
            .update_project_schema(
                &p.project,
                p.new_tags.as_deref().unwrap_or(&[]),
                p.new_entity_types.as_deref().unwrap_or(&[]),
                p.new_relationship_types.as_deref().unwrap_or(&[]),
                p.new_index_nodes.as_deref().unwrap_or(&[]),
            )
            .map_err(Self::to_error)?;

        Self::text_result(message)
    }

}


/// Implement the ServerHandler trait from rmcp.
///
/// The tool_box() call wires up the tools defined above
/// into the handler
#[tool_handler]
impl ServerHandler for OrchestratorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(r#"## Three-layer knowledge model

- Knowledge files: long-form reference docs (architecture, conventions, guides)
- Skills: reusable workflow instructions triggered by task type
- Knowledge graph: entities and observations recording decisions, components, and patterns; relations recording how entities are connected

## Start-of-task protocol

1. Call `gather_project_context` (use this exact name when loading):
   - First task in conversation: `include_overview: true` — loads full project-instructions
   - After `gather_project_context` returns, use the `when_to_read` attribute on each <file> to decide which files are relevant to the current task. Files marked "Read at the start of every session" should always be read first.
2. Scan the returned `<graph_entities>`. If any entity names match what you'll be working on, call `read_entities` on them before proceeding.
3. Check the task routing table in project-instructions. Do not begin any implementation until you have read the required knowledge files, graph entities, and/or skills for the task type. Use the section headings and entity names returned by `gather_project_context` to make targeted read tool calls.
4. If a skill in `<skills>` matches the task, read its SKILL.md before starting.

## Behavioral principles

1. Professional objectivity. Prioritize technical accuracy over validating assumptions. Disagree when evidence warrants it. Do not use phrases like "You're absolutely right." Correct information delivered respectfully is more valuable than false agreement.
2. Read before write. Never propose changes to a knowledge file, graph entity, or skill that hasn't been read in this session. Call the appropriate read tool first. Understand existing content before suggesting modifications.
3. No over-engineering updates. When updating knowledge files, make only the changes requested or clearly necessary. Do not reorganize sections, insert unrequested context, or "improve" adjacent content. The right update is the minimum change that achieves the goal.
4. Inverted pyramid output. Lead responses with the most important finding from tool results. Supporting details follow. Do not restate tool results verbatim — synthesize and highlight what matters for the current task.
5. Parallel tool calls. When you need data from multiple independent sources — reading two knowledge files, querying the graph, checking skills — issue all independent calls in a single message. Do not wait for one result before making the next call unless there is a real dependency. Example: reading files A and B is parallel; reading file A then updating a section based on it is sequential.
6. No time estimates. Do not predict how long tasks will take. Break work into concrete steps and let the user judge timing.
7. Output efficiency. Be concise and direct. Lead with the action or answer, not a summary of what you read. Do not restate tool results verbatim — synthesize what matters for the task. If the user asked a question, answer it first, then provide supporting detail.
8. Context budget awareness. Knowledge files range from a few thousand to tens of thousands of characters. Before calling read_knowledge_file, check the file's size attribute from gather_project_context. For files over 15K characters, prefer read_knowledge_section. A typical task should need 2–3 targeted section reads, not 2–3 full file reads.

## Reading

- Prefer: read_knowledge_section / search_knowledge > read_knowledge_file
- Prefer: read_entities / search_graph > read_graph
- Graph search is substring matching, not semantic. Use 1–2 word queries only.
- For known entity names, prefer read_entities — exact lookup, no false negatives.
- For uncertain lookups: read_entities on the relevant Index node first, then read_entities on the specific entity.
- Use search_knowledge or search_graph before asking the user where something is documented.
- When modifying a component with dependents, or debugging complex issues, use search_knowledge or search_graph to find related entities before scoping the fix. Symptoms and causes may live in different files.

## Writing

- After substantive work, update knowledge files and graph entities.
- Use update_knowledge_section or update_skill_section — not full rewrites — when only one section changed.
- Record new decisions, components, patterns, and relationships via add_observations or create_entities.

## Do NOT

- Do NOT read a full knowledge file just to check one fact — use read_knowledge_section or search_knowledge.
- Do NOT list all entities or read the full graph just to find one — use search_graph with a keyword.
- Do NOT restate tool results verbatim in responses — synthesize and highlight what matters.
- Do NOT call gather_project_context more than once per conversation unless files were registered or unregistered.
- Do NOT search with queries longer than 3 words — substring matching works best with 1–2 distinctive keywords.
- Do NOT write to any knowledge file, graph entity, or skill without reading its current content first in this session.
- Do NOT ask the user where something is documented before trying search_knowledge or search_graph.

## Common failure modes (avoid these)

- **Context hoarding**: Reading 3+ full knowledge files when the task only needs 1–2 sections. Check file sizes first; prefer sections.
- **Echo response**: Restating tool output verbatim instead of synthesizing. The user can see tool results — add value by interpreting them.
- **Stale write**: Updating a file or section without reading it first. Always read before write.
- **Blind search**: Asking the user "which file covers X?" before trying search_knowledge or search_graph. Search first, ask only if search fails.
- **Shotgun query**: Searching with a 5-word phrase when a single distinctive keyword would match better.
- **Early stopping**: Giving up you search early before gathering all relevant context. Smaller tasks may only require a few knowledge file sections and/or entities. Larger files may require more. Explore the knowledge base to gather rich context and understand complex dependencies."#.into()),
        }
    }
}

/// The async entry point.
#[tokio::main]
async fn main() -> Result<()> {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("PANIC: {}", info);
    }));

    let args: Vec<String> = std::env::args().collect();

    // Resolve data directory: --data-dir override or platform default
    let data_dir = if let Some(pos) = args.iter().position(|a| a == "--data-dir") {
        PathBuf::from(args.get(pos + 1).expect("--data-dir requires a path"))
    } else {
        init::data_dir()?
    };
    init::ensure_initialized(&data_dir)?;

    let config_path = data_dir.join("config.toml");
    let mut config = OrchestratorConfig::load(&config_path)?;

    // CLI test/bench commands shift — position 1 is now "test" or "bench"
    // unless --data-dir is used, in which case it shifts
    let cmd_pos = if args.iter().any(|a| a == "--data-dir") { 3 } else { 1 };

    if args.get(cmd_pos).map(|s| s.as_str()) == Some("cli") {
        cli::run_cli(&mut config, &config_path, &data_dir)?;
        return Ok(());
    } else if args.get(cmd_pos).map(|s| s.as_str()) == Some("configure") {
        let remaining: Vec<&str> = args.iter().skip(cmd_pos + 1).map(|s| s.as_str()).collect();
        if remaining.contains(&"--list") {
            configure::configure_list(&data_dir)?;
        } else if remaining.contains(&"--all") {
            configure::configure_all(&data_dir)?;
        } else if remaining.contains(&"--app") {
            let name = remaining.iter()
                .position(|a| *a == "--app")
                .and_then(|i| remaining.get(i + 1))
                .ok_or_else(|| anyhow::anyhow!(
                    "--app requires a name (claude-desktop, claude-code, codex)"
                ))?;
            configure::configure_app(name, &data_dir)?;
        } else {
            configure::configure_interactive(&data_dir)?;
        }
        return Ok(());
    }

    //Initial sync of Autopilot projects before validation.
    // This creates clone directories so effective_root() points to real paths.
    git::sync_all_autopilot(&config.projects, &data_dir);

    // Filter out projects that fail validation for the MCP server
    config.projects.retain(|p| {
        match p.validate() {
            Ok(()) => true,
            Err(e) => {
                eprintln!("[warn] Skipping project '{}': {}", p.name, e);
                false
            }
        }
    });

    let global_skills_dir = data_dir.join("skills");
    let defaults_dir = data_dir.join("defaults");
    let resolver = resolver::ProjectResolver::new(config.projects, data_dir.clone());
    let sync_resolver = resolver.clone();
    let knowledge = KnowledgeStore::new(resolver.clone());
    let skills = SkillStore::new(resolver.clone(), Some(global_skills_dir));
    let graph = GraphStore::new(resolver.clone(), defaults_dir);

    let service = OrchestratorServer::new(knowledge, skills, graph, resolver.clone(), config_path, data_dir);

    if args.get(cmd_pos).map(|s| s.as_str()) == Some("test") {
        let tool = args.get(cmd_pos + 1).expect("Usage: ... test <tool_name> '<json_params>'");
        let params = args.get(cmd_pos + 2).map(|s| s.as_str()).unwrap_or("{}");
        cli::run_test(service, tool, params).await?;
    } else if args.get(cmd_pos).map(|s| s.as_str()) == Some("bench") {
        let tool = args.get(cmd_pos + 1).expect("Usage: ... bench <tool_name> '<json>' [iterations]");
        let params = args.get(cmd_pos + 2).map(|s| s.as_str()).unwrap_or("{}");
        let iterations = args.get(cmd_pos + 3)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(10);
        cli::run_bench(service, tool, params, iterations).await?;
    } else {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_mins(20));
            interval.tick().await;
            loop {
                interval.tick().await;
                let resolver = sync_resolver.clone();
                tokio::task::spawn_blocking(move || {
                    let guard = resolver.read().unwrap();
                    git::sync_all_autopilot(guard.list(), guard.data_dir());
                }).await.ok();
            }
        });

        let server = service.serve(stdio()).await?;
        server.waiting().await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;