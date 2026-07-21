# Mclone Structure Lab

Source-first authoring and read-only review tools for reusable Mclone
structures.

Canonical structures are authored only in
`examples/<name>/structure.ts`. Checked `*.structure.json` files are generated
semantic snapshots and must never be edited directly.

From the repository root:

```sh
pnpm structure-lab:typecheck
pnpm structure-lab:test
pnpm structure-lab:structures:check
pnpm structure-lab:structures:write
```

`structures:check` regenerates every promoted structure in memory and compares
the exact canonical bytes. It rejects stale, missing, manually edited,
duplicate, and orphaned JSON. `structures:write` is the only supported way to
refresh checked JSON.

The source loader always serializes and reparses a TypeScript-authored value
before returning it. Rust, catalogue assembly, previews, and tests consume
that reparsed record rather than the live DSL object.
