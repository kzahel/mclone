import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { block, role, structure } from "../src/dsl";
import { discoverCanonicalStructureSources } from "../src/discover-structures";
import { FIRST_PARTY_STRUCTURES } from "../src/first-party-structures";
import { loadStructureJsonDocument } from "../src/load";
import { parseStructureAssetJson, serializeStructureAsset } from "../src/structure-json";

test("discovers and round-trips every canonical TypeScript structure", async () => {
  const sources = await discoverCanonicalStructureSources();
  assert.ok(sources.length >= 1);
  assert.ok(sources.every((source) => source.endsWith(path.join("structure.ts"))));
  for (const source of sources) {
    const first = await loadStructureJsonDocument(source);
    const second = await loadStructureJsonDocument(source);
    assert.equal(first.json, second.json);
    assert.deepEqual(parseStructureAssetJson(first.json, source), first.asset);
    assert.equal(serializeStructureAsset(first.asset), first.json);
  }
});

test("the standard cottage remains a promoted family member", async () => {
  assert.equal(FIRST_PARTY_STRUCTURES.length, 12);
  const standard = FIRST_PARTY_STRUCTURES.find(
    (entry) => entry.runtimeStructureId === "farmstead-cottage-a-v2",
  );
  assert.ok(standard);
  const document = await loadStructureJsonDocument(standard.sourcePath);
  assert.equal(document.asset.id, "farmstead-cottage-a-v2");
  assert.deepEqual(document.asset.size, [15, 15, 17]);
  assert.equal(document.asset.defaultTheme, "warm-oak-and-plaster-v2");
  assert.ok(document.asset.blocks.length > 1_000);
  assert.deepEqual(
    document.asset.markers.map((marker) => marker.kind),
    ["entrance:south", "attachment:west-yard"],
  );
  assert.match(document.asset.provenance.sourceSha256, /^[a-f0-9]{64}$/u);
  assert.match(document.asset.provenance.semanticSha256, /^[a-f0-9]{64}$/u);
});

test("exclusive boxes, palette references, and bounds are enforced", () => {
  assert.throws(
    () => structure({
      id: "invalid-box",
      label: "Invalid box",
      description: "A deliberately invalid structure fixture.",
      category: "decoration",
      size: [2, 2, 2],
      palette: { wall: role("wall") },
    }, ({ fillBox }) => fillBox([0, 0, 0], [0, 1, 1], "wall")),
    /must have positive extent/u,
  );
  assert.throws(
    () => structure({
      id: "invalid-palette",
      label: "Invalid palette",
      description: "A deliberately invalid palette fixture.",
      category: "decoration",
      size: [1, 1, 1],
      palette: { air: block("minecraft:air") },
    }, ({ set }) => set([0, 0, 0], "missing")),
    /unknown palette key 'missing'/u,
  );
  assert.throws(
    () => structure({
      id: "invalid-bounds",
      label: "Invalid bounds",
      description: "A deliberately invalid bounds fixture.",
      category: "decoration",
      size: [1, 1, 1],
      palette: { wall: role("wall") },
    }, ({ set }) => set([1, 0, 0], "wall")),
    /outside structure size/u,
  );
});

test("semantic hash catches generated JSON tampering", async () => {
  const document = await loadStructureJsonDocument(FIRST_PARTY_STRUCTURES[0]!.sourcePath);
  const tampered = JSON.parse(document.json) as Record<string, unknown>;
  tampered.label = "Hand-edited JSON";
  assert.throws(
    () => parseStructureAssetJson(`${JSON.stringify(tampered, null, 2)}\n`, "tampered fixture"),
    /semantic hash is stale/u,
  );
});
