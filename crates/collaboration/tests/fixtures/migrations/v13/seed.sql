-- Frozen admitted intent and migration ledger before Files schema 0014.
INSERT INTO _sqlx_migrations VALUES (13,'command admission','2026-10-08 00:00:00',1,x'd29215527787e5b66230af7d8f1f1a915c969a52f54b49bace77247936b2138a361b82f3b167142341d914562d3b1109',130);
INSERT INTO commands VALUES ('a','11111111-1111-4111-8111-111111111111',17,1,'fixture.comment',1,'pull_request','pull',NULL,x'010203',x'0405',x'0607',zeroblob(32),1,9004,'2026-10-08T00:00:00Z','queued');
INSERT INTO command_target_protections(account_id,command_id,reference_kind,reference_id,facet) VALUES ('a','11111111-1111-4111-8111-111111111111','facet','pull','files');
INSERT INTO command_evidence(account_id,command_id,ordinal,kind,version,payload,recorded_at) VALUES ('a','11111111-1111-4111-8111-111111111111',0,'validation',1,x'0102','2026-10-08T00:00:00Z');
