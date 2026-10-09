-- Historical v1 data, authored independently of the current Rust models.
-- Provider observations overlap across accounts; private intent has no item FK.
CREATE TABLE _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    success BOOLEAN NOT NULL,
    checksum BLOB NOT NULL,
    execution_time BIGINT NOT NULL
);
INSERT INTO _sqlx_migrations VALUES (
    1, 'local collaboration', '2026-10-03 00:00:00', 1,
    x'a0b4863d56b1620dae93b13df7ef2b38074c3ac5a5d5bf639b01899204cb61f6796ba9fb37bfd3b085f79e928e475e3d', 0
);

UPDATE runtime_meta SET revision=9004, authorization_view=11, log_floor=8998;
INSERT INTO accounts VALUES
    ('a', 'github', 'github.com', '100', 17, 'active',
     '{"id":"a","provider":"github","host":"github.com","actor_id":"100","login":"alice","display_name":"Alice","authorization_epoch":"17","state":"active","notifications_supported":true}'),
    ('b', 'github', 'github.com', '101', 29, 'active',
     '{"id":"b","provider":"github","host":"github.com","actor_id":"101","login":"bob","display_name":null,"authorization_epoch":"29","state":"active","notifications_supported":true}'),
    ('c', 'github', 'github.com', '102', 41, 'disconnected',
     '{"id":"c","provider":"github","host":"github.com","actor_id":"102","login":"carol","display_name":null,"authorization_epoch":"41","state":"disconnected","notifications_supported":false}'),
    ('d', 'github', 'github.com', '103', 53, 'auth_required',
     '{"id":"d","provider":"github","host":"github.com","actor_id":"103","login":"dave","display_name":null,"authorization_epoch":"53","state":"auth_required","notifications_supported":true}');

INSERT INTO repositories VALUES
    ('a', 'repository-42', '42', 'fixture/project', 1,
     '{"id":"repository-42","account_id":"a","provider_id":"42","full_name":"fixture/project","name":"project","web_url":"https://github.com/fixture/project","description":"Alice private observation","default_branch":"main","selected":true}'),
    ('b', 'repository-42', '42', 'fixture/project', 1,
     '{"id":"repository-42","account_id":"b","provider_id":"42","full_name":"fixture/project","name":"project","web_url":"https://github.com/fixture/project","description":"Bob private observation","default_branch":"develop","selected":true}');

INSERT INTO items VALUES
    ('a', 'pull-request-67', 'repository-42', 'pull_request', 'open', '2026-10-02T12:00:00Z',
     '{"id":"pull-request-67","account_id":"a","repository_id":"repository-42","provider_id":"67","kind":"pull_request","number":"67","title":"AlphaOnly private change","body":"Alice cached body","body_omitted":false,"author":"alice","web_url":"https://github.com/fixture/project/pull/67","state":"open","updated_at":"2026-10-02T12:00:00Z","head_oid":"alice-head","is_draft":false,"reason":null,"unread":null}'),
    ('b', 'pull-request-67', 'repository-42', 'pull_request', 'open', '2026-10-02T13:00:00Z',
     '{"id":"pull-request-67","account_id":"b","repository_id":"repository-42","provider_id":"67","kind":"pull_request","number":"67","title":"BetaOnly private change","body":"Bob cached body","body_omitted":false,"author":"bob","web_url":"https://github.com/fixture/project/pull/67","state":"open","updated_at":"2026-10-02T13:00:00Z","head_oid":"bob-head","is_draft":true,"reason":null,"unread":null}'),
    ('a', 'issue-68', 'repository-42', 'issue', 'open', '2026-10-02T14:00:00Z',
     '{"id":"issue-68","account_id":"a","repository_id":"repository-42","provider_id":"68","kind":"issue","number":"68","title":"DeniedOnly private issue","body":"Revoked observation","body_omitted":false,"author":"alice","web_url":null,"state":"open","updated_at":"2026-10-02T14:00:00Z","head_oid":null,"is_draft":null,"reason":null,"unread":null}');
INSERT INTO items_fts(account_id,id,title,body) VALUES
    ('a', 'pull-request-67', 'AlphaOnly private change', 'Alice cached body'),
    ('b', 'pull-request-67', 'BetaOnly private change', 'Bob cached body'),
    ('a', 'issue-68', 'DeniedOnly private issue', 'Revoked observation');

INSERT INTO sync_scopes VALUES
    ('a','repositories','discovery-a',31,'discovery-a',NULL,'discovery-validator-a',NULL,0,
     '{"state":"complete","validated_at":"2026-10-02T12:00:00Z","remote_has_more":false}',
     '{"state":"idle","last_success_at":"2026-10-02T12:00:00Z","next_retry_at":null,"error":null}'),
    ('b','repositories','discovery-b',32,'discovery-b',NULL,'discovery-validator-b',NULL,0,
     '{"state":"complete","validated_at":"2026-10-02T12:00:00Z","remote_has_more":false}',
     '{"state":"idle","last_success_at":"2026-10-02T12:00:00Z","next_retry_at":null,"error":null}'),
    ('a','repo:repository-42:pull_request','partial-a',33,NULL,'page-2-a','private-validator-a','Fri, 02 Oct 2026 12:00:00 GMT',0,
     '{"state":"partial","validated_at":"2026-10-02T12:00:00Z","remote_has_more":true}',
     '{"state":"offline","last_success_at":"2026-10-02T12:00:00Z","next_retry_at":null,"error":null}'),
    ('b','repo:repository-42:pull_request','complete-b',34,'complete-b',NULL,'private-validator-b',NULL,0,
     '{"state":"complete","validated_at":"2026-10-02T13:00:00Z","remote_has_more":false}',
     '{"state":"idle","last_success_at":"2026-10-02T13:00:00Z","next_retry_at":null,"error":null}'),
    ('a','repo:repository-42:issue','denied-a',35,NULL,NULL,NULL,NULL,1,
     '{"state":"missing","validated_at":null,"remote_has_more":false}',
     '{"state":"error","last_success_at":null,"next_retry_at":null,"error":{"code":"permission_denied","message":"Access to this scope was revoked","retry_after_seconds":null}}');
INSERT INTO scope_membership VALUES
    ('a','repositories','repository-42','discovery-a',1,0),
    ('b','repositories','repository-42','discovery-b',1,0),
    ('a','repo:repository-42:pull_request','pull-request-67','partial-a',1,0),
    ('b','repo:repository-42:pull_request','pull-request-67','complete-b',1,0),
    ('a','repo:repository-42:issue','issue-68','denied-a',1,0);

INSERT INTO drafts VALUES
    ('a','pull-request-67','Alice unsent draft with a newline
and Unicode: café 🦀',37),
    ('b','pull-request-67','Bob independent unsent draft',51),
    ('c','missing-subject','Carol draft after disconnect',63),
    ('d','missing-subject','Dave draft awaiting reconnection',79);
INSERT INTO change_log VALUES
    (8999,'a',16,'account',0),
    (9000,'a',17,'repositories',0),
    (9001,'b',29,'repo:repository-42:pull_request',0),
    (9002,'c',41,'drafts',0),
    (9003,'a',17,'repo:repository-42:issue',1),
    (9004,'d',53,'drafts',0);
