-- Pull commits reuse durable detail demand but publish through a dedicated,
-- ordered generation cache. Existing read intent is copied byte-for-byte.
CREATE TABLE detail_demand_v11 (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL CHECK(facet IN ('body','comments','reviews','checks','participants','tasks','commits')),
    authorization_epoch TEXT NOT NULL,
    requested INTEGER NOT NULL DEFAULT 1 CHECK(requested IN (0,1)),
    PRIMARY KEY(account_id,subject_id,facet)
);
INSERT INTO detail_demand_v11(account_id,subject_id,facet,authorization_epoch,requested)
SELECT account_id,subject_id,facet,authorization_epoch,requested FROM detail_demand;
CREATE TABLE detail_demand_v11_guard(valid INTEGER NOT NULL CHECK(valid=1));
INSERT INTO detail_demand_v11_guard
SELECT
    (SELECT count(*) FROM detail_demand)=(SELECT count(*) FROM detail_demand_v11)
    AND NOT EXISTS(SELECT * FROM detail_demand EXCEPT SELECT * FROM detail_demand_v11);
DROP TABLE detail_demand;
ALTER TABLE detail_demand_v11 RENAME TO detail_demand;
INSERT INTO detail_demand_v11_guard SELECT NOT EXISTS(SELECT 1 FROM pragma_foreign_key_check);
DROP TABLE detail_demand_v11_guard;

CREATE TABLE pull_commit_generations (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    generation TEXT NOT NULL,
    run_id TEXT NOT NULL,
    authorization_epoch TEXT NOT NULL,
    authorization_view TEXT NOT NULL,
    binding_json TEXT NOT NULL CHECK(json_valid(binding_json)),
    context_json TEXT NOT NULL CHECK(json_valid(context_json)),
    source_json TEXT CHECK(source_json IS NULL OR json_valid(source_json)),
    provider_order TEXT CHECK(provider_order IN ('base_to_head','head_to_base')),
    expected_cursor TEXT,
    page_count INTEGER NOT NULL DEFAULT 0 CHECK(page_count BETWEEN 0 AND 20),
    row_count INTEGER NOT NULL DEFAULT 0 CHECK(row_count BETWEEN 0 AND 500),
    completeness_json TEXT CHECK(completeness_json IS NULL OR json_valid(completeness_json)),
    state TEXT NOT NULL CHECK(state IN ('staging','active','superseded')),
    created_revision INTEGER NOT NULL CHECK(created_revision >= 0),
    PRIMARY KEY(account_id,subject_id,generation),
    UNIQUE(account_id,subject_id,run_id),
    FOREIGN KEY(account_id,subject_id) REFERENCES items(account_id,id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX pull_commit_one_active_generation
ON pull_commit_generations(account_id,subject_id) WHERE state='active';
CREATE INDEX pull_commit_staging_cleanup
ON pull_commit_generations(state,account_id,subject_id,generation);

CREATE TABLE pull_commit_rows (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    generation TEXT NOT NULL,
    provider_sequence INTEGER NOT NULL CHECK(provider_sequence BETWEEN 0 AND 499),
    position INTEGER CHECK(position BETWEEN 0 AND 499),
    oid TEXT NOT NULL,
    json TEXT NOT NULL CHECK(json_valid(json)),
    PRIMARY KEY(account_id,subject_id,generation,provider_sequence),
    UNIQUE(account_id,subject_id,generation,oid),
    FOREIGN KEY(account_id,subject_id,generation)
        REFERENCES pull_commit_generations(account_id,subject_id,generation)
        ON DELETE CASCADE
);
CREATE UNIQUE INDEX pull_commit_generation_position
ON pull_commit_rows(account_id,subject_id,generation,position) WHERE position IS NOT NULL;

CREATE TABLE pull_commit_cursors (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    generation TEXT NOT NULL,
    cursor TEXT NOT NULL,
    PRIMARY KEY(account_id,subject_id,generation,cursor),
    FOREIGN KEY(account_id,subject_id,generation)
        REFERENCES pull_commit_generations(account_id,subject_id,generation)
        ON DELETE CASCADE
);

CREATE TABLE pull_commit_facets (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    authorization_epoch TEXT NOT NULL,
    authorization_view TEXT NOT NULL,
    current_context_json TEXT NOT NULL CHECK(json_valid(current_context_json)),
    active_generation TEXT,
    facet_revision TEXT,
    stale_at TEXT,
    PRIMARY KEY(account_id,subject_id),
    FOREIGN KEY(account_id,subject_id) REFERENCES items(account_id,id) ON DELETE CASCADE,
    CHECK((active_generation IS NULL)=(facet_revision IS NULL))
);

CREATE TRIGGER pull_commit_active_generation_insert
BEFORE INSERT ON pull_commit_facets
WHEN NEW.active_generation IS NOT NULL AND NOT EXISTS(
    SELECT 1 FROM pull_commit_generations g
    WHERE g.account_id=NEW.account_id AND g.subject_id=NEW.subject_id
      AND g.generation=NEW.active_generation AND g.state='active'
)
BEGIN
    SELECT RAISE(ABORT,'invalid active pull commit generation');
END;
CREATE TRIGGER pull_commit_active_generation_update
BEFORE UPDATE OF active_generation ON pull_commit_facets
WHEN NEW.active_generation IS NOT NULL AND NOT EXISTS(
    SELECT 1 FROM pull_commit_generations g
    WHERE g.account_id=NEW.account_id AND g.subject_id=NEW.subject_id
      AND g.generation=NEW.active_generation AND g.state='active'
)
BEGIN
    SELECT RAISE(ABORT,'invalid active pull commit generation');
END;

-- Active and unpublished rows both consume the local cache budget. A crash can
-- therefore never hide unaccounted staging bytes from maintenance telemetry.
CREATE TABLE pull_commit_retention (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL DEFAULT 'commits' CHECK(facet='commits'),
    logical_bytes INTEGER NOT NULL CHECK(logical_bytes >= 0),
    last_observed_revision INTEGER NOT NULL CHECK(last_observed_revision > 0),
    PRIMARY KEY(account_id,subject_id),
    FOREIGN KEY(account_id,subject_id) REFERENCES items(account_id,id) ON DELETE CASCADE
);
CREATE INDEX pull_commit_retention_order
ON pull_commit_retention(last_observed_revision,account_id,subject_id,facet);

CREATE TRIGGER pull_commit_retention_aggregate_insert
AFTER INSERT ON pull_commit_retention
BEGIN
    UPDATE cache_retention_state
    SET indexed_logical_bytes=indexed_logical_bytes+NEW.logical_bytes,
        indexed_facet_count=indexed_facet_count+1
    WHERE singleton=1;
END;
CREATE TRIGGER pull_commit_retention_aggregate_update
AFTER UPDATE OF logical_bytes ON pull_commit_retention
BEGIN
    UPDATE cache_retention_state
    SET indexed_logical_bytes=indexed_logical_bytes-OLD.logical_bytes+NEW.logical_bytes
    WHERE singleton=1;
END;
CREATE TRIGGER pull_commit_retention_aggregate_delete
AFTER DELETE ON pull_commit_retention
BEGIN
    UPDATE cache_retention_state
    SET indexed_logical_bytes=indexed_logical_bytes-OLD.logical_bytes,
        indexed_facet_count=indexed_facet_count-1
    WHERE singleton=1;
END;
