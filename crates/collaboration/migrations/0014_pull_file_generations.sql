-- Files are a dedicated bounded facet. Keep existing read intent byte-identical.
CREATE TABLE detail_demand_v14 (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL CHECK(facet IN ('body','comments','reviews','checks','participants','tasks','commits','files')),
    authorization_epoch TEXT NOT NULL,
    requested INTEGER NOT NULL DEFAULT 1 CHECK(requested IN (0,1)),
    PRIMARY KEY(account_id,subject_id,facet)
);
INSERT INTO detail_demand_v14 SELECT * FROM detail_demand;
CREATE TABLE detail_demand_v14_guard(valid INTEGER NOT NULL CHECK(valid=1));
INSERT INTO detail_demand_v14_guard SELECT (SELECT count(*) FROM detail_demand)=(SELECT count(*) FROM detail_demand_v14) AND NOT EXISTS(SELECT * FROM detail_demand EXCEPT SELECT * FROM detail_demand_v14);
DROP TABLE detail_demand;
ALTER TABLE detail_demand_v14 RENAME TO detail_demand;
INSERT INTO detail_demand_v14_guard SELECT NOT EXISTS(SELECT 1 FROM pragma_foreign_key_check);
DROP TABLE detail_demand_v14_guard;

CREATE TABLE pull_file_generations (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    generation TEXT NOT NULL,
    run_id TEXT NOT NULL,
    authorization_epoch TEXT NOT NULL,
    authorization_view TEXT NOT NULL,
    binding_json TEXT NOT NULL CHECK(json_valid(binding_json)),
    context_json TEXT NOT NULL CHECK(json_valid(context_json)),
    source_json TEXT NOT NULL CHECK(json_valid(source_json)),
    expected_cursor TEXT,
    page_count INTEGER NOT NULL DEFAULT 0 CHECK(page_count BETWEEN 0 AND 30),
    row_count INTEGER NOT NULL DEFAULT 0 CHECK(row_count BETWEEN 0 AND 3000),
    completeness_json TEXT CHECK(completeness_json IS NULL OR json_valid(completeness_json)),
    state TEXT NOT NULL CHECK(state IN ('staging','active','superseded')),
    created_revision INTEGER NOT NULL CHECK(created_revision > 0),
    PRIMARY KEY(account_id,subject_id,generation),
    UNIQUE(account_id,subject_id,run_id),
    FOREIGN KEY(account_id,subject_id) REFERENCES items(account_id,id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX pull_file_one_active ON pull_file_generations(account_id,subject_id) WHERE state='active';
CREATE UNIQUE INDEX pull_file_one_staging ON pull_file_generations(account_id,subject_id) WHERE state='staging';
CREATE INDEX pull_file_cleanup ON pull_file_generations(state,account_id,subject_id,generation);

CREATE TABLE pull_file_rows (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    generation TEXT NOT NULL,
    file_key TEXT NOT NULL CHECK(octet_length(file_key) BETWEEN 1 AND 256),
    position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 2999),
    -- Empty sentinels represent missing sides; valid remote paths are nonempty.
    old_path TEXT NOT NULL,
    new_path TEXT NOT NULL,
    summary_json TEXT NOT NULL CHECK(json_valid(summary_json) AND octet_length(summary_json) <= 65536),
    PRIMARY KEY(account_id,subject_id,generation,file_key),
    UNIQUE(account_id,subject_id,generation,position),
    UNIQUE(account_id,subject_id,generation,old_path,new_path),
    FOREIGN KEY(account_id,subject_id,generation) REFERENCES pull_file_generations(account_id,subject_id,generation) ON DELETE CASCADE,
    CHECK(old_path<>'' OR new_path<>'')
);
CREATE TABLE pull_file_cursors (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    generation TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 29),
    cursor TEXT NOT NULL CHECK(octet_length(cursor) BETWEEN 1 AND 8192),
    PRIMARY KEY(account_id,subject_id,generation,ordinal),
    UNIQUE(account_id,subject_id,generation,cursor),
    FOREIGN KEY(account_id,subject_id,generation) REFERENCES pull_file_generations(account_id,subject_id,generation) ON DELETE CASCADE
);
CREATE TABLE pull_file_facets (
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
CREATE TRIGGER pull_file_active_insert BEFORE INSERT ON pull_file_facets
WHEN NEW.active_generation IS NOT NULL AND NOT EXISTS(SELECT 1 FROM pull_file_generations g WHERE g.account_id=NEW.account_id AND g.subject_id=NEW.subject_id AND g.generation=NEW.active_generation AND g.state='active')
BEGIN SELECT RAISE(ABORT,'invalid active file generation'); END;
CREATE TRIGGER pull_file_active_update BEFORE UPDATE OF active_generation ON pull_file_facets
WHEN NEW.active_generation IS NOT NULL AND NOT EXISTS(SELECT 1 FROM pull_file_generations g WHERE g.account_id=NEW.account_id AND g.subject_id=NEW.subject_id AND g.generation=NEW.active_generation AND g.state='active')
BEGIN SELECT RAISE(ABORT,'invalid active file generation'); END;

-- Text is separate from summaries/metadata and is never decoded by list reads.
CREATE TABLE pull_file_artifacts (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    generation TEXT NOT NULL,
    file_key TEXT NOT NULL,
    metadata_json TEXT NOT NULL CHECK(json_valid(metadata_json) AND octet_length(metadata_json) <= 65536),
    unified_text TEXT CHECK(unified_text IS NULL OR octet_length(unified_text) <= 4194304),
    logical_bytes INTEGER NOT NULL CHECK(logical_bytes >= 0),
    last_access_revision INTEGER NOT NULL CHECK(last_access_revision > 0),
    PRIMARY KEY(account_id,subject_id,generation,file_key),
    FOREIGN KEY(account_id,subject_id,generation,file_key) REFERENCES pull_file_rows(account_id,subject_id,generation,file_key) ON DELETE CASCADE
);
CREATE INDEX pull_file_artifact_lru ON pull_file_artifacts(last_access_revision,account_id,subject_id,generation,file_key);
-- Only native-owned, bounded objects can be referenced. This slice exposes no
-- blob registration IPC; provider-supplied opaque names cannot create authority.
CREATE TABLE pull_file_blob_objects (
    account_id TEXT NOT NULL,
    blob_id TEXT NOT NULL,
    oid TEXT,
    content_type TEXT NOT NULL,
    bytes BLOB NOT NULL CHECK(typeof(bytes)='blob' AND length(bytes)<=4194304),
    PRIMARY KEY(account_id,blob_id),
    FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE RESTRICT
);
CREATE TABLE pull_file_blob_references (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    generation TEXT NOT NULL,
    file_key TEXT NOT NULL,
    side TEXT NOT NULL CHECK(side IN ('old','new')),
    blob_id TEXT NOT NULL,
    PRIMARY KEY(account_id,subject_id,generation,file_key,side),
    FOREIGN KEY(account_id,subject_id,generation,file_key) REFERENCES pull_file_artifacts(account_id,subject_id,generation,file_key) ON DELETE CASCADE,
    FOREIGN KEY(account_id,blob_id) REFERENCES pull_file_blob_objects(account_id,blob_id) ON DELETE RESTRICT
);
CREATE INDEX pull_file_blob_reference_lookup ON pull_file_blob_references(account_id,blob_id);
CREATE TABLE pull_file_retention (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL DEFAULT 'files' CHECK(facet='files'),
    logical_bytes INTEGER NOT NULL CHECK(logical_bytes >= 0),
    last_observed_revision INTEGER NOT NULL CHECK(last_observed_revision > 0),
    PRIMARY KEY(account_id,subject_id),
    FOREIGN KEY(account_id,subject_id) REFERENCES items(account_id,id) ON DELETE CASCADE
);
CREATE INDEX pull_file_retention_order ON pull_file_retention(last_observed_revision,account_id,subject_id,facet);
CREATE TRIGGER pull_file_retention_insert AFTER INSERT ON pull_file_retention BEGIN
    UPDATE cache_retention_state SET indexed_logical_bytes=indexed_logical_bytes+NEW.logical_bytes,indexed_facet_count=indexed_facet_count+1 WHERE singleton=1;
END;
CREATE TRIGGER pull_file_retention_update AFTER UPDATE OF logical_bytes ON pull_file_retention BEGIN
    UPDATE cache_retention_state SET indexed_logical_bytes=indexed_logical_bytes-OLD.logical_bytes+NEW.logical_bytes WHERE singleton=1;
END;
CREATE TRIGGER pull_file_retention_delete AFTER DELETE ON pull_file_retention BEGIN
    UPDATE cache_retention_state SET indexed_logical_bytes=indexed_logical_bytes-OLD.logical_bytes,indexed_facet_count=indexed_facet_count-1 WHERE singleton=1;
END;
CREATE TABLE pull_file_artifact_retention_cursor (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    last_access_revision INTEGER,
    account_id TEXT,
    subject_id TEXT,
    generation TEXT,
    file_key TEXT,
    CHECK((last_access_revision IS NULL)=(account_id IS NULL) AND (account_id IS NULL)=(subject_id IS NULL) AND (subject_id IS NULL)=(generation IS NULL) AND (generation IS NULL)=(file_key IS NULL))
);
INSERT INTO pull_file_artifact_retention_cursor(singleton) VALUES(1);
