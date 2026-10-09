-- Provider observations are account-private. User intent has no cache-row FK.
CREATE TABLE runtime_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    revision INTEGER NOT NULL CHECK (revision >= 0),
    authorization_view INTEGER NOT NULL CHECK (authorization_view >= 0),
    log_floor INTEGER NOT NULL CHECK (log_floor >= 0)
);
INSERT INTO runtime_meta VALUES (1, 0, 0, 0);

CREATE TABLE accounts (
    id TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL,
    host TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    authorization_epoch INTEGER NOT NULL CHECK (authorization_epoch > 0),
    state TEXT NOT NULL CHECK (state IN ('active', 'auth_required', 'disconnected')),
    json TEXT NOT NULL CHECK (json_valid(json)),
    UNIQUE (provider, host, actor_id)
);

CREATE TABLE repositories (
    account_id TEXT NOT NULL,
    id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    full_name TEXT NOT NULL,
    selected INTEGER NOT NULL DEFAULT 0 CHECK (selected IN (0, 1)),
    json TEXT NOT NULL CHECK (json_valid(json)),
    PRIMARY KEY (account_id, id),
    UNIQUE (account_id, provider_id),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE RESTRICT
);
CREATE INDEX repository_order ON repositories(account_id, full_name, id);

CREATE TABLE items (
    account_id TEXT NOT NULL,
    id TEXT NOT NULL,
    repository_id TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('pull_request', 'issue', 'notification')),
    state TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    json TEXT NOT NULL CHECK (json_valid(json)),
    PRIMARY KEY (account_id, id),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE RESTRICT,
    FOREIGN KEY (account_id, repository_id) REFERENCES repositories(account_id, id) ON DELETE RESTRICT
);
CREATE INDEX item_list ON items(account_id, kind, updated_at DESC, id DESC);
CREATE INDEX repository_item_list ON items(account_id, repository_id, kind, state, updated_at DESC, id DESC);
CREATE VIRTUAL TABLE items_fts USING fts5(account_id UNINDEXED, id UNINDEXED, title, body);

CREATE TABLE sync_scopes (
    account_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    run_id TEXT NOT NULL,
    data_revision INTEGER NOT NULL DEFAULT 0 CHECK (data_revision >= 0),
    completed_run_id TEXT,
    next_cursor TEXT,
    etag TEXT,
    last_modified TEXT,
    access_denied INTEGER NOT NULL DEFAULT 0 CHECK (access_denied IN (0, 1)),
    coverage_json TEXT NOT NULL CHECK (json_valid(coverage_json)),
    sync_json TEXT NOT NULL CHECK (json_valid(sync_json)),
    PRIMARY KEY (account_id, scope),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE RESTRICT
);
CREATE TABLE scope_membership (
    account_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    last_seen_run TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    missing_count INTEGER NOT NULL DEFAULT 0 CHECK (missing_count >= 0),
    PRIMARY KEY (account_id, scope, entity_id),
    FOREIGN KEY (account_id, scope) REFERENCES sync_scopes(account_id, scope) ON DELETE CASCADE
);

CREATE TABLE drafts (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    body TEXT NOT NULL,
    generation INTEGER NOT NULL CHECK (generation > 0),
    PRIMARY KEY (account_id, subject_id),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE RESTRICT
);

CREATE TABLE change_log (
    revision INTEGER PRIMARY KEY CHECK (revision > 0),
    account_id TEXT NOT NULL,
    authorization_epoch INTEGER NOT NULL CHECK (authorization_epoch > 0),
    scope TEXT NOT NULL,
    reset INTEGER NOT NULL CHECK (reset IN (0, 1)),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE RESTRICT
);
CREATE INDEX change_account ON change_log(account_id, revision);
