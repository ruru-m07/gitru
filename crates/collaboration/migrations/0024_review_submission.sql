-- Authored review text is retained independently from provider cache authority.
CREATE TABLE review_drafts (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL CHECK(octet_length(subject_id) BETWEEN 1 AND 1024),
    event TEXT NOT NULL CHECK(event IN ('comment','approve','request_changes')),
    body TEXT NOT NULL CHECK(octet_length(body)<=16384 AND instr(body,char(0))=0),
    generation INTEGER NOT NULL CHECK(generation>0),
    PRIMARY KEY(account_id,subject_id),
    FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER review_draft_identity BEFORE UPDATE OF account_id,subject_id ON review_drafts
BEGIN SELECT RAISE(ABORT,'immutable review draft identity'); END;
CREATE TRIGGER review_draft_retained BEFORE DELETE ON review_drafts
BEGIN SELECT RAISE(ABORT,'review drafts require explicit retention'); END;

CREATE TABLE review_draft_comments (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    comment_id TEXT NOT NULL CHECK(length(comment_id)=36),
    ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 24),
    body TEXT NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 16384 AND instr(body,char(0))=0),
    generation INTEGER NOT NULL CHECK(generation>0),
    PRIMARY KEY(account_id,subject_id,comment_id),
    UNIQUE(account_id,subject_id,ordinal),
    FOREIGN KEY(account_id,subject_id) REFERENCES review_drafts(account_id,subject_id) ON DELETE CASCADE
) STRICT;
CREATE TRIGGER review_draft_comment_identity BEFORE UPDATE OF account_id,subject_id,comment_id ON review_draft_comments
BEGIN SELECT RAISE(ABORT,'immutable review comment identity'); END;

-- Provider paths, commits and line ranges are purgeable cache authority.
CREATE TABLE review_draft_authority (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    draft_generation INTEGER NOT NULL CHECK(draft_generation>0),
    authorization_epoch TEXT NOT NULL CHECK(length(authorization_epoch) BETWEEN 1 AND 19),
    authorization_view TEXT NOT NULL CHECK(length(authorization_view) BETWEEN 1 AND 19),
    context_json TEXT NOT NULL CHECK(octet_length(context_json)<=16384),
    anchors_json TEXT NOT NULL CHECK(octet_length(anchors_json)<=65536),
    PRIMARY KEY(account_id,subject_id),
    FOREIGN KEY(account_id,subject_id) REFERENCES review_drafts(account_id,subject_id) ON DELETE CASCADE,
    FOREIGN KEY(account_id,subject_id) REFERENCES items(account_id,id) ON DELETE CASCADE
) STRICT;
CREATE TRIGGER review_draft_authority_identity BEFORE UPDATE OF account_id,subject_id ON review_draft_authority
BEGIN SELECT RAISE(ABORT,'immutable review authority identity'); END;

