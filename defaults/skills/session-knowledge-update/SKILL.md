---
name: session-knowledge-update
description: >
  Run this skill at the end of any chat where changes were made to the codebase. Captures
  what was modified during the session and determines what needs to be updated across the
  knowledge base — the knowledge graph (entities, observations, relations) and the knowledge
  files. Proposes all updates for user review before making them. Trigger whenever the user
  says things like "update the docs", "update the knowledge base", "let's capture what we
  did", "log this session", "update memory", or "end of session" — or whenever a substantive
  coding session is wrapping up and the user wants to preserve what was learned.
---

# Session Knowledge Update

This skill runs at the end of a development session to capture what changed and propagate
those learnings into the project's knowledge base: the **knowledge graph** and the **knowledge files**.

The goal is a knowledge base that stays accurate and useful. This skill is the mechanism
that keeps it in sync with the actual codebase.

---

## Step 1 — Review the Session

Scan the entire conversation from the beginning. Build a precise picture of what changed.
Do not summarize vaguely — be specific about files, entities, composables, components,
patterns, and decisions. Organize your findings into these categories:

### Change Categories

**New code added**
- New files created (path, purpose)
- New components, composables, utils, types, constants, schemas
- New entity types, config types, prop types
- New patterns or conventions introduced

**Existing code modified**
- Files changed (path, what changed and why)
- API surface changes (new/removed/renamed parameters, return values)
- Behavioral changes (how something works differently now)
- Bug fixes (what the bug was, what the fix was)

**Code deleted or deprecated**
- Files removed
- Features or patterns no longer in use
- Things that were replaced by something else

**Architectural decisions made**
- Explicit choices between approaches and the rationale
- Tradeoffs documented
- Constraints discovered (gotchas, timing issues, Vue reactivity caveats, etc.)

**Conventions established**
- Naming conventions, file placement decisions
- Patterns that should be followed going forward
- Things that should NOT be done (and why)

If a change is minor (e.g., a small style fix with no architectural implication), note it
briefly but don't expand it. Prioritize changes that affect how future work should be done.

---

## Step 2 — Identify Relevant Knowledge Base Areas

With the session changes in hand, determine which parts of the knowledge base are affected.

### Knowledge Graph

The graph is organized around Index nodes per domain. Consult the project's
`project-instructions` knowledge file for the Index node map specific to this project.

For each area touched by the session:
1. Open the relevant Index node(s) to see existing entities
2. Open individual entity nodes that are directly affected
3. Identify gaps: missing entities, stale observations, missing relations

**Knowledge graph update types:**
- **New entity** — a new file/composable/component/system that doesn't exist in the graph yet
- **New observation** — a new fact about an existing entity (gotcha discovered, API surface changed, pattern documented)
- **Updated observation** — an existing observation that is now inaccurate or incomplete
- **Deleted observation** — an observation that is no longer true
- **New relation** — a connection between entities that wasn't captured
- **New entity in Index** — when adding a new entity, check if its Index node needs updating

### Knowledge Files

Consult the knowledge file index from `gather_project_context` (or `list_knowledge_files`)
to identify which registered files are relevant to the session's changes.

For each affected file, identify:
- Which section(s) need updating
- What the current text says (quote it precisely)
- What it should say instead (or what should be added/removed)

---

## Step 3 — Prepare the Update Proposal

Present a structured update proposal to the user for review. Format it clearly so the user
can approve, reject, or refine each item individually.

Use this structure:

---

### Knowledge Graph Updates

For each proposed change, include:

**[NEW ENTITY] `EntityName`**
- Type: `component` / `composable` / `system` / `type` / etc.
- Index node to add it to: `Index:XYZ`
- Observations:
  - `[file] path/to/file.ts`
  - `[purpose] What this entity does`
  - `[api] Key API surface`
  - `[gotcha] Any tricky behavior discovered`

**[NEW OBSERVATIONS] `EntityName`**
- Add: `[tag] The new observation text`
- Add: `[tag] Another new observation`

**[UPDATE OBSERVATION] `EntityName`**
- Current: `[tag] The old observation text`
- Replace with: `[tag] The corrected observation text`

**[DELETE OBSERVATION] `EntityName`**
- Remove: `[tag] The observation that is no longer true`
- Reason: Brief explanation of why it's stale

**[NEW RELATION] `EntityA` → `EntityB`**
- Relation type: `calls` / `extends` / `depends on` / `registers with` / etc.

---

### Knowledge File Updates

For each file that needs changes:

**`filename.md`**

*Section: [section name]*

Current text:
> [exact quote of what it currently says, or "N/A — new addition"]

Proposed change:
```
[The new or updated text, ready to paste in]
```

Reason: [One sentence on why this change is needed]

---

### ⏭️ No Action Needed

List any areas you checked that do NOT need updates, so the user knows you looked:
- `Index:ExampleDomain` — no changes to this domain in this session
- `example-reference.md` — this area was not touched

---

## Step 4 — Get User Approval

After presenting the proposal, explicitly ask:

> "Please review the proposed updates above. For each item, let me know:
> - ✅ Approve as-is
> - ✏️ Refine (tell me what to change)
> - ❌ Skip (not needed)
>
> Or say 'approve all' to accept everything."

Wait for the user's response before proceeding to Step 5. Do not make any changes before
approval.

---

## Step 5 — Execute Approved Updates

### Knowledge Graph Updates

Use the Lore graph tools to execute approved changes. Follow this sequence:

1. **Create new entities first** (before adding relations that reference them) — `create_entities`
2. **Add observations to entities** (new and existing) — `add_observations`
3. **Delete stale observations** — `delete_observations`
4. **Create new relations** between entities — `create_relations`
5. **Update Index nodes** to include new entity names — `add_observations` on the Index entity

**Observation format rules:**
- Always prefix observations with a `[tag]` there are universal tag references for all projects, and project-specific tags documented in the project's `project-instructions`
- Observations should be concise factual notes — one fact per observation
- Write observations so they're useful when retrieved by substring search

**Verification:** After making changes, call `read_entities` on the updated entities to confirm
the observations look correct. If an Index node was updated, read it to verify the entity
appears in the list.

### Knowledge File Updates

Use the Lore MCP tools to execute approved knowledge file changes directly.
Do not instruct the user to make manual edits.

For each approved update:
1. Use `update_knowledge_section` to replace a single section — preferred for most updates.
2. Use `write_knowledge_file` only for full document rewrites.

Parameters for `update_knowledge_section`:
- `project`: the project name from config.toml (e.g. `"rad-hub"`)
- `file_key`: the manifest key for the file (e.g. `"dynamic-ui-system-reference"`)  
- `heading`: exact heading with # prefix (e.g. `"## Architecture"`)
- `content`: replacement content below the heading — do not include the heading itself

After each update, confirm to the user which file and section was changed.

---

## Guardrails

**Do not fabricate.** Only document things that actually happened in the session. If you're
uncertain whether something changed, say so and ask the user to confirm.

**Do not over-document.** Minor implementation details that won't affect future development
don't belong in the knowledge base. Focus on: API surfaces, gotchas, architectural decisions,
conventions, and patterns.

**Do not duplicate.** Before adding a new observation, check whether the entity already has
a similar one. An entity with 20 observations is harder to scan than one with 8 precise ones.

**Keep observations atomic.** One fact per observation. If you want to capture two facts,
write two observations.

**Prefer updating over appending.** If an existing observation is partially wrong, replace it
rather than adding a contradicting one alongside it.

**Tag correctly.** Observations will be found by substring search. Use the right tag so they
show up in cross-entity searches for that topic (`gotcha`, `cascade`, `connection`, etc.).