# Oracle Fixtures

This directory is retained shared fixture data, not the legacy TypeScript test suite.

The JSON files here are generated from the Java/oracle tooling under `oracle/` and are consumed by Rust tests through `include_str!` paths. Keep this directory when deleting root-level TypeScript engine tests.

Do not move these fixtures without updating every native consumer in `native/crates/mclone-worldgen`, `native/crates/mclone-server`, and `native/crates/mclone-mesh` in the same change.

The `far-lod/` scripts are hand-authored native GPU probe inputs rather than
oracle output. Their schema and validation live in
`native/apps/mclone-native-client/src/lod_settle_probe.rs`.
