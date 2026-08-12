# First-Party Audio

Mclone's distributable sound bank combines a curated 119-file subset of
Kenney's CC0 Impact Sounds, RPG Audio, and Interface Sounds packs with two
project-generated CC0 mallard call variants. The runtime catalog is
`sound-bank.v1.json`; `provenance.v1.json` records every source file, hash,
origin, semantic use, source URL where applicable, and retained license.

Ordinary builds are offline and consume the checked-in files. Verify them with:

```bash
pnpm assets:sfx:check
```

To reproduce the import, download the three archives from the URLs recorded in
`provenance.v1.json`, name them `impact.zip`, `rpg.zip`, and `interface.zip`,
then run:

```bash
pnpm assets:sfx:import -- --archive-dir /path/to/archive-directory
```

The importer rejects any archive, license, or selected-file drift before
rewriting the deterministic catalog and provenance ledger. Do not add effects
by dropping files into the directory; extend the allowlist and semantic
families in `tools/minecraft_assets/kenney_audio.py` and rerun the importer.

The mallard variants are reproducible synthesis, intentionally stylized rather
than claimed wildlife recordings. Regenerate and refresh their manifests with:

```bash
bash tools/minecraft_assets/generate_mallard_calls.sh
bash tools/minecraft_assets/generate_deer_sounds.sh
python3 tools/minecraft_assets/kenney_audio.py refresh-local
```
