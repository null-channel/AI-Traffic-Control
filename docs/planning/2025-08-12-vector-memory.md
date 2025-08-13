## Feature planning: Vector memory (per-session and global)

### 1. Summary

- Introduce vector memory to augment prompts with semantically relevant context. Maintain per-session “local memory” and an optional “global memory” for reusable knowledge. Persist embeddings and metadata in SQLite; build in-memory ANN indices on startup for fast retrieval.
- Origin: Follow-up to session persistence plan to support richer, durable context beyond recent messages.

### 2. Problem statement

- Current context building uses only recent-N messages and/or a running summary. This loses older but relevant information and lacks semantic recall across sessions.
- We need reliable retrieval of relevant snippets while keeping latency low and ensuring data is durable and scoped (per-session vs global).

### 3. Goals and non-goals

- Goals:
  - Provide a vector memory subsystem with two scopes: per-session and global.
  - Persist embeddings, chunks, and metadata in SQLite; rebuild in-memory ANN indices at startup.
  - Retrieval API to fetch top-K relevant items for a session, optionally including global memory.
  - Integrate with prompt building: recent-N messages + top-K retrieved items.
  - Basic chunking and deduplication strategy.
- Non-goals:
  - Distributed vector databases or multi-process shared indexes.
  - Complex RAG pipelines (tool-augmented browsing, citation graphs).
  - Advanced re-ranking (can be future work).

### 4. Stakeholders and reviewers

- Owner(s): @marek (maintainer)
- Reviewers/Maintainers: TBD
- Impacted users: Local client users seeking better recall in conversations and tasks

### 5. Requirements

- Functional requirements:
  - Upsert/delete vector entries associated with a session or global scope.
  - Query interface: given a session and query text, return top-K results from session memory and (optionally) global memory.
  - Prompt assembly: combine recent-N messages, optional running summary, and retrieved items.
  - Startup behavior: load metadata from SQLite and build in-memory ANN indices.
- Non-functional requirements:
  - Latency: retrieval suitable for interactive use (<50-100ms typical on modest corpora).
  - Durability: embeddings and metadata are persisted; indices can be rebuilt on startup.
  - Privacy/scope: session memory is isolated; global memory is shared by design.
  - Concurrency: safe concurrent reads/writes within a single process.

### 6. High-level design

- Components
  - `VectorStore` trait exposing upsert/delete/query APIs.
  - `SqliteBackedVectorStore` that persists rows in SQLite and maintains in-memory ANN indices.
  - Embedding provider abstraction (e.g., OpenAI-compatible or local model), configured via settings.
- Data model (SQLite)
  - `embeddings`:
    - `id` TEXT PRIMARY KEY (UUID)
    - `scope` TEXT NOT NULL CHECK(scope IN ('session','global'))
    - `session_id` TEXT NULL (required when scope='session')
    - `created_at` TEXT NOT NULL
    - `model` TEXT NOT NULL
    - `dim` INTEGER NOT NULL
    - `vector` BLOB NOT NULL (binary f32 array)
    - `content_hash` TEXT NOT NULL
    - `metadata_json` TEXT NOT NULL (includes source, offsets, tags)
    - INDEX on (`scope`, `session_id`)
  - `embedding_chunks` (optional, if splitting tracked separately) or encode in `metadata_json`.
- Retrieval flow
  - Query -> embed -> ANN search over session index (if present) and optional global index -> merge and return top-K.
- Prompt assembly
  - Use recent-N messages and/or running summary; append retrieved snippets (with minimal tokens) under a bounded budget.
- Indexing strategy
  - Build in-memory ANN indices (e.g., HNSW or IVF-like) from SQLite rows at startup; incremental updates on upsert/delete.
  - Periodic compaction or rebuild if needed.

### 7. Scope and milestones

- Milestone 1 (MVP):
  - Define `VectorStore` trait and `SqliteBackedVectorStore` skeleton.
  - SQLite schema and migrations for `embeddings`.
  - Basic chunking, hashing, and deduplication.
  - In-memory ANN index with add/remove and top-K query.
  - Unit and integration tests for upsert/query/delete and rebuild-on-startup.
- Milestone 2:
  - Embed-and-ingest pipeline hooks in handlers to capture useful content (messages, tool outputs, files).
  - Prompt builder integration: combine recent-N with retrieved top-K (configurable K, token budget).
  - Configurable embedding provider.
- Milestone 3:
  - Quality improvements: hybrid scoring (BM25 + vectors) or lightweight re-ranking; metrics on hit rates and latencies.

### 8. High-level test interfaces (must-have)

- Unit-level:
  - `VectorStore` APIs: upsert/delete/query behavior; scope and session isolation; dedup by `content_hash`.
  - ANN index: incremental add/remove; query correctness on small corpora.
- Integration-level:
  - Rebuild indices from SQLite on startup; results match pre-shutdown state.
  - End-to-end retrieve-then-assemble prompt respects token budget and ordering.
- System-level (optional for MVP):
  - Long-running stability test: concurrent upserts and queries; latency within target bounds.

### 9. Risks and mitigations

- Embedding cost/latency: batch or cache embeddings; allow configurable provider.
- Index growth and memory use: bound by retention policies, pruning, or on-disk index in future.
- Relevance quality: start simple; add re-ranking later.

### 10. Rollout and adoption

- Config: enable vector memory via settings; choose provider, N/K, token budgets.
- Migrations: add new tables; backward compatible with session persistence.
- Observability: counters for upserts/queries, index size, latencies.

### 11. Open questions

- Which ANN implementation/library to use for Rust? (HNSW vs alternatives)
- Default embedding model and vector dimension?
- What sources to ingest by default (messages only vs tool/file outputs)?

### 12. Approval

- Approved by: <!-- maintainer(s) -->
- Approval date:



