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
pnpm structure-lab:previews:check
pnpm structure-lab:previews:write
pnpm structure-lab:web:build
pnpm structure-lab:web:test
```

`structures:check` regenerates every promoted structure in memory and compares
the exact canonical bytes. It rejects stale, missing, manually edited,
duplicate, and orphaned JSON. `structures:write` is the only supported way to
refresh checked JSON.

The source loader always serializes and reparses a TypeScript-authored value
before returning it. Rust, catalogue assembly, previews, and tests consume
that reparsed record rather than the live DSL object.

Local TypeScript imports participate in the source provenance hash. Shared
family helpers are therefore safe to use: changing a helper invalidates every
generated member that imports it.

Preview generation consumes the two staged first-party packs and compiles each
canonical record through the Rust engine mesher. Run
`pnpm assets:pack:first-party` before `previews:write` after changing Texture
Lab recipes. The checked GLBs, shared atlas, and receipts under
`assets/mclone/structure-previews/` are the only geometry the read-only browser
catalogue consumes.

The current catalogue contains promoted cottage and barn families plus the
lab-only Rosehip Chicken Coop. The coop is the first structure authored wholly
through this workflow without a Rust constructor.
