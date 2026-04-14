use serde::Deserialize;
use schemars::JsonSchema;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GatherProjectContextParams {
    #[schemars(description = "The project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "If true, includes the full project-instructions document (~10K chars). Set to true on the first call of a conversation. Set to false on follow-up tasks to avoid re-loading instructions already in context. Default: false.")]
    pub include_overview: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InitProjectParams {
    #[schemars(description = "Project name. Used as the display name in knowledge.toml and as the registration key in config.")]
    pub project: String,
    #[schemars(description = "Absolute path to the project root directory. Required if the project is not yet registered in config. Ignored if the project is already registered (the existing root is used).")]
    pub root: Option<String>,
    #[schemars(description = "Git repository URL for Autopilot mode (e.g. git@github.com:org/repo.git). Required for Autopilot mode if the project is not yet registered. Triggers a clone to ~/.lore/repos/{name}. Mutually exclusive with root.")]
    pub repo: Option<String>,
    #[schemars(description = "Branch to track in Autopilot mode. Defaults to 'main'. Ignored for Manual mode.")]
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SyncProjectParams {
    #[schemars(description = "Project name to sync")]
    pub project: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ListProjectsParams {}

#[derive(Deserialize, JsonSchema)]
pub struct ListFilesParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadFileParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "File key from the project manifest (as shown in gather_project_context output)")]
    pub file_key: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct WriteFileParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "File key from the project manifest. The file must already be registered.")]
    pub file_key: String,
    #[schemars(description = "Full file content to write. This replaces the entire file — read the current content first to avoid losing sections you didn't intend to change.")]
    pub content: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateSectionParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "File key from the project manifest")]
    pub file_key: String,
    #[schemars(description = "The exact markdown heading to update, including the # prefix and level (e.g. '## Architecture', '### Key Gotchas'). Must match the heading text exactly as shown in gather_project_context output.")]
    pub heading: String,
    #[schemars(description = "New content to place under the heading. This replaces everything between the heading and the next heading at the same or higher level. The heading line itself is preserved — do not include it in the content. Write content as if it follows immediately after the heading line.")]
    pub content: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct RegisterFileParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "Unique key for this file in the manifest. Use lowercase-kebab-case (e.g. 'auth-reference', 'api-conventions'). Must not already exist in the manifest.")]
    pub key: String,
    #[schemars(description = "Path relative to the project root (e.g. '.lore/docs/auth-reference.md'). The file does not need to exist on disk yet.")]
    pub path: String,
    #[schemars(description = "Short description of what this file contains. This appears in gather_project_context output and helps decide when to read the file.")]
    pub summary: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UnregisterFileParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "The manifest key of the file to remove. Verify with gather_project_context or list_knowledge_files first.")]
    pub file_key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchKnowledgeParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "Case-insensitive substring to search for. Use 1–3 distinctive keywords for best results. Avoid long phrases — they reduce matches. For component names, use the name without the path (e.g. 'useContainer' not 'composables/dynamic/use-container.ts').")]
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadSectionParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
    #[schemars(description = "The manifest key of the file to read from")]
    pub file_key: String,
    #[schemars(description = "The exact markdown heading to read, including the # prefix and level (e.g. '## Architecture', '### Key Gotchas'). Must match the heading text exactly as shown in gather_project_context output. The section includes all content until the next heading at the same or higher level.")]
    pub heading: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListFileHeadingsParams {
    #[schemars(description = "The project name as defined in config.toml")]
    pub project: String,
    #[schemars(description = "The manifest key of the file to list headings for")]
    pub file_key: String,
}