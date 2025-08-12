# Air Traffic Control — Brain

Concise, always-current knowledge base for major project information. Keep this file short, link to canonical docs, and update it whenever significant changes occur.

## Purpose
- Centralize essential context to speed up navigation and decision-making.

## Current snapshot

- Architecture: Headless Rust AI coding agent skeleton in place. HTTP API via `axum`; CLI via `clap`.
  - Implemented HTTP endpoints: sessions create/list, session settings get/patch, session history (messages/tools pagination), discovery (list/search/read), files (write/move/delete with dry-run), git (status/diff/add_all/commit).
  - Implemented CLI commands: `start`, `git` (status/diff/add-all/commit), `discovery` (list/search/read), `files` (write/move/delete).
  - Pending (MVP): message posting + model integration/selection, `/v1/healthz`, session delete endpoint, CLI `session` commands, URL ingestion, basic metrics.
- Processes and policies:
  - Conventional Commits required. See `.cursor/rules/conventional-commits.mdc`.
  - Planning-first policy for new features. See `docs/process/feature-planning.md` and template `docs/templates/FEATURE_PLANNING_TEMPLATE.md`.
  - Tests: new tests allowed; existing tests immutable. See `.cursor/rules/no-auto-tests.mdc`.

## Conventions

- Commits: Conventional Commits v1.0.0. Example: `feat(api): add runway allocation service`.
- Branches: `feature/<feature-slug>` for feature work.
- Planning docs: `docs/planning/YYYY-MM-DD-feature-slug.md` using `docs/templates/FEATURE_PLANNING_TEMPLATE.md`.

## PR expectations

- Link the planning doc.
- Include tests per planned test interfaces.
- Note deviations from plan and address compatibility/migration if relevant.

## Key commands

- Create planning doc (example date/slug):
  - Path: `docs/planning/2025-08-09-coding-agent-mvp.md`
  - Commit example:
    - `git add -A`
    - `git commit -m "feat(planning): scaffold coding-agent MVP plan" -m "Plan: docs/planning/2025-08-09-coding-agent-mvp.md"`

## Links

- Planning process: [feature-planning.md](mdc:docs/process/feature-planning.md)
- Planning template: [FEATURE_PLANNING_TEMPLATE.md](mdc:docs/templates/FEATURE_PLANNING_TEMPLATE.md)
- Conventional commits rule: [.cursor/rules/conventional-commits.mdc](mdc:.cursor/rules/conventional-commits.mdc)
- No-auto-tests rule: [.cursor/rules/no-auto-tests.mdc](mdc:.cursor/rules/no-auto-tests.mdc)

## Ownership

- Approver(s): TBD. Add maintainer names/handles here when defined.


