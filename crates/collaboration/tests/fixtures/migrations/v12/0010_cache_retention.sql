-- Pins are durable authored state. Reuse the canonical identity primary key so
-- opening an existing database does not build an unbounded secondary index.
CREATE TABLE cache_pins (
    account_id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('pull_request','issue')),
    pinned_revision INTEGER NOT NULL CHECK(pinned_revision > 0),
    PRIMARY KEY(account_id,instance_id,entity_id),
    FOREIGN KEY(account_id,instance_id,entity_id)
        REFERENCES resource_identities(account_id,instance_id,entity_id)
        ON DELETE RESTRICT
);

-- The parent primary key omits kind. Point lookups keep the label exact without
-- scanning or adding an O(N) index during this forward-only migration.
CREATE TRIGGER cache_pins_kind_insert
BEFORE INSERT ON cache_pins
WHEN NOT EXISTS(
    SELECT 1 FROM resource_identities
    WHERE account_id=NEW.account_id
      AND instance_id=NEW.instance_id
      AND entity_id=NEW.entity_id
      AND kind=NEW.kind
)
BEGIN
    SELECT RAISE(ABORT,'cache pin identity kind mismatch');
END;

CREATE TRIGGER cache_pins_kind_update
BEFORE UPDATE OF account_id,instance_id,entity_id,kind ON cache_pins
WHEN NOT EXISTS(
    SELECT 1 FROM resource_identities
    WHERE account_id=NEW.account_id
      AND instance_id=NEW.instance_id
      AND entity_id=NEW.entity_id
      AND kind=NEW.kind
)
BEGIN
    SELECT RAISE(ABORT,'cache pin identity kind mismatch');
END;

CREATE TRIGGER resource_identity_pinned_kind_update
BEFORE UPDATE OF kind ON resource_identities
WHEN NEW.kind<>OLD.kind AND EXISTS(
    SELECT 1 FROM cache_pins
    WHERE account_id=OLD.account_id
      AND instance_id=OLD.instance_id
      AND entity_id=OLD.entity_id
)
BEGIN
    SELECT RAISE(ABORT,'pinned identity kind is immutable');
END;

-- This ledger starts empty. Runtime maintenance indexes historical detail
-- observations in bounded batches; opening an existing database never performs
-- an unbounded payload scan in the migration transaction.
CREATE TABLE cache_retention_entries (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL,
    logical_bytes INTEGER NOT NULL CHECK(logical_bytes >= 0),
    last_observed_revision INTEGER NOT NULL CHECK(last_observed_revision > 0),
    PRIMARY KEY(account_id,subject_id,facet),
    FOREIGN KEY(account_id,subject_id,facet)
        REFERENCES detail_observations(account_id,subject_id,facet)
        ON DELETE CASCADE
);
CREATE INDEX cache_retention_eviction_order
ON cache_retention_entries(last_observed_revision,account_id,subject_id,facet);

-- Cursors are values rather than foreign keys: the eviction cursor must remain
-- valid after its ledger row cascades away. Null denotes the start of either
-- keyset scan. A completed historical index has no residual index cursor.
CREATE TABLE cache_retention_state (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    index_complete INTEGER NOT NULL DEFAULT 0 CHECK(index_complete IN (0,1)),
    index_cursor_account_id TEXT,
    index_cursor_subject_id TEXT,
    index_cursor_facet TEXT,
    eviction_cursor_revision INTEGER CHECK(eviction_cursor_revision > 0),
    eviction_cursor_account_id TEXT,
    eviction_cursor_subject_id TEXT,
    eviction_cursor_facet TEXT,
    indexed_logical_bytes INTEGER NOT NULL DEFAULT 0 CHECK(indexed_logical_bytes >= 0),
    indexed_facet_count INTEGER NOT NULL DEFAULT 0 CHECK(indexed_facet_count >= 0),
    CHECK(
        (index_cursor_account_id IS NULL
            AND index_cursor_subject_id IS NULL
            AND index_cursor_facet IS NULL)
        OR
        (index_cursor_account_id IS NOT NULL
            AND index_cursor_subject_id IS NOT NULL
            AND index_cursor_facet IS NOT NULL)
    ),
    CHECK(
        index_complete = 0
        OR (index_cursor_account_id IS NULL
            AND index_cursor_subject_id IS NULL
            AND index_cursor_facet IS NULL)
    ),
    CHECK(
        (eviction_cursor_revision IS NULL
            AND eviction_cursor_account_id IS NULL
            AND eviction_cursor_subject_id IS NULL
            AND eviction_cursor_facet IS NULL)
        OR
        (eviction_cursor_revision IS NOT NULL
            AND eviction_cursor_account_id IS NOT NULL
            AND eviction_cursor_subject_id IS NOT NULL
            AND eviction_cursor_facet IS NOT NULL)
    )
);
INSERT INTO cache_retention_state(singleton) VALUES(1);

-- Keep usage reads O(1). These triggers also run for foreign-key cascades from
-- detail_observations, so cache resets and retention eviction cannot leave the
-- aggregate ahead of the ledger.
CREATE TRIGGER cache_retention_entries_aggregate_insert
AFTER INSERT ON cache_retention_entries
BEGIN
    UPDATE cache_retention_state
    SET indexed_logical_bytes=indexed_logical_bytes+NEW.logical_bytes,
        indexed_facet_count=indexed_facet_count+1
    WHERE singleton=1;
END;

CREATE TRIGGER cache_retention_entries_aggregate_update
AFTER UPDATE OF logical_bytes ON cache_retention_entries
BEGIN
    UPDATE cache_retention_state
    SET indexed_logical_bytes=indexed_logical_bytes-OLD.logical_bytes+NEW.logical_bytes
    WHERE singleton=1;
END;

CREATE TRIGGER cache_retention_entries_aggregate_delete
AFTER DELETE ON cache_retention_entries
BEGIN
    UPDATE cache_retention_state
    SET indexed_logical_bytes=indexed_logical_bytes-OLD.logical_bytes,
        indexed_facet_count=indexed_facet_count-1
    WHERE singleton=1;
END;
