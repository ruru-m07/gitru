-- Empty on plaintext stores. Only a new native keyed reservation may bind it.
CREATE TABLE database_storage_identity (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    format_version INTEGER NOT NULL CHECK(format_version=1),
    database_id TEXT NOT NULL CHECK(length(database_id)=36),
    key_generation INTEGER NOT NULL CHECK(key_generation>=1),
    cipher_profile TEXT NOT NULL CHECK(cipher_profile='sqlcipher-v4-p4096-k256000-hmacsha512')
) STRICT;
CREATE TRIGGER database_storage_identity_immutable_update BEFORE UPDATE ON database_storage_identity
BEGIN SELECT RAISE(ABORT,'database storage identity is immutable'); END;
CREATE TRIGGER database_storage_identity_immutable_delete BEFORE DELETE ON database_storage_identity
BEGIN SELECT RAISE(ABORT,'database storage identity is immutable'); END;
