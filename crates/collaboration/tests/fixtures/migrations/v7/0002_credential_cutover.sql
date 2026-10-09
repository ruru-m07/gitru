-- Native credential metadata only. No token payload is stored in SQLite.
CREATE TABLE account_credentials (
    account_id TEXT PRIMARY KEY NOT NULL,
    credential_ref TEXT UNIQUE NOT NULL,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE RESTRICT
);

CREATE TABLE credential_cleanup (
    credential_ref TEXT PRIMARY KEY NOT NULL,
    -- Staging precedes account creation, so this is deliberately not a FK.
    account_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('staged', 'retired')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 20),
    next_retry_at INTEGER NOT NULL DEFAULT 0 CHECK (next_retry_at >= 0)
);
CREATE INDEX credential_cleanup_due ON credential_cleanup(next_retry_at, credential_ref);

-- Version one used the account ID directly as its vault reference.
INSERT INTO account_credentials(account_id, credential_ref)
SELECT id, id FROM accounts WHERE state <> 'disconnected';
-- Disconnect could have committed while a locked vault prevented removal.
INSERT INTO credential_cleanup(credential_ref, account_id, state)
SELECT id, id, 'retired' FROM accounts WHERE state = 'disconnected';
