-- Extend the two facet CHECK constraints without disabling foreign keys.
-- Children reference the replacement parent, so dropping the old parent cannot
-- cascade away copied entries or Body metadata. SQLx owns this whole transaction.
CREATE TABLE detail_observations_v9 (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL CHECK(facet IN ('body','comments','reviews','checks','participants','tasks')),
    authorization_epoch TEXT NOT NULL,
    facet_revision TEXT NOT NULL,
    body_json TEXT NOT NULL,
    source_json TEXT NOT NULL,
    value_source_json TEXT,
    observed_state TEXT NOT NULL,
    stale_at TEXT,
    PRIMARY KEY(account_id,subject_id,facet)
);
CREATE TABLE detail_entries_v9 (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL,
    id TEXT NOT NULL,
    json TEXT NOT NULL,
    last_seen_run TEXT NOT NULL,
    PRIMARY KEY(account_id,subject_id,facet,id),
    FOREIGN KEY(account_id,subject_id,facet) REFERENCES detail_observations_v9(account_id,subject_id,facet) ON DELETE CASCADE
);
CREATE TABLE detail_resource_metadata_v9 (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL DEFAULT 'body' CHECK(facet='body'),
    authorization_epoch TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    source_json TEXT NOT NULL,
    PRIMARY KEY(account_id,subject_id),
    FOREIGN KEY(account_id,subject_id,facet) REFERENCES detail_observations_v9(account_id,subject_id,facet) ON DELETE CASCADE
);
CREATE TABLE detail_demand_v9 (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL CHECK(facet IN ('body','comments','reviews','checks','participants','tasks')),
    authorization_epoch TEXT NOT NULL,
    requested INTEGER NOT NULL DEFAULT 1 CHECK(requested IN (0,1)),
    PRIMARY KEY(account_id,subject_id,facet)
);

INSERT INTO detail_observations_v9(account_id,subject_id,facet,authorization_epoch,facet_revision,body_json,source_json,value_source_json,observed_state,stale_at)
SELECT account_id,subject_id,facet,authorization_epoch,facet_revision,body_json,source_json,value_source_json,observed_state,stale_at FROM detail_observations;
INSERT INTO detail_entries_v9(account_id,subject_id,facet,id,json,last_seen_run)
SELECT account_id,subject_id,facet,id,json,last_seen_run FROM detail_entries;
INSERT INTO detail_resource_metadata_v9(account_id,subject_id,facet,authorization_epoch,metadata_json,source_json)
SELECT account_id,subject_id,facet,authorization_epoch,metadata_json,source_json FROM detail_resource_metadata;
INSERT INTO detail_demand_v9(account_id,subject_id,facet,authorization_epoch,requested)
SELECT account_id,subject_id,facet,authorization_epoch,requested FROM detail_demand;

-- Fail inside the migration if a copy changed even one saved byte/value.
CREATE TABLE detail_facets_v9_guard(valid INTEGER NOT NULL CHECK(valid=1));
INSERT INTO detail_facets_v9_guard
SELECT
    (SELECT count(*) FROM detail_observations)=(SELECT count(*) FROM detail_observations_v9)
    AND NOT EXISTS(SELECT * FROM detail_observations EXCEPT SELECT * FROM detail_observations_v9)
    AND (SELECT count(*) FROM detail_entries)=(SELECT count(*) FROM detail_entries_v9)
    AND NOT EXISTS(SELECT * FROM detail_entries EXCEPT SELECT * FROM detail_entries_v9)
    AND (SELECT count(*) FROM detail_resource_metadata)=(SELECT count(*) FROM detail_resource_metadata_v9)
    AND NOT EXISTS(SELECT * FROM detail_resource_metadata EXCEPT SELECT * FROM detail_resource_metadata_v9)
    AND (SELECT count(*) FROM detail_demand)=(SELECT count(*) FROM detail_demand_v9)
    AND NOT EXISTS(SELECT * FROM detail_demand EXCEPT SELECT * FROM detail_demand_v9);

DROP TABLE detail_resource_metadata;
DROP TABLE detail_entries;
DROP TABLE detail_demand;
DROP TABLE detail_observations;

ALTER TABLE detail_observations_v9 RENAME TO detail_observations;
ALTER TABLE detail_entries_v9 RENAME TO detail_entries;
ALTER TABLE detail_resource_metadata_v9 RENAME TO detail_resource_metadata;
ALTER TABLE detail_demand_v9 RENAME TO detail_demand;

-- SQLite rewrites the replacement children's FK target when the parent is
-- renamed. Keep the Body-only metadata CHECK and account RESTRICT references.
INSERT INTO detail_facets_v9_guard
SELECT NOT EXISTS(SELECT 1 FROM pragma_foreign_key_check);
DROP TABLE detail_facets_v9_guard;
