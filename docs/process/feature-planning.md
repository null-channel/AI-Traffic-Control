## Feature planning process

This document outlines how we plan new features before implementation.

### When to plan

- Required for any net-new feature or material scope expansion
- Not required for small refactors and straightforward bug fixes

### Where to plan

- Create a new file in `docs/planning/` using the template `docs/templates/FEATURE_PLANNING_TEMPLATE.md`
- Filenames should follow `YYYY-MM-DD-feature-slug.md`

### Review and approval

- At least one maintainer must review the planning doc
- Approval is required before implementation begins

### Test-first mindset

- Every planning doc must define "High-level test interfaces" that describe what should be testable and how (unit, integration, e2e)
- Implementation is considered complete only when those interfaces have corresponding automated tests

### PR expectations

- Every PR must link to the planning doc
- If scope changes, update the plan and request re-review

### Interactive planning guidance

- Conversational loop:
  - Use a back-and-forth Q&A to rapidly clarify requirements, constraints, and acceptance criteria.
  - Group questions to minimize interruption; propose defaults when reasonable rather than blocking.
- Agent responsibilities:
  - State assumptions explicitly and continue with provisional defaults if low risk; flag them in the doc.
  - Offer 1–3 concrete options with a recommendation when choices arise (e.g., directory paths, APIs).
  - Keep the plan concise and scannable; link to canonical docs instead of duplicating long content.
- User responsibilities:
  - Confirm or adjust assumptions and defaults; highlight non-negotiables early.
  - Call out domain constraints (security, compliance, performance budgets) up front.
- Documentation expectations:
  - Capture decisions and assumptions within relevant sections (Summary, Requirements, Design).
  - Track unresolved items in "### 11. Open questions"; move them into decisions once resolved.
  - Ensure "### 8. High-level test interfaces" is co-authored and concrete before approval.
- Flow and timing:
  - Iterate in short passes: clarify → update plan → confirm → proceed.
  - Keep "### 12. Approval" numbering unchanged per repository policy.


