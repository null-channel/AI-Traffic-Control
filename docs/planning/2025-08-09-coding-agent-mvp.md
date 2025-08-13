## Feature planning template

### 1. Summary

- Build a headless Rust-based AI coding agent capable of maintaining multiple concurrent sessions for one or more clients. It supports multiple backend AI models and, when not explicitly selected, will automatically choose the model it deems best for the task. The agent must run on Windows, Linux, and macOS.
- Initial toolset focuses on developer productivity for local projects: project discovery (filesystem and content search), file manipulation (CRUD/rename/move), and source control (Git operations).
- Client prompts can include additional context references: file, directory, and websites.
- Why now: Establish a robust, cross-platform, locally runnable foundation for iterative expansion of tools and providers.
- Originating issue: <!-- #TBD -->

### 2. Problem statement

- Current state: No agent runtime exists; developers rely on ad-hoc tools or cloud UIs without deep project context and limited automation over their repositories.
- Pain points: Lack of consistent, scriptable agent with local filesystem and Git awareness; difficulty switching between model providers; no straightforward multi-session orchestration.
- Success criteria:
  - Cross-platform binary/daemon starts and exposes an API/CLI.
  - Multiple concurrent sessions per client, with durable session state in memory (MVP) and optional persistence later.
  - Pluggable model provider abstraction with at least one remote (OpenAI-compatible) and one local (OpenAI-compatible like Ollama) backend.
  - Automatic model selection enabled by simple heuristics/metadata; clients can override.
  - Tools: discovery, file manipulation, and Git ops working end-to-end with guardrails and tests.

### 3. Goals and non-goals

- Goals:
  - Multi-session orchestration with an API surface to create/use/close sessions.
  - Model provider abstraction with at least two providers; automatic selection when unspecified.
  - Core tools: project discovery, file manipulation, and Git source control.
  - Context injection from file, directory, and websites (fetch + readability extraction) into prompts.
  - Cross-platform support (Windows, Linux, macOS); produce a single binary per OS.
- Non-goals (MVP):
  - GUI/desktop app or web UI.
  - Long-running autonomous agents that execute external commands beyond the specified tools.
  - Advanced RAG/vector stores or multi-repo orchestration.
  - Authentication/multi-tenant security hardening beyond basic API token (can be added later).

### 4. Stakeholders and reviewers

- Owner(s): TBD
- Reviewers/Maintainers: TBD
- Impacted teams/users: Individual developers; small teams adopting local agent workflows.

### 5. Requirements

- Functional requirements:
  - Run as a headless service with a minimal HTTP+JSON API and CLI entrypoint.
  - Sessions: create/list/get/post-message/close; concurrent handling.
  - Model providers: implement a `LanguageModel` trait; adapters for OpenAI-compatible HTTP and a local OpenAI-compatible endpoint (e.g., Ollama/OpenRouter).
  - Model selection: rule-based strategy using task metadata (context size, cost preference, streaming need), with override per request or per session.
  - Session-level settings (defaults for the session; overridable per request):
    - `default_model` and `model_params` (temperature, max_tokens, top_p, etc.).
    - `project_root` path restricting tool operations to a sandbox.
    - `tool_policies` like default `dry_run` for edits, max read size, allow/deny globs.
    - `network_allowlist` for URL fetching.
    - Precedence: request override > session settings > global config defaults.
  - Session history:
    - Record chronological history of messages and tool events with timestamps and minimal metadata (model used, token counts if available, tool status, error if any).
    - For file edits, include a diff preview summary (bounded in size) and resulting file paths affected.
    - Expose history via API/CLI with pagination and filtering (messages vs tool events).
    - Redact secrets and apply size limits to stored payloads.
  - Tools:
    - Discovery: list files, glob search, read file(s), search in files; respect ignore rules (.gitignore) and safe project root boundaries.
    - File manipulation: create, write, append, move/rename, delete (with dry-run and confirmation flags in API), diff/preview for edits.
    - Source control: Git status, diff, add, commit (message), branch list/switch/create, restore/checkout file, log (paginated).
  - Context ingestion: accept references of type file, directory, or URL; fetch URL content with basic readability extraction and size limits; summarize/trim before passing to model.
  - Configuration: env vars and config file for API keys, provider endpoints, and safe root directory.
  - Observability: structured logs; basic metrics counters (requests, tool invocations, errors).
- Non-functional requirements:
  - Cross-platform (Windows, Linux, macOS) with minimal native deps.
  - Concurrency: handle at least 20 active sessions and 100 in-flight tool/model requests.
  - Security/safety: sandbox root path; deny paths outside root; redact secrets in logs; network egress restricted to allowed hosts for URL fetch (configurable allowlist).
  - Reliability: graceful shutdown; in-memory session state with optional snapshotting; retries for transient provider failures.
  - Performance: file discovery should handle 50k-file repos with responsive pagination/streaming.

### 6. High-level design

- Architecture overview:
  - Core runtime in Rust (Tokio). HTTP API via `axum`; CLI via `clap`.
  - `SessionManager` orchestrates `Agent` instances keyed by session ID.
  - `LanguageModel` trait with provider adapters (OpenAI-compatible, local provider). Streaming token support.
  - `Tool` trait with concrete tools: `DiscoveryTool`, `FileTool`, `GitTool`.
  - `ContextProvider` utilities for file/directory ingestion and URL fetch + readability extraction.
  - Simple `ModelSelector` implementing rule-based selection.
- Key data flows and components:
  - Client -> API -> SessionManager -> Agent -> (Model + Tools) -> Responses/Edits.
  - Tools may request previews/diffs; file writes gated by safe root and optional dry-run.
