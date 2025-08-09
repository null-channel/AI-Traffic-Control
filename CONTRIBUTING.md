## Contribution process

This repository follows a simple, explicit working agreement to keep feature work predictable and collaborative.

### Feature work: planning first

Before implementing any new feature, create a planning document and collaborate on it until the scope is clear and testability is defined.

- Location for plans: `docs/planning/`
- Template to use: `docs/templates/FEATURE_PLANNING_TEMPLATE.md`
- Recommended filename: `YYYY-MM-DD-feature-slug.md` (e.g., `2025-08-09-flight-scheduling-ui.md`)

#### Required steps

1. Open a feature issue using the "Feature request" issue template.
2. Create a new planning doc from the template in `docs/planning/` and link it in the issue.
3. Collaborate to finalize:
   - Requirements (functional and non-functional)
   - High-level architecture/design
   - Milestones/scope for initial PR(s)
   - High-level test interfaces to be implemented (what should be testable and how at a high level)
4. Only start implementation once the planning doc has been reviewed and approved by at least one maintainer.
5. Every PR must link the planning doc and follow the plan. The PR template includes a checklist for this.

### Definition of Done (for a feature)

- Implementation aligns with the approved planning doc
- Tests exist according to the "High-level test interfaces" defined in the plan
- Documentation updated as needed
- Backward compatibility, migration, and rollout considerations addressed (if relevant)

### Where things live

- Planning docs: `docs/planning/`
- Planning template: `docs/templates/FEATURE_PLANNING_TEMPLATE.md`
- Process guide: `docs/process/feature-planning.md`

### Notes

- Small refactors and bug fixes do not require a planning doc, but still require clear PR descriptions and tests.
- If the scope changes materially mid-implementation, update the planning doc and have it re-reviewed.


