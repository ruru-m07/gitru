-- Review summaries and review threads page independently. Replace the bounded
-- detail tables without changing any previously saved row, demand bit, or
-- retention accounting. Pull files and commits keep their dedicated stores.
CREATE TABLE detail_observations_v18 (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL CHECK(facet IN ('body','comments','reviews','review_summaries','review_threads','checks','participants','tasks')),
    authorization_epoch TEXT NOT NULL,
    facet_revision TEXT NOT NULL,
    body_json TEXT NOT NULL,
    source_json TEXT NOT NULL,
    value_source_json TEXT,
    observed_state TEXT NOT NULL,
    stale_at TEXT,
    PRIMARY KEY(account_id,subject_id,facet)
);
CREATE TABLE detail_entries_v18 (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL,
    id TEXT NOT NULL,
    json TEXT NOT NULL,
    last_seen_run TEXT NOT NULL,
    PRIMARY KEY(account_id,subject_id,facet,id),
    FOREIGN KEY(account_id,subject_id,facet) REFERENCES detail_observations_v18(account_id,subject_id,facet) ON DELETE CASCADE
);
CREATE TABLE detail_resource_metadata_v18 (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL DEFAULT 'body' CHECK(facet='body'),
    authorization_epoch TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    source_json TEXT NOT NULL,
    PRIMARY KEY(account_id,subject_id),
    FOREIGN KEY(account_id,subject_id,facet) REFERENCES detail_observations_v18(account_id,subject_id,facet) ON DELETE CASCADE
);
CREATE TABLE detail_demand_v18 (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL CHECK(facet IN ('body','comments','reviews','review_summaries','review_threads','checks','participants','tasks','commits','files')),
    authorization_epoch TEXT NOT NULL,
    requested INTEGER NOT NULL DEFAULT 1 CHECK(requested IN (0,1)),
    PRIMARY KEY(account_id,subject_id,facet)
);
CREATE TABLE cache_retention_entries_v18 (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL,
    logical_bytes INTEGER NOT NULL CHECK(logical_bytes >= 0),
    last_observed_revision INTEGER NOT NULL CHECK(last_observed_revision > 0),
    PRIMARY KEY(account_id,subject_id,facet),
    FOREIGN KEY(account_id,subject_id,facet) REFERENCES detail_observations_v18(account_id,subject_id,facet) ON DELETE CASCADE
);

INSERT INTO detail_observations_v18 SELECT * FROM detail_observations;
INSERT INTO detail_entries_v18 SELECT * FROM detail_entries;
INSERT INTO detail_resource_metadata_v18 SELECT * FROM detail_resource_metadata;
INSERT INTO detail_demand_v18 SELECT * FROM detail_demand;
INSERT INTO cache_retention_entries_v18 SELECT * FROM cache_retention_entries;

CREATE TABLE detail_facets_v18_guard(valid INTEGER NOT NULL CHECK(valid=1));
INSERT INTO detail_facets_v18_guard
SELECT
    (SELECT count(*) FROM detail_observations)=(SELECT count(*) FROM detail_observations_v18)
    AND NOT EXISTS(SELECT * FROM detail_observations EXCEPT SELECT * FROM detail_observations_v18)
    AND (SELECT count(*) FROM detail_entries)=(SELECT count(*) FROM detail_entries_v18)
    AND NOT EXISTS(SELECT * FROM detail_entries EXCEPT SELECT * FROM detail_entries_v18)
    AND (SELECT count(*) FROM detail_resource_metadata)=(SELECT count(*) FROM detail_resource_metadata_v18)
    AND NOT EXISTS(SELECT * FROM detail_resource_metadata EXCEPT SELECT * FROM detail_resource_metadata_v18)
    AND (SELECT count(*) FROM detail_demand)=(SELECT count(*) FROM detail_demand_v18)
    AND NOT EXISTS(SELECT * FROM detail_demand EXCEPT SELECT * FROM detail_demand_v18)
    AND (SELECT count(*) FROM cache_retention_entries)=(SELECT count(*) FROM cache_retention_entries_v18)
    AND NOT EXISTS(SELECT * FROM cache_retention_entries EXCEPT SELECT * FROM cache_retention_entries_v18);

-- Avoid changing aggregate retention counters while replacing their exact
-- ledger rows. The counters remain byte-for-byte valid after the rename.
DROP TRIGGER cache_retention_entries_aggregate_insert;
DROP TRIGGER cache_retention_entries_aggregate_update;
DROP TRIGGER cache_retention_entries_aggregate_delete;
DROP TABLE cache_retention_entries;
DROP TABLE detail_resource_metadata;
DROP TABLE detail_entries;
DROP TABLE detail_demand;
DROP TABLE detail_observations;

ALTER TABLE detail_observations_v18 RENAME TO detail_observations;
ALTER TABLE detail_entries_v18 RENAME TO detail_entries;
ALTER TABLE detail_resource_metadata_v18 RENAME TO detail_resource_metadata;
ALTER TABLE detail_demand_v18 RENAME TO detail_demand;
ALTER TABLE cache_retention_entries_v18 RENAME TO cache_retention_entries;
CREATE INDEX cache_retention_eviction_order ON cache_retention_entries(last_observed_revision,account_id,subject_id,facet);
CREATE TRIGGER cache_retention_entries_aggregate_insert AFTER INSERT ON cache_retention_entries BEGIN
    UPDATE cache_retention_state SET indexed_logical_bytes=indexed_logical_bytes+NEW.logical_bytes,indexed_facet_count=indexed_facet_count+1 WHERE singleton=1;
END;
CREATE TRIGGER cache_retention_entries_aggregate_update AFTER UPDATE OF logical_bytes ON cache_retention_entries BEGIN
    UPDATE cache_retention_state SET indexed_logical_bytes=indexed_logical_bytes-OLD.logical_bytes+NEW.logical_bytes WHERE singleton=1;
END;
CREATE TRIGGER cache_retention_entries_aggregate_delete AFTER DELETE ON cache_retention_entries BEGIN
    UPDATE cache_retention_state SET indexed_logical_bytes=indexed_logical_bytes-OLD.logical_bytes,indexed_facet_count=indexed_facet_count-1 WHERE singleton=1;
END;

INSERT INTO detail_facets_v18_guard SELECT NOT EXISTS(SELECT 1 FROM pragma_foreign_key_check);
DROP TABLE detail_facets_v18_guard;
