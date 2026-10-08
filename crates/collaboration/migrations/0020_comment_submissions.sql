-- Public comment authorship is intentionally separate from private notes.
CREATE TABLE comment_drafts (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL CHECK(octet_length(subject_id) BETWEEN 1 AND 1024),
    body TEXT NOT NULL CHECK(octet_length(body)<=16384),
    generation INTEGER NOT NULL CHECK(generation>0),
    PRIMARY KEY(account_id,subject_id),
    FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER comment_draft_identity BEFORE UPDATE OF account_id,subject_id ON comment_drafts
BEGIN SELECT RAISE(ABORT,'immutable comment draft identity'); END;
CREATE TRIGGER comment_draft_retained BEFORE DELETE ON comment_drafts
BEGIN SELECT RAISE(ABORT,'comment drafts require explicit retention'); END;
CREATE TABLE comment_submissions (
    account_id TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    draft_generation INTEGER NOT NULL CHECK(draft_generation>0),
    command_id TEXT NOT NULL,
    submission_hash BLOB NOT NULL CHECK(typeof(submission_hash)='blob' AND length(submission_hash)=32),
    body_hash BLOB NOT NULL CHECK(typeof(body_hash)='blob' AND length(body_hash)=32),
    PRIMARY KEY(account_id,subject_id,draft_generation),
    UNIQUE(account_id,command_id),
    FOREIGN KEY(account_id,subject_id) REFERENCES comment_drafts(account_id,subject_id) ON DELETE RESTRICT,
    FOREIGN KEY(account_id,command_id,submission_hash) REFERENCES commands(account_id,command_id,submission_hash) ON DELETE RESTRICT
) STRICT;
CREATE TRIGGER comment_submission_bound BEFORE INSERT ON comment_submissions
WHEN NOT EXISTS(SELECT 1 FROM comment_drafts d JOIN commands c ON c.account_id=d.account_id WHERE d.account_id=NEW.account_id AND d.subject_id=NEW.subject_id AND d.generation=NEW.draft_generation AND c.command_id=NEW.command_id AND c.target_id=NEW.subject_id AND c.target_kind IN ('issue','pull_request') AND c.operation_kind='github.create_comment' AND c.payload_version=1 AND c.state='queued')
BEGIN SELECT RAISE(ABORT,'invalid comment submission binding'); END;
CREATE TRIGGER comment_submission_immutable BEFORE UPDATE ON comment_submissions
BEGIN SELECT RAISE(ABORT,'immutable comment submission'); END;
CREATE TRIGGER comment_submission_retained BEFORE DELETE ON comment_submissions
BEGIN SELECT RAISE(ABORT,'comment submissions require explicit retention'); END;
CREATE INDEX comment_submissions_history ON comment_submissions(account_id,subject_id,draft_generation DESC);
-- A creation receipt cannot claim the same canonical GitHub comment twice.
CREATE UNIQUE INDEX comment_created_receipt_id ON command_evidence(
    account_id,json_extract(CAST(payload AS TEXT),'$.receipt.provider_id')
) WHERE kind='github.comment_created' AND version=1;
