---
name: project-init
description: >
  Guide the collaborative setup of a project's .lore/ knowledge base after scaffolding
  has been created. Use this skill whenever a user asks to "set up", "initialize",
  "populate", or "onboard" a project's knowledge base — or when gather_project_context
  returns a project with empty or template-only knowledge files. This skill covers
  reading the codebase, creating knowledge files, seeding the knowledge graph, customizing
  the graph schema, and optionally creating project-specific skills. Always use this skill
  for new project onboarding, even if the user just says "help me get started with lore"
  or "set up knowledge for my project".
---

# Project Init

A collaborative workflow for populating a project's `.lore/` knowledge base. The scaffolding
(directory structure, template files) already exists — this skill is about reading the
codebase and filling those templates with real content.

## Prerequisites

Before starting, verify:

1. The project is registered — call `list_all_projects` and confirm it appears.
2. The scaffolding exists — call `gather_project_context` for the project. You should see
   `knowledge.toml` with at least a `project-instructions` entry, an empty or template
   `project-instructions.md`, and an empty graph.

If the project isn't registered, ask the user to run:
```
lore cli add-project <name> --root <path>
```

If scaffolding is missing, ask the user to run:
```
lore cli init-project <name>
```

Once both are confirmed, proceed.

## Workflow Overview

The init process has four phases. Each phase involves Claude reading the codebase,
proposing content, and getting user confirmation before writing. Never write knowledge
files or graph data without the user's approval.

```
Phase 1: Orient → Read the codebase, understand the project
Phase 2: Knowledge Files → Write project-instructions.md, create reference docs
Phase 3: Knowledge Graph → Customize the schema, seed initial entities
Phase 4: Skills (optional) → Create project-specific workflow skills
```

Phases 2–4 each end with a checkpoint where you present what you plan to write and ask
the user to confirm, edit, or skip.

---

## Phase 1: Orient

**Goal:** Build a mental model of the project before writing anything.

Start by reading the project's directory structure. Ask the user to share or describe:

- The top-level directory layout (or ask them to paste a tree output)
- The primary language(s) and framework(s)
- The project's purpose in one sentence
- Any existing documentation (README, wiki, CONTRIBUTING.md, etc.)

Then ask these questions — skip any the user has already answered:

1. **Architecture style**: monolith, monorepo, microservices, library, CLI tool?
2. **Key subsystems**: What are the 3–5 major parts of the codebase? (e.g., "auth",
   "API layer", "UI components", "data pipeline")
3. **Team context**: Solo project or team? If team, how many developers?
4. **Development patterns**: Any strong conventions? (naming, file organization,
   testing approach, PR workflow)
5. **What do you want to use lore for?** Helping Claude Code work more effectively?
   Onboarding new developers? Tracking architectural decisions? All of the above?

Don't try to read the entire codebase. Focus on:
- README or top-level documentation
- Package manifest (package.json, Cargo.toml, pyproject.toml, etc.)
- Top-level directory structure (1–2 levels deep)
- Configuration files that reveal architecture (docker-compose, tsconfig, etc.)

By the end of this phase, you should be able to describe the project's architecture,
tech stack, and key conventions in your own words. Confirm your understanding with the
user before proceeding.

---

## Phase 2: Knowledge Files

**Goal:** Populate `project-instructions.md` and register any additional reference docs.

### Step 2.1: Project Instructions

The scaffolded `project-instructions.md` has template section headings. These are
suggestions, not requirements — if the project's structure calls for a different
organization, override the template entirely with `write_knowledge_file`. The
template exists to prevent a blank-page problem, not to constrain the output.

That said, the standard sections work well for most projects:

- **Overview** — What the project is, who it's for, core responsibilities
- **Architecture** — High-level structure, key subsystems, data flow
- **Tech Stack** — Languages, frameworks, key dependencies with versions
- **Conventions** — Naming, file organization, code style, testing patterns
- **Development Workflow** — How to run, test, deploy; branch strategy
- **Task Routing** — When to read which knowledge files or skills (fill this in
  after all files are created — revisit at the end)

