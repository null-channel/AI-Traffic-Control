## Feature planning: AI agent architecture, tools, and custom commands

### 1. Summary

- Build a first-class AI agent runtime with a clear module boundary, a unified tool interface, and custom commands to supercharge software engineering tasks. Place the agent in its own directory with a dedicated tools subdirectory, and introduce commands for including files/websites as references and adding rules (system-level in SQLite, repository-level as files). Lay groundwork to later add specialized workflows (code review, code writing, test writing).
- Origin: Request to make the agent "super powered" for SE tasks, with structure and extensibility for tools, commands, rules, and workflows.

### 2. Problem statement

- Current state: The project includes sessions, discovery, file ops, git ops, URL ingestion, and a model adapter, but lacks a cohesive agent module with pluggable tools, command parsing, rule management, and workflow orchestration.
- Pain points:
  - No single abstraction for tools callable by the agent.
  - No first-class command parsing for user-issued agent commands (e.g., include file/URL, add rules).
  - No rule store and policy layering (system vs repository) directly accessible to the agent.
  - No workflow engine to encapsulate opinionated flows like code review or test authoring.
- Success criteria:
  - Agent has a modular structure with a stable Tool trait and tool registry.
  - Commands exist for include-file, include-url, and add-rule (system/repo) with tests.
  - System rules persist in SQLite; repo rules materialize under a conventional path.
  - Workflow scaffolding present behind a feature flag, with stubs for code review/code writing/test writing.

### 3. Goals and non-goals

- Goals:
  - Introduce an `agent` module and `agent/tools` directory with a `Tool` trait and shared context.
  - Implement core tools: discovery/file/git adapters, URL include, rule writer, rule reader/merger.
  - Implement custom commands: `/include-file`, `/include-url`, `/add-rule --system|--repo`.
  - Persist system rules in SQLite; write repo rules to `.cursor/rules/` or `docs/rules/` (configurable).
  - Provide a minimal workflow engine interface and stubs for key workflows.
- Non-goals (this plan):
  - Full agent planning/execution trees (e.g., multi-step autonomous loops with recovery).
  - Vector memory and semantic retrieval. Covered by a separate plan.
  - Complete workflow implementations. Here we add interfaces and stubs only.

### 4. Stakeholders and reviewers

- Owner(s): @marek
- Reviewers/Maintainers: TBD
- Impacted users: API/CLI consumers; contributors extending the agent and tools

### 5. Requirements

- Functional requirements:
  - A structured `agent` runtime with a registry for tools and commands.
  - Commands:
    - `/include-file path:<path> [max_bytes:N]` reads file under `project_root` and records it as reference.
    - `/include-url url:<url> [max_bytes:N]` fetches readable content (subject to allowlist) and records it as reference.
    - `/add-rule --system name:<name> [path_hint:<path>]` persists content provided in message or follows up via CLI arg.
    - `/add-rule --repo name:<name> [dir:.cursor/rules]` writes rule file within repo.
  - Rules precedence: repository rules override/augment system rules; merging strategy is deterministic and documented.
  - API/CLI wiring so sessions can use these commands via HTTP or CLI.
  - Observability: tool and command events recorded in `tool_events` with concise summaries; counters via Prometheus.
- Non-functional requirements:
  - Reliability: SQLite-backed persistence for system rules; idempotent repository writes with safe path handling.
  - Security: Maintain network allowlist for URL includes; restrict file includes to `project_root`.
  - Extensibility: Tool and workflow interfaces stable and documented; easy to add new tools/flows.
  - Performance: Bounded content sizes via `max_bytes`; avoid reading large files by default.

### 6. High-level design

- Architecture overview
  - Introduce `src/agent/` with submodules:
    - `src/agent/mod.rs`: public facade (`Agent`, `AgentCommand`, `AgentEvent`).
    - `src/agent/engine.rs`: command parsing, dispatch to tools, and rule merging.
    - `src/agent/tools/mod.rs`: `Tool` trait, `ToolContext`, `ToolResult`, `ToolError`, `ToolRegistry`.
    - `src/agent/tools/` implementations: `include_file.rs`, `include_url.rs`, `rules.rs`, adapters wrapping existing discovery/file/git ops.
    - `src/agent/workflows/mod.rs`: `Workflow` trait and stubs (`code_review.rs`, `code_write.rs`, `test_write.rs`) behind a feature flag.
  - Persisted data additions:
    - New table `rules` for system-level rules.
    - New table `context_items` for included references (file/url) with small content excerpts.
  - External interfaces:
    - HTTP: add `/v1/sessions/:id/agent/command` to post a structured command; continue supporting inline parsing in `/messages` for `/`-prefixed commands.
    - CLI: add `agent` subcommands mirroring HTTP commands for non-interactive usage.

- Key data flows and components
  - Command path: HTTP/CLI -> parse `AgentCommand` -> resolve `ToolContext` (session settings, repo path, allowlist, DB repo) -> dispatch to specific `Tool` implementation -> persist results (tool_events, rules/context items if applicable) -> return structured response.
  - Rule precedence: `EffectiveRules = merge(system_rules, repo_rules)` with an ordered, name-based merge and explicit conflict policy (repo wins by default; surfaced in event summary).

