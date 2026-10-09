-- Identity metadata is durable across rebuildable provider projection cutovers.
CREATE TABLE provider_instances (
    id TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL,
    base_url TEXT NOT NULL,
    UNIQUE(provider, base_url)
);
CREATE TABLE account_instances (
    account_id TEXT PRIMARY KEY NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    instance_id TEXT NOT NULL REFERENCES provider_instances(id) ON DELETE RESTRICT,
    UNIQUE(account_id, instance_id)
);
-- v1/v2 use host-only installation metadata; Rust validates every binding on read.
INSERT INTO provider_instances(id,provider,base_url)
SELECT DISTINCT provider||':https://'||host||'/',provider,'https://'||host||'/' FROM accounts;
INSERT INTO account_instances(account_id,instance_id)
SELECT id,provider||':https://'||host||'/' FROM accounts;

CREATE TABLE resource_identities (
    account_id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('repository','pull_request','issue','notification')),
    provider_id TEXT NOT NULL,
    repository_provider_id TEXT NOT NULL DEFAULT '',
    number TEXT,
    PRIMARY KEY(account_id,instance_id,entity_id),
    UNIQUE(account_id,instance_id,kind,provider_id),
    FOREIGN KEY(account_id,instance_id) REFERENCES account_instances(account_id,instance_id) ON DELETE RESTRICT
);
CREATE INDEX identity_repository_number ON resource_identities(account_id,instance_id,kind,repository_provider_id,number);
CREATE TABLE resource_aliases (
    account_id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    alias_kind TEXT NOT NULL,
    value TEXT NOT NULL,
    repository_path TEXT NOT NULL DEFAULT '',
    entity_id TEXT NOT NULL,
    PRIMARY KEY(account_id,instance_id,kind,alias_kind,value,repository_path,entity_id),
    FOREIGN KEY(account_id,instance_id,entity_id) REFERENCES resource_identities(account_id,instance_id,entity_id) ON DELETE RESTRICT
);
CREATE TABLE pending_endpoint_aliases (
    account_id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    repository_provider_id TEXT NOT NULL,
    number TEXT NOT NULL,
    native_identity TEXT NOT NULL,
    web_url TEXT,
    PRIMARY KEY(account_id,instance_id,kind,repository_provider_id,number,native_identity),
    FOREIGN KEY(account_id,instance_id) REFERENCES account_instances(account_id,instance_id) ON DELETE RESTRICT
);

INSERT INTO resource_identities(account_id,instance_id,entity_id,kind,provider_id)
SELECT r.account_id,a.instance_id,r.id,'repository',r.provider_id FROM repositories r JOIN account_instances a ON a.account_id=r.account_id;
INSERT INTO resource_identities(account_id,instance_id,entity_id,kind,provider_id,repository_provider_id,number)
SELECT i.account_id,a.instance_id,i.id,i.kind,json_extract(i.json,'$.provider_id'),coalesce(r.provider_id,''),json_extract(i.json,'$.number')
FROM items i JOIN account_instances a ON a.account_id=i.account_id LEFT JOIN repositories r ON r.account_id=i.account_id AND r.id=i.repository_id;
INSERT INTO resource_aliases(account_id,instance_id,kind,alias_kind,value,entity_id)
SELECT account_id,instance_id,kind,'canonical',entity_id,entity_id FROM resource_identities;
INSERT INTO resource_aliases(account_id,instance_id,kind,alias_kind,value,entity_id)
SELECT account_id,instance_id,kind,'native',kind||':'||provider_id,entity_id FROM resource_identities;
INSERT INTO resource_aliases(account_id,instance_id,kind,alias_kind,value,entity_id)
SELECT r.account_id,a.instance_id,'repository','repository_path',r.full_name,r.id FROM repositories r JOIN account_instances a ON a.account_id=r.account_id;
INSERT OR IGNORE INTO resource_aliases(account_id,instance_id,kind,alias_kind,value,entity_id)
SELECT r.account_id,a.instance_id,'repository','web_url',json_extract(r.json,'$.web_url'),r.id FROM repositories r JOIN account_instances a ON a.account_id=r.account_id;
INSERT OR IGNORE INTO resource_aliases(account_id,instance_id,kind,alias_kind,value,entity_id)
SELECT i.account_id,a.instance_id,i.kind,'web_url',json_extract(i.json,'$.web_url'),i.id FROM items i JOIN account_instances a ON a.account_id=i.account_id WHERE json_extract(i.json,'$.web_url') IS NOT NULL;
INSERT INTO resource_aliases(account_id,instance_id,kind,alias_kind,value,repository_path,entity_id)
SELECT i.account_id,a.instance_id,i.kind,'repository_number',json_extract(i.json,'$.number'),r.full_name,i.id
FROM items i JOIN account_instances a ON a.account_id=i.account_id JOIN repositories r ON r.account_id=i.account_id AND r.id=i.repository_id WHERE json_extract(i.json,'$.number') IS NOT NULL;
