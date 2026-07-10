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

    def test_generated_fallback_pack_is_deterministic_and_reference_free(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo_assets = root / "repo/assets"
            self.write_fixture_figure(repo_assets)
            reference_sentinel = root / "repo/reference/minecraft-1.17.1/sentinel.bin"
            reference_sentinel.parent.mkdir(parents=True)
            reference_sentinel.write_bytes(b"MINECRAFT_REFERENCE_SENTINEL")
            inventory = root / "inventory.json"
            inventory.write_text(json.dumps(self.fixture_inventory()), encoding="utf-8")

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
            self.assertEqual(manifest["origin"], "generated")
            self.assertNotIn(reference_sentinel.read_bytes(), outputs[0][0].read_bytes())

            with zipfile.ZipFile(outputs[0][0]) as archive:
                names = set(archive.namelist())
                self.assertIn("assets/mclone/textures/block/stone.png", names)
                self.assertIn("assets/minecraft/textures/entity/cow/cow.png", names)
                self.assertIn("assets/minecraft/textures/misc/underwater.png", names)
                self.assertIn("assets/mclone/figures/player.figure.json", names)
                self.assertIn("assets/mclone/visuals/blocks.v1.json", names)
                self.assertIn("assets/mclone/missing/registry.v1.json", names)
                self.assertTrue(
                    archive.read("assets/mclone/textures/block/stone.png").startswith(
                        b"\x89PNG\r\n\x1a\n"
                    )
                )
                self.assertFalse(any("reference/" in name for name in names))

    def test_authored_pack_is_deterministic_and_uses_only_first_party_roots(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runtime_root = root / "runtime-pack"
            runtime_texture = runtime_root / "assets/minecraft/textures/block/stone.png"
            runtime_texture.parent.mkdir(parents=True)
            runtime_texture.write_bytes(b"original-authored-png")
            lod = runtime_root / "assets/mclone/lod/materials.v1.json"
            lod.parent.mkdir(parents=True)
            lod.write_text('{"schema_version":1}\n', encoding="utf-8")
            authored_root = root / "authored-pack"
            authored_texture = authored_root / "assets/mclone/textures/block/stone.png"
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
                    runtime_root, authored_root, repo_assets, output, sidecar
                )
                packs.append((output, sidecar, manifest))

            self.assertEqual(packs[0][0].read_bytes(), packs[1][0].read_bytes())
            self.assertEqual(packs[0][1].read_bytes(), packs[1][1].read_bytes())
            self.assertEqual(packs[0][2]["origin"], "first_party")
            self.assertNotIn(sentinel.read_bytes(), packs[0][0].read_bytes())
            with zipfile.ZipFile(packs[0][0]) as archive:
                self.assertEqual(
                    archive.read("assets/minecraft/textures/block/stone.png"),
                    b"original-authored-png",
                )
                self.assertEqual(
                    archive.read("assets/mclone/textures/block/stone.png"),
                    b"canonical-original-authored-png",
                )
                self.assertFalse(any("reference/" in name for name in archive.namelist()))

    def test_stage_consumes_the_two_canonical_pack_outputs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo_assets = root / "repo/assets"
            self.write_fixture_figure(repo_assets)
            runtime_root = root / "runtime-pack"
            texture = runtime_root / "assets/minecraft/textures/block/stone.png"
            texture.parent.mkdir(parents=True)
            texture.write_bytes(b"authored")
            authored_root = root / "authored-pack"
            canonical_texture = authored_root / "assets/mclone/textures/block/stone.png"
            canonical_texture.parent.mkdir(parents=True)
            canonical_texture.write_bytes(b"canonical-authored")
            inventory = root / "inventory.json"
            inventory.write_text(json.dumps(self.fixture_inventory()), encoding="utf-8")
            authored = root / "mclone-authored.pbp"
            fallback = root / "mclone-generated-fallback.pbp"
            first_party_pack.build_authored_pack(
                runtime_root,
                authored_root,
                repo_assets,
                authored,
                Path(f"{authored}.json"),
            )
            first_party_pack.build_generated_fallback_pack(
                inventory,
                repo_assets,
                root / "generated-fallback-root",
                first_party_pack.FONT_PATH,
                fallback,
                Path(f"{fallback}.json"),
            )

            stage_root = root / "stage/first-party-packs"
            catalog = first_party_pack.stage_first_party_packs(
                authored, fallback, stage_root
            )

            self.assertEqual(
                [pack["pack_id"] for pack in catalog["packs"]],
                [first_party_pack.AUTHORED_PACK_ID, first_party_pack.FALLBACK_PACK_ID],
            )
            self.assertEqual(
                (stage_root / authored.name).read_bytes(), authored.read_bytes()
            )
            self.assertEqual(
                (stage_root / fallback.name).read_bytes(), fallback.read_bytes()
            )
            self.assertTrue((stage_root / "asset-pack-catalog.v1.json").is_file())

    @staticmethod
    def write_fixture_figure(repo_assets: Path) -> None:
        figure = repo_assets / "mclone/figures/player.figure.json"
        figure.parent.mkdir(parents=True)
        figure.write_text('{"name":"fixture"}\n', encoding="utf-8")

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