Discuss the structure with the user before writing. If they want different sections,
fewer sections, or a completely different layout — go with their preference.

Read the current template with:
```
read_knowledge_file(project: "<name>", file_key: "project-instructions")
```

Draft the content section by section. Present each section to the user for review.
If using the standard sections, write them incrementally:
```
update_knowledge_section(
  project: "<name>",
  file_key: "project-instructions",
  heading: "## Overview",
  new_content: "<approved content>"
)
```

If the user chose a custom structure, write the entire file at once:
```
write_knowledge_file(
  project: "<name>",
  file_key: "project-instructions",
  content: "<full approved content>"
)
```

Repeat for each section. Leave "Task Routing" for last — it references other files
that may not exist yet.

### Step 2.2: Additional Reference Docs

Based on the project's complexity, propose additional knowledge files. Common patterns:

| File | When to create | Content |
|------|----------------|---------|
| `api-reference.md` | Project has a REST/GraphQL API | Endpoint patterns, auth, error handling |
| `data-model.md` | Complex data layer (DB, ORM) | Schema overview, relationships, migrations |
| `ui-system.md` | Frontend with component library | Component patterns, styling approach, state management |
| `auth-reference.md` | Non-trivial auth system | Auth flow, providers, roles/permissions |
| `deployment.md` | Complex deploy pipeline | Environments, CI/CD, infrastructure |
| `testing-guide.md` | Specific testing patterns | Test utilities, fixtures, coverage expectations |

For each proposed file:

1. Explain what it would contain and why it's useful
2. Ask the user: create it, skip it, or modify the scope?
3. If approved, create the file and register it:

```
register_knowledge_file(
  project: "<name>",
  file_key: "<key>",
  file_path: ".lore/docs/<filename>.md",
  summary: "<one-line description>",
  when_to_read: "<guidance on when this file is relevant>"
)
```

```
write_knowledge_file(
  project: "<name>",
  file_key: "<key>",
  content: "<content>"
)
```

**Important:** Don't create files for the sake of completeness. A solo developer
working on a CLI tool doesn't need `ui-system.md`. Match the documentation to the
project's actual complexity. Two or three well-written reference docs are better than
six thin ones.

### Step 2.3: Update Task Routing

Now that all knowledge files exist, go back and fill in the Task Routing section of
`project-instructions.md`. This tells Claude which files to read for which kinds of
tasks. Example format:

```markdown
## Task Routing

| Task type | Read first | Then |
|-----------|-----------|------|
| UI work | ui-system | project-instructions (Conventions) |
| API changes | api-reference | data-model |
| Auth changes | auth-reference | api-reference |
| New feature | project-instructions | relevant reference doc |
```

---

## Phase 3: Knowledge Graph

**Goal:** Customize the graph schema for this project and seed initial entities.

### Step 3.1: Schema Customization

The scaffolded `schema.toml` has generic defaults. Customize it for this project.
Read the current schema:

```
read_knowledge_file(project: "<name>", file_key: "graph-schema")
```

The schema has four sections to consider:

**Entity types** — What kinds of things exist in this project? Common types:
`Component`, `Module`, `Service`, `Endpoint`, `DataModel`, `Pattern`, `Decision`,
`Convention`, `Dependency`, `Utility`. Only add types that the project actually has.

**Relation types** — How do entities relate? Common relations:
`implements`, `uses`, `extends`, `depends_on`, `contains`, `validates`,
`configured_by`, `tested_by`. Use snake_case, no spaces.

**Tag prefixes** — Observation categorization prefixes. Standard set:
`purpose:`, `pattern:`, `convention:`, `tech:`, `status:`, `note:`.
Add domain-specific prefixes if needed (e.g., `security:` for auth-heavy projects).