CREATE TABLE review_submissions (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    draft_generation INTEGER NOT NULL CHECK(draft_generation>0),
    command_id TEXT NOT NULL,
    submission_hash BLOB NOT NULL CHECK(typeof(submission_hash)='blob' AND length(submission_hash)=32),
    content_hash BLOB NOT NULL CHECK(typeof(content_hash)='blob' AND length(content_hash)=32),
    PRIMARY KEY(account_id,subject_id,draft_generation),
    UNIQUE(account_id,command_id),
    FOREIGN KEY(account_id,subject_id) REFERENCES review_drafts(account_id,subject_id) ON DELETE RESTRICT,
    FOREIGN KEY(account_id,command_id,submission_hash) REFERENCES commands(account_id,command_id,submission_hash) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER review_submission_bound BEFORE INSERT ON review_submissions
WHEN NOT EXISTS(
    SELECT 1 FROM review_drafts d JOIN commands c ON c.account_id=d.account_id
    WHERE d.account_id=NEW.account_id AND d.subject_id=NEW.subject_id
      AND d.generation=NEW.draft_generation AND c.command_id=NEW.command_id
      AND c.target_id=d.subject_id AND c.target_kind='pull_request'
      AND c.operation_kind='github.submit_review' AND c.payload_version=1 AND c.state='queued'
)
BEGIN SELECT RAISE(ABORT,'invalid review submission binding'); END;
CREATE TRIGGER review_submission_immutable BEFORE UPDATE ON review_submissions
BEGIN SELECT RAISE(ABORT,'immutable review submission'); END;
CREATE TRIGGER review_submission_retained BEFORE DELETE ON review_submissions
BEGIN SELECT RAISE(ABORT,'review submissions require explicit retention'); END;
CREATE INDEX review_submissions_history ON review_submissions(account_id,subject_id,draft_generation DESC);

-- A strong create response establishes one immutable command-to-review mapping.
CREATE TABLE review_resolutions (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    draft_generation INTEGER NOT NULL CHECK(draft_generation>0),
    command_id TEXT NOT NULL,
    provider_id TEXT NOT NULL CHECK(octet_length(provider_id) BETWEEN 1 AND 19),
    url TEXT NOT NULL CHECK(octet_length(url)<=2048),
    event TEXT NOT NULL CHECK(event IN ('comment','approve','request_changes')),
    provider_state TEXT NOT NULL CHECK(provider_state IN ('COMMENTED','APPROVED','CHANGES_REQUESTED')),
    reviewed_commit_oid TEXT NOT NULL CHECK(length(reviewed_commit_oid) IN (40,64) AND reviewed_commit_oid NOT GLOB '*[^0-9a-f]*'),
    submitted_at TEXT NOT NULL CHECK(octet_length(submitted_at) BETWEEN 20 AND 64),
    observed_at TEXT NOT NULL CHECK(octet_length(observed_at) BETWEEN 20 AND 64),
    accepted_ordinal INTEGER NOT NULL CHECK(accepted_ordinal BETWEEN 0 AND 127),
    PRIMARY KEY(account_id,command_id),
    UNIQUE(account_id,provider_id),
    UNIQUE(account_id,subject_id,draft_generation),
    UNIQUE(account_id,command_id,provider_id),
    FOREIGN KEY(account_id,subject_id,draft_generation) REFERENCES review_submissions(account_id,subject_id,draft_generation) ON DELETE RESTRICT,
    FOREIGN KEY(account_id,command_id,accepted_ordinal) REFERENCES command_evidence(account_id,command_id,ordinal) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER review_resolution_bound BEFORE INSERT ON review_resolutions
WHEN NOT EXISTS(
    SELECT 1 FROM command_evidence e
    WHERE e.account_id=NEW.account_id AND e.command_id=NEW.command_id
      AND e.ordinal=NEW.accepted_ordinal AND e.kind='github.review_accepted' AND e.version=1
      AND json_extract(CAST(e.payload AS TEXT),'$.preparation.command_id')=NEW.command_id
      AND json_extract(CAST(e.payload AS TEXT),'$.receipt.provider_id')=NEW.provider_id
)
BEGIN SELECT RAISE(ABORT,'invalid accepted review resolution'); END;
CREATE TRIGGER review_resolution_immutable BEFORE UPDATE ON review_resolutions
BEGIN SELECT RAISE(ABORT,'immutable review resolution'); END;
CREATE TRIGGER review_resolution_retained BEFORE DELETE ON review_resolutions
BEGIN SELECT RAISE(ABORT,'review resolutions require explicit retention'); END;

-- Terminal exact-ID readback is append-only and never upgrades the mapping row.
CREATE TABLE review_confirmations (
    account_id TEXT NOT NULL,
    command_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    confirmed_ordinal INTEGER NOT NULL CHECK(confirmed_ordinal BETWEEN 0 AND 127),
    confirmed_at TEXT NOT NULL CHECK(octet_length(confirmed_at) BETWEEN 20 AND 64),
    inline_comment_count INTEGER NOT NULL CHECK(inline_comment_count BETWEEN 0 AND 25),
    PRIMARY KEY(account_id,command_id),
    UNIQUE(account_id,provider_id),
    FOREIGN KEY(account_id,command_id,provider_id) REFERENCES review_resolutions(account_id,command_id,provider_id) ON DELETE RESTRICT,
    FOREIGN KEY(account_id,command_id,confirmed_ordinal) REFERENCES command_evidence(account_id,command_id,ordinal) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER review_confirmation_bound BEFORE INSERT ON review_confirmations
WHEN NOT EXISTS(
    SELECT 1 FROM command_evidence e
    WHERE e.account_id=NEW.account_id AND e.command_id=NEW.command_id
      AND e.ordinal=NEW.confirmed_ordinal AND e.kind='github.review_submitted' AND e.version=1
      AND json_extract(CAST(e.payload AS TEXT),'$.preparation.command_id')=NEW.command_id
      AND json_extract(CAST(e.payload AS TEXT),'$.receipt.provider_id')=NEW.provider_id
      AND json_array_length(json_extract(CAST(e.payload AS TEXT),'$.comments'))=NEW.inline_comment_count
)
BEGIN SELECT RAISE(ABORT,'invalid submitted review confirmation'); END;
CREATE TRIGGER review_confirmation_immutable BEFORE UPDATE ON review_confirmations
BEGIN SELECT RAISE(ABORT,'immutable review confirmation'); END;
CREATE TRIGGER review_confirmation_retained BEFORE DELETE ON review_confirmations
BEGIN SELECT RAISE(ABORT,'review confirmations require explicit retention'); END;

CREATE UNIQUE INDEX github_review_accepted_receipt_id
ON command_evidence(account_id,json_extract(CAST(payload AS TEXT),'$.receipt.provider_id'))
WHERE kind='github.review_accepted' AND version=1;
CREATE UNIQUE INDEX github_review_submitted_receipt_id
ON command_evidence(account_id,json_extract(CAST(payload AS TEXT),'$.receipt.provider_id'))
WHERE kind='github.review_submitted' AND version=1;
CREATE TRIGGER github_review_submitted_requires_resolution BEFORE INSERT ON command_evidence
WHEN NEW.kind='github.review_submitted' AND NEW.version=1 AND NOT EXISTS(
    SELECT 1 FROM review_resolutions r
    WHERE r.account_id=NEW.account_id AND r.command_id=NEW.command_id
      AND r.provider_id=json_extract(CAST(NEW.payload AS TEXT),'$.receipt.provider_id')
)
BEGIN SELECT RAISE(ABORT,'submitted review requires accepted resolution'); END;
