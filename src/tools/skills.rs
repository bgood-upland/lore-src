use schemars::JsonSchema;
use serde::Deserialize;

// ─── list_skills ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSkillsParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,
}

// ─── read_skill ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadSkillParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,

    #[schemars(description = "The skill directory name (e.g. 'dynamic-ui'). Must match a key shown in gather_project_context output.")]
    pub skill_key: String,

    #[schemars(description = "Optional path to a file within the skill directory (e.g. 'references/new-component.md'). Omit to read SKILL.md (the default). Use list_skill_files first to discover available reference files.")]
    pub file_path: Option<String>,
}

// ─── list_skill_files ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSkillFilesParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,

    #[schemars(description = "The skill directory name (e.g. 'dynamic-ui')")]
    pub skill_key: String,
}

// ─── create_skill ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateSkillParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,

    #[schemars(description = "Directory name for the new skill (e.g. 'my-new-skill'). Use lowercase-kebab-case. Must be unique within the target scope.")]
    pub skill_name: String,

    #[schemars(description = "Full content of SKILL.md. Must contain valid YAML frontmatter with at minimum 'name' and 'description' fields. The 'description' field is the trigger text shown in gather_project_context — write it to clearly describe when the skill should be used.")]
    pub content: String,

    #[schemars(description = "Where to create the skill: 'project' (default) or 'global'. Project skills live in <project_root>/.lore/skills/. Global skills are shared across all projects.")]
    pub scope: Option<String>,
}

// ─── write_skill_file ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteSkillFileParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,

    #[schemars(description = "The skill directory name (e.g. 'dynamic-ui')")]
    pub skill_key: String,

    #[schemars(description = "Path to the file within the skill directory (e.g. 'references/new-component.md'). Intermediate directories are created automatically.")]
    pub file_path: String,

    #[schemars(description = "Full content to write. Creates the file if it doesn't exist, overwrites if it does. Read existing content first if the file already exists.")]
    pub content: String,
}

// ─── delete_skill ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteSkillParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,

    #[schemars(description = "The skill directory name to delete. Verify with list_skills or gather_project_context first.")]
    pub skill_key: String,

    #[schemars(description = "Which scope to delete from: 'project' (default) or 'global'. Must match where the skill was created.")]
    pub scope: Option<String>,
}

// ─── delete_skill_file ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteSkillFileParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,

    #[schemars(description = "The skill directory name (e.g. 'dynamic-ui')")]
    pub skill_key: String,

    #[schemars(description = "Path to the file within the skill directory to delete (e.g. 'references/old-guide.md'). Cannot delete SKILL.md — use delete_skill to remove the entire skill.")]
    pub file_path: String,
}

// ─── update_skill_section ────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateSkillSectionParams {
    #[schemars(description = "Project name as registered in config.toml")]
    pub project: String,

    #[schemars(description = "The skill directory name (e.g. 'dynamic-ui')")]
    pub skill_key: String,

    #[schemars(description = "Path to the file within the skill directory. Omit or leave empty to target SKILL.md (the default).")]
    pub file_path: Option<String>,

    #[schemars(description = "The exact markdown heading to update, including # prefix (e.g. '## Usage'). Must match the heading text exactly.")]
    pub heading: String,

    #[schemars(description = "New content to place under the heading. Replaces everything between the heading and the next heading at the same or higher level. The heading line is preserved — do not include it.")]
    pub content: String,
}