**Index nodes** — Required index entities that serve as navigational anchors.
Usually one is enough: `Index:Core`. Projects with distinct subsystems might add
`Index:Frontend`, `Index:Backend`, etc.

Present the customized schema to the user. Once approved, write it:

```
write_knowledge_file(
  project: "<name>",
  file_key: "graph-schema",
  content: "<approved schema>"
)
```

### Step 3.2: Seed Initial Entities

Create the foundational entities that represent the project's architecture. Start with
Index nodes, then add the major subsystems.

**Always create the Index node(s) first:**

```
create_entities(
  project: "<name>",
  entities: [
    {
      "name": "Index:Core",
      "entity_type": "Index",
      "observations": ["purpose: Master index of all major project entities"]
    }
  ]
)
```

**Then create entities for the major subsystems** identified in Phase 1.
For each entity, include 2–4 observations that capture its purpose, key patterns,
and relationships. Use the tag prefixes from the schema.

Example for a web application:

```
create_entities(
  project: "<name>",
  entities: [
    {
      "name": "AuthSystem",
      "entity_type": "Module",
      "observations": [
        "purpose: Handles authentication and authorization via OAuth2/OIDC",
        "tech: Uses better-auth with Microsoft Entra ID provider",
        "pattern: Session-based auth with JWT tokens for API routes"
      ]
    }
  ]
)
```

**Then wire up relations:**

```
create_relations(
  project: "<name>",
  relations: [
    { "from": "Index:Core", "to": "AuthSystem", "relation_type": "contains" },
    { "from": "APILayer", "to": "AuthSystem", "relation_type": "uses" }
  ]
)
```

**Finally, register new entities with the Index node:**

```
add_observations(
  project: "<name>",
  observations: [
    {
      "entity_name": "Index:Core",
      "contents": ["contains: AuthSystem, APILayer, UIComponents, DataLayer"]
    }
  ]
)
```

Present the full entity and relation plan to the user before executing. The user
knows their codebase — they'll catch misunderstandings. Aim for 5–15 initial entities
depending on project size. You can always add more later.

**Checkpoint:** Run `validate_graph` after seeding to catch any schema violations
(wrong entity types, bad relation types, missing tag prefixes). Fix any issues before
moving on.

---

## Phase 4: Skills (Optional)

**Goal:** Create project-specific workflow skills if the user wants them.

Ask the user: "Are there any repetitive or complex workflows in this project that
you'd like documented as step-by-step skills? For example: adding a new API endpoint,
creating a new UI component, setting up a new test suite, deploying to staging."

If the user identifies workflows, create a skill for each one:

```
create_skill(
  project: "<name>",
  skill_name: "<workflow-name>",
  content: "<SKILL.md content>",
  scope: "project"
)
```

Each skill should follow the standard SKILL.md format:

```markdown
---
name: <workflow-name>
description: <when to use this skill and what it does>
---

# <Workflow Name>

<Brief description of what this workflow accomplishes>

## Steps

1. <Step with specific file paths, commands, or tool calls>
2. <Next step>
...

## Conventions

- <Project-specific rules for this workflow>

## Checklist

- [ ] <Verification item>
```

For complex workflows, consider adding reference files under the skill directory
using `write_skill_file`. For example, a "new component" skill might include a
`references/component-template.md` with a boilerplate template.

If the user doesn't want skills right now, skip this phase. Skills can always
be created later as patterns emerge during development.

---

## Wrap-Up

After completing the phases, give the user a summary:

1. List all knowledge files created (with keys and summaries)
2. List the graph entities and relation count
3. List any skills created
4. Remind them of the key tools for ongoing maintenance:
   - `gather_project_context` — start every task with this
   - `update_knowledge_section` — keep docs current after changes
   - `add_observations` / `create_entities` — grow the graph over time
   - `session-knowledge-update` skill — end-of-session knowledge updates

Finally, do one last `gather_project_context` call and present the output so the
user can see their fully initialized knowledge base.
