## Feature planning: Agent planning/execution trees and autonomous loops (TBD)

### 1. Summary

- Define and implement a robust planning/execution engine for the AI agent that supports multi-step autonomous loops with introspection, branching, and recovery. This includes plan generation, execution, monitoring, rollback/compensation, and deterministic replay. This document is a TBD placeholder to scope and track the work; details will be iterated and approved before implementation.
- Origin: Need for a "super powered" agent that can handle complex software engineering tasks end-to-end with resilience.

### 2. Problem statement

- Current state: The agent lacks a dedicated planning/execution system. It executes request/response style actions without plan graphs, error classification, or recovery strategies.
- Pain: Complex tasks (multi-file refactors, staged rollouts, test generation then fixes) require coordinated multi-step plans, retries, and safe rollback.
- Success criteria:
  - Plans are represented as explicit graphs/trees with state and step contracts.
  - Execution is observable, interruptible, and recoverable with checkpoints.
  - Failures are classified and routed to retry/backoff, alternate strategies, or human-in-the-loop.

### 3. Goals and non-goals

- Goals:
  - Plan representation (task graph), step contracts, and metadata (preconditions, postconditions, effects).
  - Execution engine with step scheduling, timeouts, retries, and backoff.
  - Recovery: checkpointing, resume, rollback/compensation hooks, and deterministic replay.
  - Search strategies: linear (ReAct), shallow branching (Tree-of-Thoughts-lite), and critique/reflection loops.
  - Safety: guardrails for file/system access, diff-based edits, and policy enforcement.
- Non-goals (initial phase):
  - Full MCTS or large-branch beam search across tools.
  - Distributed multi-agent coordination.
  - UI/visualization beyond basic logs/metrics.

### 4. Stakeholders and reviewers

- Owner(s): @marek
- Reviewers/Maintainers: TBD
- Impacted users: API/CLI consumers; maintainers managing large changes via autonomous workflows

### 5. Requirements

- Functional requirements:
  - Define `Plan`, `PlanStep`, and `PlanState` with serialization for persistence and replay.
  - Support step kinds: tool call, model call, decision/evaluation, checkpoint/savepoint, compensation.
  - Policy-driven constraints: time/cost budgets, file scopes, network allowlist.
  - Interrupt/resume via session ID; export plan status and logs.
- Non-functional requirements:
  - Determinism: seedable randomness; record prompts/inputs for replay.
  - Reliability: atomic checkpoints; crash-safe resume.
  - Observability: metrics, structured logs, and per-step events.

### 6. High-level design (TBD)

- Components (proposed):
  - Planner: generates/updates `Plan` graphs (prompted + heuristic).
  - Executor: schedules steps, handles dependencies, and enforces policies.
  - Critic/Reflector: evaluates outputs, proposes fixes, and triggers alternate branches.
  - Checkpointer: persists plan state and artifacts; supports resume/replay.
  - Recovery Manager: classifies failures (transient/logic/policy) and selects strategies (retry/backoff/alternate/abort).
  - Safety Layer: validates diffs/commands against rules and scopes.
- Patterns: ReAct, Reflexion, ToT-lite; evaluation via self-consistency or heuristics.
- Persistence: store plan graphs and step logs in SQLite tables (TBD schema).

### 7. Scope and milestones (TBD)

- Milestone 0: Design spike and prototype plan/step types with minimal executor (linear ReAct).
- Milestone 1: Checkpointing and resume; deterministic replay of a recorded plan.
- Milestone 2: Failure classification and recovery strategies; compensation hooks for file edits.
- Milestone 3: Critique loop and shallow branching (ToT-lite) with budget controls.
- Milestone 4: Observability (metrics/logs) and CLI/HTTP surfaces for plan control.

### 8. High-level test interfaces (must-have)

- Unit-level:
  - `Plan`/`PlanStep` validation, serialization/deserialization, budget accounting.
  - Executor scheduling, timeouts, and retry/backoff logic.
- Integration-level:
  - Checkpoint/resume across process restart; deterministic replay.
  - File edit steps apply idempotent edits and compensation on failure.
- End-to-end:
  - Multi-step refactor with tests: plan → apply edits → run tests → recover on failure → converge or abort.

### 9. Risks and mitigations

- Runaway loops/cost blowups → budgets, max depth, watchdogs, and kill switches.
- Nondeterminism breaking replay → record all prompts/inputs/outputs with hashes; seed RNG.
- Partial failures corrupting workspace → diff-based edits, dry runs, and compensation steps.

### 10. Rollout and adoption

- Feature flags: `agent_planning` gate; off by default.
- Backwards compatibility: additive APIs; legacy flows unaffected.
- Documentation: developer guide for writing plan-aware tools and steps.

### 11. Open questions

- Exact schema for plan/step storage; how to efficiently query and paginate histories?
- How to bound ToT branching to stay within budgets while improving quality?
- Standardized compensation for non-file tools (e.g., git operations)?

### 12. Approval

- Approved by: <!-- maintainer(s) -->
- Approval date:


