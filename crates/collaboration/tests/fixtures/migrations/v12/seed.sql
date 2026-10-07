-- Frozen additions to the v8 literal dataset before command admission.
INSERT INTO _sqlx_migrations VALUES
(9, 'task facets', '2026-10-07 00:00:00', 1, x'd0d0ac8e029c92dbc3ad804518219c5849edb4c8dd2818578202dee4596dcf56d173aef92e3d0c49d5e717685d606f8a', 90),
(10, 'cache retention', '2026-10-07 00:00:00', 1, x'441544b7f7125838a5532096dd40487323537ea7e4f9917c1174498a0b24c5695d07783a2ca7a149500cce9e00a80621', 100),
(11, 'pull commit generations', '2026-10-07 00:00:00', 1, x'28c0e19477d1d940af1508ace6bf353529f0393bc8d0c27e8529e8fde3764ae9bcafc932e93bd466c04e340095c089c4', 110),
(12, 'local inbox state', '2026-10-07 00:00:00', 1, x'65b06409d491935ea19d209a0317f96894e6c3e2cbb69ee03e130bb983ddc28e7aa720521f9963b5957db1d97fe5dd1b', 120);
INSERT INTO local_inbox_projection VALUES ('a',42);
INSERT INTO local_inbox_state VALUES ('a','retained-missing-notification','done',1,NULL,'2026-10-07T00:00:00Z',7);
