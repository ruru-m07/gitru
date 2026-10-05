-- Locally authored intent is independent of rebuildable provider projections.
CREATE TABLE local_link_meta (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    bindings_generation INTEGER NOT NULL CHECK(bindings_generation>0)
) STRICT;
INSERT INTO local_link_meta(singleton,bindings_generation) VALUES(1,1);
CREATE TABLE local_transport_bindings (
    id TEXT PRIMARY KEY NOT NULL,
    instance_id TEXT NOT NULL REFERENCES provider_instances(id) ON DELETE RESTRICT,
    transport TEXT NOT NULL CHECK(transport IN ('https','ssh','scp')),
    host TEXT NOT NULL,
    port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
    path_prefix TEXT NOT NULL,
    layout TEXT NOT NULL CHECK(layout IN ('owner_repository','subgroups')),
    generation INTEGER NOT NULL CHECK(generation>0),
    UNIQUE(transport,host,port,path_prefix)
) STRICT;
CREATE TABLE local_repository_links (
    id TEXT PRIMARY KEY NOT NULL,
    local_repository_id TEXT NOT NULL,
    endpoint_json TEXT NOT NULL,
    account_id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    repository_id TEXT NOT NULL,
    repository_provider_id TEXT NOT NULL,
    registration_proof TEXT NOT NULL,
    remote_digest TEXT NOT NULL,
    generation INTEGER NOT NULL CHECK(generation>0),
    FOREIGN KEY(account_id,instance_id) REFERENCES account_instances(account_id,instance_id) ON DELETE RESTRICT,
    UNIQUE(local_repository_id,endpoint_json,account_id)
) STRICT;
CREATE INDEX local_link_repository ON local_repository_links(local_repository_id,id);
CREATE INDEX local_link_remote_identity ON local_repository_links(account_id,instance_id,repository_provider_id,id);