- Data model (MVP, in-memory):
  - `Session { id, client_id?, created_at, messages[], tool_history[], settings }`
  - `ToolInvocation { tool, params, result, error? }`
  - `ContextItem { kind: file|directory|url, reference, content_meta }`
  - `SessionSettings { default_model?, model_params?, project_root, tool_policies?, network_allowlist? }`
  - `SettingsResolver` merges global config, session settings, and per-request overrides.
  - `Message { id, role, content_ref (trimmed content or pointer), model_used?, token_usage?, created_at }`
  - `ToolEvent { id, tool, params_ref (redacted), summary, status, error?, diff_ref?, created_at }`
- External interfaces/APIs:
  - HTTP endpoints (prefix `/v1`):
    - `POST /sessions` body may include `settings`; returns `{ id, settings }`.
    - `GET /sessions/{id}` returns basic session info including `settings`.
    - `GET /sessions/{id}/settings` returns effective session settings.
    - `PATCH /sessions/{id}/settings` to update session-level settings.
    - `GET /sessions/{id}/history?kind=messages|tools&cursor=&limit=` returns paginated history.
    - `POST /sessions/{id}/messages` (prompt, optional model, context refs)
    - `DELETE /sessions/{id}`
    - `GET /healthz`
  - CLI: `atc start`, `atc session create`, `atc session send`, `atc session close`.
    - Examples: `atc session create --model gpt-4.1 --root /path/to/project`;
      `atc session settings get <id>`; `atc session settings set <id> --dry-run true --max-read-bytes 1048576`;
      `atc session history <id> --kind tools --limit 50`.

### 7. Scope and milestones

- Milestone 1 (MVP):
  - HTTP API + CLI skeleton; session CRUD; in-memory store.
  - `LanguageModel` trait + OpenAI-compatible provider; basic streaming.
  - `DiscoveryTool`: list, read, search; `FileTool`: write/create/move/delete with dry-run; `GitTool`: status/diff/add/commit.
  - Context ingestion: files/dirs and URL fetch with size limits.
  - Model selection: minimal rules; manual override supported.
  - Session-level settings CRUD (create/get/patch) and precedence resolution.
  - Session history capture (messages + tool events) with pagination endpoints and CLI access.
  - Logs/metrics; configuration; cross-platform builds.
- Milestone 2:
  - Additional providers (Anthropic adapter, model capability registry).
  - Advanced diffs/patch previews; multi-file edit transactions.
  - Session persistence (sqlite/serde snapshot).
  - Authorization tokens per client; rate limiting and sandbox per-session root.
  - History persistence/export to disk (sqlite/JSONL) and retention policies.

### 8. High-level test interfaces (must-have)

- Unit-level:
  - Interfaces: `LanguageModel` adapter stubs; `ModelSelector` policy; `Tool` trait implementations (file ops, discovery filters, git ops mocks).
  - Behaviors to validate:
    - Provider request/response mapping and error handling.
    - Selection chooses expected provider given task hints and constraints.
    - Settings precedence (request > session > global) and defaults.
    - File boundary enforcement and ignore rules; dry-run vs apply behavior.
    - Git commands produce expected porcelain outputs (mocked repo).
    - History appends correct entries for messages and tool events; redaction and size limits enforced.
- Integration-level:
  - Interfaces: HTTP API flows across session create -> send message -> tool invocation -> response stream.
  - Behaviors to validate:
    - End-to-end prompt with file context triggers discovery/read and returns summarized content.
    - Editing a file via API provides a diff preview then apply works and is reflected in git status, honoring session `project_root` and default `dry_run`.
    - URL ingestion fetches and trims content within limits.
    - Patching session settings takes effect for subsequent requests.
    - History endpoints return expected data with pagination and filters.
- End-to-end / system-level (smoke):
  - User journeys / invariants:
    - Start daemon, create session with `--root`, run discovery, make an edit, commit changes.
    - Concurrency: run N sessions performing read/search without deadlocks.

### 9. Risks and mitigations

- Cross-platform filesystem and git differences -> Use crates `ignore`, `walkdir`, `git2`; add platform-specific tests in CI matrix.
- Provider API incompatibilities/rate limits -> Use OpenAI-compatible surface first; implement backoff and request size guards.
- Security of file edits and URL fetch -> Enforce safe root, path normalization, denylist/allowlist for network; redact logs.
 - Misconfiguration of session `project_root` -> Validate path on create/patch; restrict to configured sandbox; clear error messages.
 - Sensitive data in history -> Redact secrets and large payloads; provide configurable retention and opt-in export; do not persist by default in MVP.
- Performance on large repos -> Paginate and stream results; respect `.gitignore` aggressively; allow cancelation.

### 10. Rollout and adoption

- Feature flags / config: enable/disable providers and tools; configure safe root and allowlisted hosts via config/env.
- Backwards compatibility / migrations: N/A for MVP (in-memory). Future persistence via sqlite with migrations.
- Observability: structured logs with `tracing`; metrics via `metrics` crate; health endpoint for readiness/liveness.

### 11. Open questions

- API style sufficiency: HTTP+JSON only for MVP, or add stdio JSON-RPC?
- Initial provider set: OpenAI-compatible remote plus which local default (Ollama)?
- Minimum supported Rust toolchain version and MSRV policy?
- Website fetch: which readability/extraction approach and HTML size cap?

### 12. Approval

- Approved by: Marek Counts
- Approval date: Today

Status: Completed (2025-08-12)


