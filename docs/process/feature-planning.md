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


