from __future__ import annotations

import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path


TOOL_DIR = Path(__file__).resolve().parent
if str(TOOL_DIR) not in sys.path:
    sys.path.insert(0, str(TOOL_DIR))

import asset_pack  # noqa: E402
import first_party_pack  # noqa: E402


class FirstPartyPackTests(unittest.TestCase):
    def test_short_codes_extend_only_colliding_prefixes(self) -> None:
        digests = {
            b"mclone:block/a": bytes.fromhex("ABCD0" + "0" * 59),
            b"mclone:block/b": bytes.fromhex("ABCD1" + "0" * 59),
            b"mclone:block/c": bytes.fromhex("CDEF0" + "0" * 59),
        }

        codes = first_party_pack.assign_short_codes(
            ["mclone:block/c", "mclone:block/b", "mclone:block/a"],
            lambda value: digests[value],
        )

        self.assertEqual(codes["mclone:block/a"], "ABCD0")
        self.assertEqual(codes["mclone:block/b"], "ABCD1")
        self.assertEqual(codes["mclone:block/c"], "CDEF")

    def test_full_short_code_collision_fails(self) -> None:
        with self.assertRaisesRegex(SystemExit, "full hash collision"):
            first_party_pack.assign_short_codes(
                ["mclone:block/a", "mclone:block/b"],
                lambda _value: b"\x11" * 32,
            )

    def test_provisional_pack_is_deterministic_and_reference_free(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo_assets = root / "repo/assets"
            self.write_fixture_figure(repo_assets)
            reference_sentinel = root / "repo/reference/minecraft-1.17.1/sentinel.bin"
            reference_sentinel.parent.mkdir(parents=True)
            reference_sentinel.write_bytes(b"MINECRAFT_REFERENCE_SENTINEL")
            inventory = root / "inventory.json"
            inventory.write_text(json.dumps(self.fixture_inventory()), encoding="utf-8")
            provisional_root = root / "lifecycle-provisional-root"
            provisional_texture = (
                provisional_root / "assets/mclone/textures/block/stone.png"
            )
            provisional_texture.parent.mkdir(parents=True)
            provisional_texture.write_bytes(b"lifecycle-provisional-png")

            outputs = []
            for run in ("one", "two"):
                run_root = root / run
                staging = run_root / "generated-fallback-root"
                output = run_root / "mclone-generated-fallback.pbp"
                sidecar = Path(f"{output}.json")
                manifest = first_party_pack.build_generated_fallback_pack(
                    inventory,
                    repo_assets,
                    staging,
                    provisional_root,
                    first_party_pack.FONT_PATH,
                    output,
                    sidecar,
                )
                outputs.append((output, sidecar, manifest))

            self.assertEqual(outputs[0][0].read_bytes(), outputs[1][0].read_bytes())
            self.assertEqual(outputs[0][1].read_bytes(), outputs[1][1].read_bytes())
            manifest = outputs[0][2]
            payload_sha = manifest["fingerprints"][asset_pack.PAYLOAD_FINGERPRINT_ID]["sha256"]
            self.assertEqual(manifest["content_fingerprint"], f"sha256:{payload_sha}")
            self.assertEqual(manifest["origin"], "first_party_provisional")
            self.assertNotIn(reference_sentinel.read_bytes(), outputs[0][0].read_bytes())

            with zipfile.ZipFile(outputs[0][0]) as archive:
                names = set(archive.namelist())
                self.assertIn("assets/mclone/textures/block/stone.png", names)
                self.assertIn("assets/minecraft/textures/entity/cow/cow.png", names)
                self.assertIn("assets/minecraft/textures/misc/underwater.png", names)
                self.assertIn("assets/mclone/figures/player.figure.json", names)
                self.assertIn("assets/mclone/visuals/blocks.v1.json", names)
                self.assertIn("assets/mclone/provisional/registry.v1.json", names)
                self.assertNotIn("assets/mclone/missing/registry.v1.json", names)
                self.assertTrue(
                    archive.read("assets/mclone/textures/block/stone.png")
                    == b"lifecycle-provisional-png"
                )
                self.assertEqual(manifest["coverage"]["lifecycle_override_count"], 1)
                self.assertFalse(any("reference/" in name for name in names))

    def test_diagnostic_pack_owns_numbered_missing_registry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo_assets = root / "repo/assets"
            self.write_fixture_figure(repo_assets)
            inventory = root / "inventory.json"
            inventory.write_text(json.dumps(self.fixture_inventory()), encoding="utf-8")
            output = root / "mclone-diagnostic-missing.pbp"
            manifest = first_party_pack.build_diagnostic_missing_pack(
                inventory,
                repo_assets,
                root / "diagnostic-missing-root",
                first_party_pack.FONT_PATH,
                output,
                Path(f"{output}.json"),
            )

            self.assertEqual(manifest["origin"], "diagnostic")
            with zipfile.ZipFile(output) as archive:
                names = set(archive.namelist())
                self.assertIn("assets/mclone/missing/registry.v1.json", names)
                self.assertNotIn("assets/mclone/provisional/registry.v1.json", names)
                registry = json.loads(
                    archive.read("assets/mclone/missing/registry.v1.json")
                )
                self.assertEqual(len(registry["entries"]), 4)

    def test_authored_pack_is_deterministic_and_uses_only_first_party_roots(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            curated_root = root / "lifecycle-curated-root"
            authored_texture = curated_root / "assets/mclone/textures/block/stone.png"
            authored_texture.parent.mkdir(parents=True)
            authored_texture.write_bytes(b"canonical-original-authored-png")
            repo_assets = root / "repo/assets"
            self.write_fixture_figure(repo_assets)
            sentinel = root / "repo/reference/minecraft-1.17.1/sentinel.bin"
            sentinel.parent.mkdir(parents=True)
            sentinel.write_bytes(b"MINECRAFT_REFERENCE_SENTINEL")

            packs = []
            for run in ("one", "two"):
                output = root / run / "mclone-authored.pbp"
                sidecar = Path(f"{output}.json")
                manifest = first_party_pack.build_authored_pack(
                    curated_root, repo_assets, output, sidecar
                )
                packs.append((output, sidecar, manifest))

            self.assertEqual(packs[0][0].read_bytes(), packs[1][0].read_bytes())
            self.assertEqual(packs[0][1].read_bytes(), packs[1][1].read_bytes())
            self.assertEqual(packs[0][2]["origin"], "first_party")
            self.assertIn("audio_content", packs[0][2]["roles"])
            self.assertNotIn(sentinel.read_bytes(), packs[0][0].read_bytes())
            with zipfile.ZipFile(packs[0][0]) as archive:
                self.assertEqual(
                    archive.read("assets/mclone/textures/block/stone.png"),
                    b"canonical-original-authored-png",
                )
                self.assertNotIn(
                    "assets/minecraft/textures/block/stone.png",
                    archive.namelist(),
                )
                self.assertFalse(any("reference/" in name for name in archive.namelist()))

    def test_stage_consumes_the_three_canonical_pack_outputs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo_assets = root / "repo/assets"
            self.write_fixture_figure(repo_assets)
            curated_root = root / "lifecycle-curated-root"
            canonical_texture = curated_root / "assets/mclone/textures/block/stone.png"
            canonical_texture.parent.mkdir(parents=True)
            canonical_texture.write_bytes(b"canonical-authored")
            inventory = root / "inventory.json"
            inventory.write_text(json.dumps(self.fixture_inventory()), encoding="utf-8")
            authored = root / "mclone-authored.pbp"
            fallback = root / "mclone-generated-fallback.pbp"
            diagnostic = root / "mclone-diagnostic-missing.pbp"
            first_party_pack.build_authored_pack(
                curated_root,
                repo_assets,
                authored,
                Path(f"{authored}.json"),
            )
            first_party_pack.build_generated_fallback_pack(
                inventory,
                repo_assets,
                root / "generated-fallback-root",
                self.empty_directory(root / "lifecycle-provisional-root"),
                first_party_pack.FONT_PATH,
                fallback,
                Path(f"{fallback}.json"),
            )
            first_party_pack.build_diagnostic_missing_pack(
                inventory,
                repo_assets,
                root / "diagnostic-missing-root",
                first_party_pack.FONT_PATH,
                diagnostic,
                Path(f"{diagnostic}.json"),
            )

            stage_root = root / "stage/first-party-packs"
            catalog = first_party_pack.stage_first_party_packs(
                authored, fallback, diagnostic, stage_root
            )

            self.assertEqual(
                [pack["pack_id"] for pack in catalog["packs"]],
                [
                    first_party_pack.AUTHORED_PACK_ID,
                    first_party_pack.FALLBACK_PACK_ID,
                    first_party_pack.DIAGNOSTIC_PACK_ID,
                ],
            )
            self.assertEqual(
                (stage_root / authored.name).read_bytes(), authored.read_bytes()
            )
            self.assertEqual(
                (stage_root / fallback.name).read_bytes(), fallback.read_bytes()
            )
            self.assertEqual(
                (stage_root / diagnostic.name).read_bytes(), diagnostic.read_bytes()
            )
            self.assertTrue((stage_root / "asset-pack-catalog.v1.json").is_file())

    @staticmethod
    def write_fixture_figure(repo_assets: Path) -> None:
        figure = repo_assets / "mclone/figures/player.figure.json"
        figure.parent.mkdir(parents=True)
        figure.write_text('{"name":"fixture"}\n', encoding="utf-8")

    @staticmethod
    def empty_directory(directory: Path) -> Path:
        directory.mkdir(parents=True, exist_ok=True)
        return directory

    @staticmethod
    def fixture_inventory() -> dict[str, object]:
        return {
            "schema_version": 1,
            "asset_schema": first_party_pack.ASSET_SCHEMA,
            "block_visuals": [
                {
                    "state_id": 1,
                    "state": "minecraft:stone",
                    "class": "solid",
                    "material": "mclone:block/stone",
                },
                {
                    "state_id": 2,
                    "state": "minecraft:water[level=0]",
                    "class": "fluid",
                    "material": "mclone:block/water",
                },
            ],
            "materials": ["mclone:block/stone", "mclone:block/water"],
            "direct_assets": [
                {
                    "path": "assets/mclone/figures/player.figure.json",
                    "consumer": "actor_figure",
                    "policy": "required",
                },
                {
                    "path": "assets/minecraft/textures/entity/cow/cow.png",
                    "consumer": "actor_texture",
                    "policy": "required",
                },
                {
                    "path": "assets/minecraft/textures/misc/underwater.png",
                    "consumer": "screen_effect",
                    "policy": "required",
                },
                {
                    "path": "assets/minecraft/sounds/damage/fallsmall.ogg",
                    "consumer": "audio",
                    "policy": "suppressible",
                },
            ],
        }


if __name__ == "__main__":
    unittest.main()
