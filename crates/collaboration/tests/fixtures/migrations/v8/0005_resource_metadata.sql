-- Rebuildable resource metadata shares Body access and atomic publication.
CREATE TABLE detail_resource_metadata (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    facet TEXT NOT NULL DEFAULT 'body' CHECK(facet='body'),
    authorization_epoch TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    source_json TEXT NOT NULL,
    PRIMARY KEY(account_id,subject_id),
    FOREIGN KEY(account_id,subject_id,facet) REFERENCES detail_observations(account_id,subject_id,facet) ON DELETE CASCADE
);
