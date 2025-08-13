-- system-level rules
CREATE TABLE IF NOT EXISTS rules (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_rules_name ON rules(name);

-- per-session included references
CREATE TABLE IF NOT EXISTS context_items (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('file','url')),
  key TEXT NOT NULL,
  content_excerpt TEXT NOT NULL,
  byte_len INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_context_items_session_created_at ON context_items(session_id, created_at);