- Data model / schema changes
  - `rules` (system-level):
    - `id` TEXT PRIMARY KEY (UUID v4)
    - `name` TEXT NOT NULL
    - `content` TEXT NOT NULL
    - `created_at` TEXT NOT NULL
    - `updated_at` TEXT NOT NULL
  - `context_items` (references included into a session):
    - `id` TEXT PRIMARY KEY
    - `session_id` TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE
    - `kind` TEXT NOT NULL CHECK(kind IN ('file','url'))
    - `key` TEXT NOT NULL -- path or URL
    - `content_excerpt` TEXT NOT NULL -- bounded by max_bytes
    - `byte_len` INTEGER NOT NULL
    - `created_at` TEXT NOT NULL
    - INDEX on `(session_id, created_at)`

- External interfaces/APIs/contracts
  - HTTP additions:
    - `POST /v1/sessions/:id/agent/command` body: `{ kind: "include_file"|"include_url"|"add_rule", args: {...} }` → structured result plus a `tool_events` entry and optional `context_items` or `rules` write.
  - CLI additions:
    - `agent include-file --id <session> --path <path> [--max-bytes N]`
    - `agent include-url --id <session> --url <url> [--max-bytes N]`
    - `agent add-rule --id <session> --system|--repo --name <name> [--content-file <file>] [--dir <dir>]`

### 7. Scope and milestones

- Milestone 1 (Agent skeleton and tools)
  - Create `src/agent/` and `src/agent/tools/` with `Tool` trait and registry.
  - Implement `include_file` and `include_url` tools using existing discovery/URL ingestion logic, bounded by `max_bytes` and allowlist.
  - Add `context_items` table and repository methods to persist includes.
  - Unit tests for tool parsing and execution.

- Milestone 2 (Rules and commands)
  - Add `rules` table and repository methods for CRUD on system rules.
  - Implement `rules` tool to add system rules and write repo rules to `.cursor/rules/<slug>.md` by default (configurable path).
  - Add HTTP endpoint `/v1/sessions/:id/agent/command` and CLI `agent` subcommands.
  - Integration tests for include and add-rule commands (HTTP and CLI).

- Milestone 3 (Workflow scaffolding)
  - Introduce `Workflow` trait and minimal stubs for `code_review`, `code_write`, `test_write` behind a `workflows` feature flag.
  - Define per-workflow config surfaces and rule hooks; no full implementations in this plan.
  - E2E smoke tests that a stub workflow can be invoked and records tool events.

### 8. High-level test interfaces (must-have)

- Unit-level:
  - Interfaces: `Tool` trait implementations (`include_file`, `include_url`, `rules`), command parser.
  - Behaviors:
    - Parses commands and validates args (path under root, URL allowlist, size bounds).
    - `include_file` respects `max_bytes` and stores `context_items` row.
    - `include_url` uses readability extraction and stores `context_items` row.
    - `rules` creates/updates system rules in DB and writes repo rules to configured dir; name-to-slug mapping stable.

- Integration-level:
  - Interfaces: HTTP endpoint `/v1/sessions/:id/agent/command`, CLI `agent` commands, SQLite repository.
  - Behaviors:
    - Endpoints return structured results and append `tool_events`.
    - DB migrations apply and `rules`/`context_items` roundtrip works.

- End-to-end / system-level:
  - Journeys:
    - Create session → include a file and a URL → verify history, `context_items`, and summaries across restart.
    - Add a system rule and a repo rule → verify precedence and visibility via a "get effective rules" helper.

### 9. Risks and mitigations

- Risk: Command parsing ambiguity vs freeform chat messages.
  - Mitigation: Separate HTTP command endpoint; for `/messages`, only parse leading `/` tokens under a feature flag.
- Risk: Large content ingestion.
  - Mitigation: Require `max_bytes` with upper bound; store only excerpts; provide preview in responses.
- Risk: Rule conflicts and drift between DB and repo.
  - Mitigation: Deterministic merge (repo wins by default) and tool event surfacing conflicts; optional lint.
- Risk: Security of URL fetches.
  - Mitigation: Strict allowlist already enforced; reuse and extend existing policy.

### 10. Rollout and adoption

- Feature flags / config: `workflows` feature gate for workflow stubs; config for repo rules directory (default `.cursor/rules`).
- Backwards compatibility / migrations: Add `0002_rules_and_context.sql`; idempotent. Existing endpoints unchanged; new endpoint additive.
- Observability: Add metrics for command executions and per-tool success/error counters; summarize content lengths in events.

### 11. Open questions

- Should repository rules live under `.cursor/rules/` or `docs/rules/` by default? Propose `.cursor/rules/` for editor/tooling proximity.
- Do we need a "get effective rules" endpoint now, or in the workflows plan?
- Should `include_file` optionally snapshot file content to DB beyond excerpt (trade-off: size vs fidelity)?

### 12. Approval

- Approved by: @marek
- Approval date: 2025-08-13


