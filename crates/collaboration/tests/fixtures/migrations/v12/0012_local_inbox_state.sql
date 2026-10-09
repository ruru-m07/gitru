-- User-authored inbox intent is independent from provider observations.
CREATE TABLE local_inbox_projection (
    account_id TEXT PRIMARY KEY NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE RESTRICT
);

CREATE TABLE local_inbox_state (
    account_id TEXT NOT NULL,
    notification_id TEXT NOT NULL,
    disposition TEXT NOT NULL CHECK (disposition IN ('inbox', 'done')),
    bookmarked INTEGER NOT NULL DEFAULT 0 CHECK (bookmarked IN (0, 1)),
    snoozed_until TEXT,
    activity_updated_at TEXT NOT NULL,
    generation INTEGER NOT NULL CHECK (generation > 0),
    PRIMARY KEY (account_id, notification_id),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE RESTRICT,
    CHECK (disposition = 'inbox' OR snoozed_until IS NULL)
);
CREATE INDEX local_inbox_deadlines
    ON local_inbox_state(account_id, snoozed_until)
    WHERE snoozed_until IS NOT NULL;
