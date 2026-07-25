import { expect, test } from "@playwright/test";
import fs from "node:fs/promises";
import path from "node:path";
import { promoteFrozenTextureFromFreezeRequest } from "../../src/core/frozen-curation";
import { loadTexturePack } from "../../src/load";
import { decodePng, type RgbaImage } from "../../src/png";

test("indexes authored textures, generated candidates, and allowlisted images", async ({ request }) => {
  const indexResponse = await request.get("/api/index");
  await expect(indexResponse).toBeOK();
  const index = await indexResponse.json();

  expect(index.pack.name).toBe("mclone-default");
  expect(index.summary.authoredTextures).toBe(108);
  expect(index.summary.candidateCount).toBe(4);
  expect(index.summary.associatedCandidateCount).toBe(4);
  expect(index.summary.archivedCandidateCount).toBe(1);
  expect(index.summary.curatedSelectionCount).toBe(0);
  expect(index.summary.frozenTextureCount).toBe(2);
  expect(index.summary.proceduralPlaceholderCount).toBeGreaterThan(40);
  expect(index.summary.candidateLifecycleCount).toBe(33);
  expect(index.summary.provisionalLifecycleCount).toBe(0);
  expect(index.summary.curatedLifecycleCount).toBe(2);
  expect(index.summary.legacyDerivedTextureCount).toBe(73);
  expect(index.curation.selectedCount).toBe(0);
  expect(index.curation.manifestPath).toContain("generated-assets/texture-lab-playwright/curation/selections.v1.json");
  expect(index.pack.blockCount).toBe(27);
  expect(index.blocks.length).toBeGreaterThan(index.pack.blockCount);
  expect(index.vanillaCoverage.summary.vanillaTextureCount).toBeGreaterThan(700);
  expect(index.vanillaCoverage.summary.missingTextureCount).toBeGreaterThan(500);
  expect(index.vanillaCoverage.summary.coveredTextureCount).toBeGreaterThan(50);

  expect(index.vanillaCoverage.entries.find((entry: { name: string }) => entry.name === "acacia_planks")).toMatchObject({
    texture: "minecraft:block/acacia_planks",
    status: "missing",
    authoredTextures: [],
  });
  expect(index.vanillaCoverage.entries.find((entry: { name: string }) => entry.name === "stone")).toMatchObject({
    texture: "minecraft:block/stone",
    status: "frozen",
    authoredTextures: [{ textureName: "stone", frozen: true }],
  });

  const grassCandidatesResponse = await request.get("/api/candidates?texture=grass_block_top");
  await expect(grassCandidatesResponse).toBeOK();
  const grassCandidates = await grassCandidatesResponse.json();
  expect(grassCandidates).toHaveLength(4);
  expect(grassCandidates.map((candidate: { codename: string; source: string }) => `${candidate.codename}:${candidate.source}`)).toEqual([
    "G5101S74:archive",
    "G5101S74:diffusion",
    "G5101S74:projection",
    "G5102S62:diffusion",
  ]);
  expect(grassCandidates.find((candidate: { source: string; promotable: boolean }) => candidate.source === "diffusion")?.promotable).toBe(false);
  const archivedCandidate = grassCandidates.find((candidate: { source: string; id: string; promotable: boolean }) => candidate.source === "archive");
  expect(archivedCandidate?.promotable).toBe(true);
  if (!archivedCandidate) {
    throw new Error("Missing archived grass candidate fixture");
  }

  const grass = index.textures.find((texture: { name: string }) => texture.name === "grass_block_top");
  expect(grass?.images.currentExport.exists).toBe(true);
  expect(grass?.frozen).toMatchObject({
    asset: "frozen/block/grass_block_top.png",
    codename: "G5101S74",
    candidateId: "candidate-seed5101-strength0p740",
    sha256: "2e361a434f83d7dbe5c8b810880897bb5308f4243394e03777e86d47b2d7c190",
  });
  expect(grass?.artSource).toMatchObject({
    kind: "frozen",
    label: "frozen asset",
  });
  expect(grass?.lifecycle).toMatchObject({
    state: "curated",
    runtimeMaterials: ["mclone:block/grass_block"],
    promotable: true,
  });
  expect(grass?.images.curated.exists).toBe(true);
  expect(grass?.images.provisional.exists).toBe(false);
  expect(grass?.images.minecraftReference.exists).toBe(true);
  expect(grass?.tint.normal).toBe("#79b34e");
  expect(grass?.tint.sourceNeutrality).toEqual({
    maxMeanSaturation: 0.005,
    maxPixelSaturation: 0.01,
  });
  const imageResponse = await request.get(`/api/image?path=${encodeURIComponent(grass.images.currentExport.path)}`);
  await expect(imageResponse).toBeOK();
  expect(imageResponse.headers()["content-type"]).toBe("image/png");
  expect(saturation(meanRgb(decodePng(await imageResponse.body())))).toBeLessThan(0.005);
  const tintedImageResponse = await request.get(
    `/api/tinted-image?path=${encodeURIComponent(grass.images.currentExport.path)}&tint=${encodeURIComponent(grass.tint.normal)}`,
  );
  await expect(tintedImageResponse).toBeOK();
  expect(tintedImageResponse.headers()["content-type"]).toBe("image/png");

  const pointedDripstone = index.textures.find((texture: { name: string }) => texture.name === "pointed_dripstone");
  expect(pointedDripstone?.artSource).toMatchObject({
    kind: "procedural-placeholder",
    label: "noise placeholder",
  });
  expect(pointedDripstone?.images.minecraftReference.exists).toBe(true);
  expect(pointedDripstone?.images.minecraftReference.path).toContain("reference-composites/pointed_dripstone-up-down-thickness.png");
  const pointedReferenceResponse = await request.get(
    `/api/image?path=${encodeURIComponent(pointedDripstone.images.minecraftReference.path)}`,
  );
  await expect(pointedReferenceResponse).toBeOK();
  const pointedReference = decodePng(await pointedReferenceResponse.body());
  expect(`${pointedReference.width}x${pointedReference.height}`).toBe("32x80");

  const carvedPumpkin = index.blocks.find((block: { name: string }) => block.name === "carved-pumpkin");
  expect(carvedPumpkin?.previewSource).toBe("authored");
  expect(carvedPumpkin?.faces.map((face: { face: string; textureName: string }) => `${face.face}:${face.textureName}`)).toEqual([
    "top:pumpkin_top",
    "bottom:pumpkin_top",
    "north:carved_pumpkin",
    "east:pumpkin_side",
    "south:pumpkin_side",
    "west:pumpkin_side",
  ]);
  expect(carvedPumpkin?.sheet.exists).toBe(true);
  const carvedPumpkinSheetResponse = await request.get(`/api/image?path=${encodeURIComponent(carvedPumpkin.sheet.path)}`);
  await expect(carvedPumpkinSheetResponse).toBeOK();
  const carvedPumpkinSheet = decodePng(await carvedPumpkinSheetResponse.body());
  expect(`${carvedPumpkinSheet.width}x${carvedPumpkinSheet.height}`).toBe("1040x672");

  const grassBlock = index.blocks.find((block: { name: string }) => block.name === "grass");
  if (!grassBlock) {
    throw new Error("Missing grass cross block fixture");
  }
  expect(grassBlock?.kind).toBe("cross");
  expect(grassBlock?.previewSource).toBe("authored");
  expect(grassBlock?.faces.map((face: { face: string; textureName: string }) => `${face.face}:${face.textureName}`)).toEqual([
    "all:grass_cross",
  ]);
  expect(grassBlock?.sheet.path).toContain("grass-block-sheet.png");
  const grassBlockSheetResponse = await request.get(`/api/image?path=${encodeURIComponent(grassBlock.sheet.path)}`);
  await expect(grassBlockSheetResponse).toBeOK();
  const grassBlockSheet = decodePng(await grassBlockSheetResponse.body());
  expect(`${grassBlockSheet.width}x${grassBlockSheet.height}`).toBe("1040x672");

  const redstoneDustDot = index.blocks.find((block: { name: string }) => block.name === "redstone-dust-dot");
  if (!redstoneDustDot) {
    throw new Error("Missing redstone dust dot flat block fixture");
  }
  expect(redstoneDustDot?.kind).toBe("flat");
  expect(redstoneDustDot?.previewSource).toBe("authored");
  expect(redstoneDustDot?.faces.map((face: { face: string; textureName: string }) => `${face.face}:${face.textureName}`)).toEqual([
    "top:redstone_dust_dot",
  ]);
  expect(redstoneDustDot?.sheet.path).toContain("redstone-dust-dot-sheet.png");

  expect(index.blocks.find((block: { name: string }) => block.name === "glass-pane")).toMatchObject({
    kind: "pane",
    previewSource: "vanilla-derived",
    faces: [
      { face: "top", textureName: "glass_pane_top" },
      { face: "side", textureName: "glass" },
    ],
  });
  expect(index.blocks.find((block: { name: string }) => block.name === "rail")).toMatchObject({
    kind: "rail",
    previewSource: "vanilla-derived",
    faces: [{ face: "top", textureName: "rail" }],
  });
  expect(index.blocks.find((block: { name: string }) => block.name === "torch")).toMatchObject({
    kind: "torch",
    previewSource: "vanilla-derived",
    faces: [{ face: "side", textureName: "torch" }],
  });
  expect(index.blocks.find((block: { name: string }) => block.name === "oak-door")).toMatchObject({
    kind: "door",
    previewSource: "vanilla-derived",
    faces: [
      { face: "top", textureName: "oak_door_top" },
      { face: "bottom", textureName: "oak_door_bottom" },
    ],
  });
  expect(index.blocks.find((block: { name: string }) => block.name === "oak-trapdoor")).toMatchObject({
    kind: "trapdoor",
    previewSource: "vanilla-derived",
    faces: [{ face: "top", textureName: "oak_trapdoor" }],
  });

  for (const textureName of ["grass_cross", "fern_cross"]) {
    const texture = index.textures.find((entry: { name: string }) => entry.name === textureName);
    if (!texture) {
      throw new Error(`Missing ${textureName} fixture`);
    }
    expect(texture.vanillaUsage).toMatchObject({
      previewHint: "cross",
      geometryKinds: ["cross-sprite"],
      renderLayers: ["cutout"],
      tintRoles: ["grass"],
    });
    expect(texture.vanillaUsage.textureSlots).toContain(textureName === "grass_cross" ? "cross" : "plant");
    expect(texture.vanillaUsage.uses.map((use: { block: string }) => use.block)).toContain(
      textureName === "grass_cross" ? "minecraft:grass" : "minecraft:fern",
    );
    const exportedResponse = await request.get(`/api/image?path=${encodeURIComponent(texture.images.currentExport.path)}`);
    await expect(exportedResponse).toBeOK();
    const referenceResponse = await request.get(`/api/image?path=${encodeURIComponent(texture.images.minecraftReference.path)}`);
    await expect(referenceResponse).toBeOK();
    expect(Math.abs(normalizedAlphaCentroidX(decodePng(await exportedResponse.body())) - normalizedAlphaCentroidX(decodePng(await referenceResponse.body())))).toBeLessThan(
      0.05,
    );
  }

  const redstoneDustDotTexture = index.textures.find((entry: { name: string }) => entry.name === "redstone_dust_dot");
  expect(redstoneDustDotTexture?.vanillaUsage).toMatchObject({
    previewHint: "flat",
    geometryKinds: ["flat-ground"],
    renderLayers: ["cutout"],
    textureSlots: ["line"],
    tintRoles: ["tintindex:0"],
  });
  expect(redstoneDustDotTexture?.vanillaUsage.uses.map((use: { block: string }) => use.block)).toContain("minecraft:redstone_wire");

  const shapeUsageExpectations = [
    ["glass_pane_top", "partial", "pane", "minecraft:glass_pane"],
    ["rail", "flat", "flat-ground", "minecraft:rail"],
    ["torch", "partial", "torch", "minecraft:torch"],
    ["oak_door_top", "partial", "door", "minecraft:oak_door"],
    ["oak_trapdoor", "partial", "trapdoor", "minecraft:oak_trapdoor"],
  ] as const;
  for (const [textureName, previewHint, geometryKind, vanillaBlock] of shapeUsageExpectations) {
    const texture = index.textures.find((entry: { name: string }) => entry.name === textureName);
    expect(texture?.images.currentExport.exists).toBe(true);
    expect(texture?.images.sheet.exists).toBe(true);
    expect(texture?.vanillaUsage?.previewHint).toBe(previewHint);
    expect(texture?.vanillaUsage?.geometryKinds).toContain(geometryKind);
    expect(texture?.vanillaUsage?.uses.map((use: { block: string }) => use.block)).toContain(vanillaBlock);
  }

  const selectResponse = await request.post("/api/curation/select", {
    data: { textureName: "grass_block_top", candidateId: archivedCandidate.id },
  });
  await expect(selectResponse).toBeOK();
  const selectedIndex = await selectResponse.json();
  expect(selectedIndex.summary.curatedSelectionCount).toBe(1);
  expect(selectedIndex.curation.selections[0]).toMatchObject({
    textureName: "grass_block_top",
    candidateId: archivedCandidate.id,
    codename: "G5101S74",
    source: "archive",
  });

  const freezeResponse = await request.post("/api/freeze-request", {
    data: { textureName: "grass_block_top" },
  });
  await expect(freezeResponse).toBeOK();
  const freezePayload = await freezeResponse.json();
  expect(freezePayload.request.path).toContain("generated-assets/texture-lab-playwright/freeze-requests/");
  expect(freezePayload.request.texture).toMatchObject({
    name: "grass_block_top",
    exportPath: "assets/mclone/textures/block/grass_block_top.png",
    source: "tintable",
    tintRole: "grass",
  });
  expect(freezePayload.request.candidate).toMatchObject({
    id: archivedCandidate.id,
    codename: "G5101S74",
    freezeReadiness: "archived",
    seed: 5101,
    strength: 0.74,
    promptPreset: "grass-top-tufts",
  });
  expect(freezePayload.request.images.projected.sha256).toMatch(/^[a-f0-9]{64}$/);
  expect(freezePayload.request.images.raw.sha256).toMatch(/^[a-f0-9]{64}$/);
  expect(freezePayload.request.sourcePolicy.errors).toEqual([]);
  expect(freezePayload.request.sourcePolicy.stats.meanSaturation).toBeLessThan(0.005);
  expect(freezePayload.request.sourcePatch).toMatchObject({
    mode: "agent-or-cli-mediated",
    sourceFileHint: "tools/texture-lab/packs/mclone-default/block/grass-block.ts",
    textureName: "grass_block_top",
  });
  expect(freezePayload.request.sourcePatch.intendedSourceShape).toContain("canonical curation.v1.json");
  const freezeListResponse = await request.get("/api/freeze-requests?texture=grass_block_top");
  await expect(freezeListResponse).toBeOK();
  const freezeList = await freezeListResponse.json();
  expect(freezeList.requests.map((entry: { path: string }) => entry.path)).toContain(freezePayload.request.path);

  const pack = await loadTexturePack(index.pack.inputPath);
  const tempPackInputPath = path.join(index.outputRoot, "promote-freeze-request-pack", "texture.ts");
  await fs.mkdir(path.dirname(tempPackInputPath), { recursive: true });
  const promoted = await promoteFrozenTextureFromFreezeRequest({
    pack,
    packInputPath: tempPackInputPath,
    requestPath: freezePayload.request.path,
  });
  expect(promoted.textureName).toBe("grass_block_top");
  expect(promoted.entry).toMatchObject({
    asset: "frozen/block/grass_block_top.png",
    codename: "G5101S74",
    candidateId: "candidate-seed5101-strength0p740",
    seed: 5101,
    strength: 0.74,
  });
  expect(promoted.entry.sourceContext?.freezeRequest).toContain("generated-assets/texture-lab-playwright/freeze-requests/");
  const promotedImage = decodePng(await fs.readFile(promoted.assetPath));
  expect(`${promotedImage.width}x${promotedImage.height}`).toBe("64x64");
  const promotedManifest = JSON.parse(await fs.readFile(promoted.manifestPath, "utf8"));
  expect(promotedManifest.textures.grass_block_top.sha256).toBe(promoted.entry.sha256);

  const applyResponse = await request.post("/api/curation/apply");
  await expect(applyResponse).toBeOK();
  const applyPayload = await applyResponse.json();
  expect(applyPayload.result.applied).toHaveLength(1);
  expect(applyPayload.result.applied[0]).toMatchObject({
    textureName: "grass_block_top",
    codename: "G5101S74",
    runtimeCompatPath: "assets/minecraft/textures/block/grass_block_top.png",
  });
  const appliedGrass = applyPayload.index.textures.find((texture: { name: string }) => texture.name === "grass_block_top");
  const appliedImageResponse = await request.get(`/api/image?path=${encodeURIComponent(appliedGrass.images.currentExport.path)}`);
  await expect(appliedImageResponse).toBeOK();
  expect(saturation(meanRgb(decodePng(await appliedImageResponse.body())))).toBeLessThan(0.005);

  const clearResponse = await request.post("/api/curation/clear", {
    data: { textureName: "grass_block_top" },
  });
  await expect(clearResponse).toBeOK();
  expect((await clearResponse.json()).summary.curatedSelectionCount).toBe(0);
});

test("defaults to the system theme and toggles light or dark mode", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");

  const shell = page.locator(".appShell");
  await expect(shell).toHaveAttribute("data-theme", "dark");
  await expect(page.getByRole("button", { name: "Switch to light mode" })).toHaveText("Light");
  await expect(page.getByRole("heading", { name: "Carved Pumpkin" })).toBeVisible();
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-dark-mode.png", fullPage: true });

  await page.getByRole("button", { name: "Switch to light mode" }).click();
  await expect(shell).toHaveAttribute("data-theme", "light");
  await expect(page.getByRole("button", { name: "Switch to dark mode" })).toHaveText("Dark");

  await page.emulateMedia({ colorScheme: "light" });
  await expect(shell).toHaveAttribute("data-theme", "light");
});

test("keeps the texture list in its own scroll pane", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Carved Pumpkin" })).toBeVisible();

  const before = await layoutMetrics(page);
  expect(before.documentScrollHeight).toBeLessThanOrEqual(before.viewportHeight + 1);
  expect(before.textureListScrollable).toBe(true);
  expect(before.previewTop).toBeGreaterThanOrEqual(0);
  expect(before.previewBottom).toBeLessThanOrEqual(before.viewportHeight);

  await page.locator(".textureList").evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });

  const after = await layoutMetrics(page);
  expect(after.textureListScrollTop).toBeGreaterThan(200);
  expect(after.previewTop).toBe(before.previewTop);
  expect(after.previewBottom).toBe(before.previewBottom);

  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-scroll-pane.png", fullPage: true });
});

test("defaults to automatic rendered previews from vanilla metadata", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("button", { name: "Auto" })).toHaveAttribute("aria-pressed", "true");

  await page.getByLabel("Search").fill("grass_cross");
  await page.getByRole("button", { name: /grass_cross/ }).click();
  await expect(page.getByRole("heading", { name: "Grass Cross" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Rendered Uses" })).toBeVisible();
  await expect(page.locator(".overviewHeader")).toContainText("cross / 1 block / 1 face");
  await expect(page.getByRole("button", { name: "grass all uses Grass Cross", exact: true })).toBeVisible();
  await expect(page.locator(".chipGroup")).toContainText("cross");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-auto-cross.png", fullPage: true });

  await page.getByLabel("Search").fill("redstone-dust-dot");
  await page.getByRole("button", { name: /redstone_dust_dot/ }).click();
  await expect(page.getByRole("heading", { name: "Redstone Dust Dot" })).toBeVisible();
  await expect(page.locator(".overviewHeader")).toContainText("flat / 1 block / 1 face");
  await expect(page.getByRole("button", { name: "redstone-dust-dot top uses Redstone Dust Dot", exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Detail" }).click();
  await expect(page.getByRole("button", { name: "Detail" })).toHaveAttribute("aria-pressed", "true");
  await page.getByLabel("Search").fill("fern_cross");
  await page.getByRole("button", { name: /fern_cross/ }).click();
  await expect(page.getByRole("button", { name: "Detail" })).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("heading", { name: "Fern Cross" })).toBeVisible();
});

test("shows shape-specific automatic previews for partial block families", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("button", { name: "Auto" })).toHaveAttribute("aria-pressed", "true");

  await page.getByLabel("Search").fill("glass_pane_top");
  await page.getByRole("button", { name: /glass_pane_top/ }).click();
  await expect(page.getByRole("heading", { name: "Glass Pane Top" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Rendered Uses" })).toBeVisible();
  await expect(page.locator(".overviewHeader")).toContainText("partial / 1 block / 1 face");
  const glassPaneBundle = page.locator(".blockBundleCard").filter({ hasText: "Glass Pane" });
  await expect(glassPaneBundle).toContainText("pane / 1 faces");
  await expect(glassPaneBundle).toContainText("vanilla-derived preview");
  await expect(page.getByRole("heading", { name: "Vanilla-Derived Review Blocks" })).toBeVisible();
  await expect(page.getByRole("button", { name: "glass-pane top uses Glass Pane Top", exact: true })).toBeVisible();
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-shape-pane.png", fullPage: true });

  await page.getByLabel("Search").fill("oak_door_top");
  await page.getByRole("button", { name: /oak_door_top/ }).click();
  await expect(page.getByRole("heading", { name: "Oak Door Top" })).toBeVisible();
  const doorBundle = page.locator(".blockBundleCard").filter({ hasText: "Oak Door" });
  await expect(doorBundle).toContainText("door / 1 faces");
  await expect(doorBundle).toContainText("vanilla-derived preview");
  await expect(page.getByRole("button", { name: "oak-door top uses Oak Door Top", exact: true })).toBeVisible();
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-shape-door.png", fullPage: true });

  for (const [textureName, blockName, faceName, displayName, kind] of [
    ["rail", "rail", "top", "Rail", "rail"],
    ["torch", "torch", "side", "Torch", "torch"],
    ["oak_trapdoor", "oak-trapdoor", "top", "Oak Trapdoor", "trapdoor"],
  ] as const) {
    await page.getByLabel("Search").fill(textureName);
    await page.getByRole("button", { name: new RegExp(`^${textureName} `) }).click();
    const bundle = page.locator(".blockBundleCard").filter({ hasText: displayName });
    await expect(bundle).toContainText(`${kind} / 1 faces`);
    await expect(page.getByRole("button", { name: `${blockName} ${faceName} uses ${displayName}`, exact: true })).toBeVisible();
  }
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-shape-specials.png", fullPage: true });
});

test("shows atlas and block bundle overview comparisons", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Carved Pumpkin" })).toBeVisible();

  await page.getByRole("button", { name: "Atlas" }).click();
  await expect(page.getByRole("heading", { name: "Texture Atlas" })).toBeVisible();
  expect(await page.locator(".atlasCard").count()).toBeGreaterThan(20);

  const coalOreCard = page.getByRole("button", { name: "Coal Ore atlas comparison", exact: true });
  await expect(coalOreCard).toContainText("candidate");
  await expect(coalOreCard).toContainText("mclone:block/coal_ore");

  const stoneCard = page.getByRole("button", { name: "Stone atlas comparison", exact: true });
  await expect(stoneCard).toBeVisible();
  await expect(stoneCard).toContainText("Ours");
  await expect(stoneCard).toContainText("Minecraft");
  await expect(stoneCard.locator("img")).toHaveCount(2);
  await stoneCard.click();
  await expect(stoneCard).toHaveAttribute("aria-pressed", "true");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-atlas.png", fullPage: true });

  await page.getByRole("button", { name: "Minecraft Reference" }).click();
  await expect(page.getByRole("heading", { name: "Minecraft Reference Atlas" })).toBeVisible();
  await expect(page.locator(".overviewHeader")).toContainText("vanilla textures");
  await expect(page.locator(".overviewHeader")).toContainText("missing");
  const unfilteredCoverageSummary = await page.locator(".overviewHeader").innerText();
  const unfilteredCoverageMatch = unfilteredCoverageSummary.match(/(\d+) shown \/ (\d+) vanilla textures/u);
  if (!unfilteredCoverageMatch) {
    throw new Error(`Unexpected MC coverage summary: ${unfilteredCoverageSummary}`);
  }
  expect(unfilteredCoverageMatch[1]).toBe(unfilteredCoverageMatch[2]);
  expect(await page.locator(".mcCoverageCard").count()).toBe(Number(unfilteredCoverageMatch[2]));
  await expect(page.getByRole("heading", { name: "Missing In Ours" })).toBeVisible();
  const tabsTopBeforeScroll = await page.locator(".viewTabs").evaluate((element) => element.getBoundingClientRect().top);
  await page.locator(".previewPane").evaluate((element) => {
    element.scrollTop = 1600;
  });
  await expect(page.getByRole("button", { name: "Auto" })).toBeVisible();
  const tabsTopAfterScroll = await page.locator(".viewTabs").evaluate((element) => element.getBoundingClientRect().top);
  expect(Math.abs(tabsTopAfterScroll - tabsTopBeforeScroll)).toBeLessThan(2);
  await page.locator(".previewPane").evaluate((element) => {
    element.scrollTop = 0;
  });
  const acaciaCoverage = page.locator(".mcCoverageCard").filter({ hasText: "Acacia Planks" });
  await expect(acaciaCoverage).toContainText("missing");
  await expect(acaciaCoverage).toContainText("None");
  await page.getByLabel("Search").fill("stone");
  await expect(page.getByRole("heading", { name: "Frozen Coverage" })).toBeVisible();
  const stoneCoverage = page.getByRole("button", { name: "Stone MC coverage", exact: true });
  await expect(stoneCoverage).toContainText("frozen");
  await expect(stoneCoverage).toContainText("Ours");
  await stoneCoverage.click();
  await page.getByLabel("Search").fill("acacia_planks");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-mc-atlas.png", fullPage: true });

  await page.getByLabel("Search").fill("");

  await page.getByRole("button", { name: "Blocks" }).click();
  await expect(page.getByRole("heading", { name: "Block Bundles" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Authored Pack Blocks" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Vanilla-Derived Review Blocks" })).toBeVisible();
  expect(await page.locator(".blockBundleCard").count()).toBeGreaterThan(0);
  const stoneBundle = page.getByRole("region", { name: "stone block bundle", exact: true });
  await expect(stoneBundle).toContainText("authored block");
  await expect(page.getByRole("button", { name: "stone all uses Stone", exact: true })).toBeVisible();
  await page.getByLabel("Search").fill("pumpkin");
  await expect(page.getByText("Carved Pumpkin")).toBeVisible();
  await expect(page.getByText("6 faces")).toBeVisible();
  await expect(page.getByRole("button", { name: "carved-pumpkin north uses Carved Pumpkin", exact: true })).toBeVisible();
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-carved-pumpkin-block.png", fullPage: true });

  await page.getByLabel("Search").fill("grass");
  const grassBundle = page.locator(".blockBundleCard").filter({ hasText: "Grass" }).filter({ hasText: "cross / 1 faces" });
  await expect(grassBundle).toBeVisible();
  await expect(grassBundle.getByRole("button", { name: "grass all uses Grass Cross", exact: true })).toBeVisible();

  await page.getByLabel("Search").fill("redstone-dust-dot");
  const redstoneBundle = page.locator(".blockBundleCard").filter({ hasText: "Redstone Dust Dot" });
  await expect(redstoneBundle).toBeVisible();
  await expect(redstoneBundle).toContainText("flat / 1 faces");
  const redstoneFaceButton = redstoneBundle.getByRole("button", { name: "redstone-dust-dot top uses Redstone Dust Dot", exact: true });
  await expect(redstoneFaceButton).toBeVisible();
  await redstoneFaceButton.click();
  await expect(page.locator(".inspector")).toContainText("redstone_dust_dot");
  await expect(page.locator(".inspector")).toContainText("redstone");
  await expect(page.locator(".inspector")).toContainText("Vanilla Usage");
  await expect(page.locator(".inspector")).toContainText("flat-ground");
  await expect(page.locator(".inspector")).toContainText("minecraft:redstone_wire");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-model-shapes.png", fullPage: true });

  await page.getByLabel("Search").fill("");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-blocks.png", fullPage: true });
});

test("filters the first-party texture lifecycle", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Carved Pumpkin" })).toBeVisible();
  const textureList = page.locator(".textureList");

  await page.getByLabel("Lifecycle filter").selectOption({ label: "Curated" });
  await expect(page.locator(".filterCount")).toHaveText("2 textures");
  await expect(textureList.getByRole("button", { name: /grass_block_top/ })).toBeVisible();
  await expect(textureList.getByRole("button", { name: /stone/ })).toBeVisible();
  await expect(textureList.getByRole("button", { name: /coal_ore/ })).toHaveCount(0);

  await page.getByLabel("Lifecycle filter").selectOption({ label: "Candidate" });
  await expect(page.locator(".filterCount")).toHaveText("33 textures");
  await expect(textureList.getByRole("button", { name: /coal_ore/ })).toBeVisible();
  await expect(textureList.getByRole("button", { name: /grass_block_top/ })).toHaveCount(0);
  await expect(textureList.getByRole("button", { name: /^stone/ })).toHaveCount(0);

  await page.getByLabel("Search").fill("torch");
  await expect(textureList.getByRole("button", { name: /farmstead_wall_torch/ })).toBeVisible();
  await expect(textureList.getByRole("button", { name: /^torch / })).toBeVisible();

  await page.getByLabel("Search").fill("");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-lifecycle-filter.png", fullPage: true });
});

test("uses generated candidates as temporary atlas and block previews", async ({ page }) => {
  await page.goto("/");

  await page.getByLabel("Search").fill("grass_block_top");
  await page.getByRole("button", { name: /grass_block_top/ }).click();
  const archiveCard = page.getByRole("button", { name: "G5101S74 archive candidate" });
  await archiveCard.click();
  await expect(archiveCard).toContainText("Previewing");

  await page.getByRole("button", { name: "Atlas" }).click();
  const grassCard = page.getByRole("button", { name: "Grass Block Top atlas comparison", exact: true });
  await expect(grassCard).toContainText("Ours raw");
  await expect(grassCard).toContainText("Ours tinted");
  await expect(grassCard).toContainText("Minecraft raw");
  await expect(grassCard).toContainText("Minecraft tinted");
  await expect(grassCard).toContainText("preview G5101S74");
  await expect(grassCard.locator("img")).toHaveCount(4);
  await expect(grassCard.locator("img").first()).toHaveAttribute("alt", "G5101S74 raw preview candidate");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-preview-selection.png", fullPage: true });

  await page.getByRole("button", { name: "Blocks" }).click();
  const grassTopFace = page.getByRole("button", { name: "grass-block top uses Grass Block Top", exact: true });
  await expect(grassTopFace).toContainText("Ours raw");
  await expect(grassTopFace).toContainText("Ours tinted");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-preview-blocks.png", fullPage: true });

  await page.getByRole("button", { name: "Detail" }).click();
  await page.getByRole("button", { name: "Clear Preview" }).click();
  await page.getByRole("button", { name: "Atlas" }).click();
  await expect(grassCard).toContainText("Curated Mclone");
  await expect(grassCard).not.toContainText("preview G5101S74");
});

test("supports texture filtering, candidate selection, inspector details, keyboard activation, and reindex preservation", async ({
  page,
}) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Carved Pumpkin" })).toBeVisible();
  await expect(page.locator(".chipGroup")).toContainText("Candidate");
  await expect(page.locator(".inspector")).toContainText("Authoring candidate only");
  await expect(page.getByText("No local generated candidates are present for this texture.")).toBeVisible();

  await page.getByLabel("Search").fill("grass_block_top");
  await page.getByRole("button", { name: /grass_block_top/ }).click();

  await expect(page.getByRole("heading", { name: "Grass Block Top" })).toBeVisible();
  await expect(page.getByText("4 linked to grass_block_top")).toBeVisible();
  await expect(page.locator(".candidateCard")).toHaveCount(4);

  const archiveCard = page.getByRole("button", { name: "G5101S74 archive candidate" });
  await archiveCard.click();
  await expect(archiveCard).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".candidateCard.selected")).toHaveCount(1);

  const inspector = page.locator(".inspector");
  await expect(inspector).toContainText("neutral mean <= 0.005");
  await expect(inspector).toContainText("G5101S74");
  await expect(inspector).toContainText("grass-top-tufts");
  await expect(inspector).toContainText("playwright-fixture-model");
  await expect(inspector).toContainText("photographic top-down macro grass");
  await expect(inspector).toContainText("playwright archive accepted");
  await expect(inspector).toContainText("diffusion-archive");
  await expect(page.locator(".paletteSwatches span")).toHaveCount(4);

  await page.getByRole("button", { name: "Use as Provisional" }).click();
  await expect(page.getByText("Using grass_block_top as Provisional Mclone")).toBeVisible();
  await expect(page.locator(".chipGroup")).toContainText("Provisional");
  await expect(page.locator(".lifecycleCard").filter({ hasText: "Provisional" }).locator("img")).toHaveCount(1);

  await page.getByRole("button", { name: "Accept as Curated" }).click();
  await expect(page.getByText("Accepted grass_block_top as Curated Mclone")).toBeVisible();
  await expect(page.locator(".chipGroup")).toContainText("Curated");
  await expect(page.locator(".lifecycleCard").filter({ hasText: "Curated" }).locator("img")).toHaveCount(1);
  await expect(page.locator(".lifecycleCard").filter({ hasText: "Minecraft Reference" })).toContainText("local · read-only");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-lifecycle.png", fullPage: true });

  const diffusionCard = page.getByRole("button", { name: "G5102S62 diffusion candidate" });
  await diffusionCard.focus();
  await page.keyboard.press("Enter");
  await expect(diffusionCard).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".candidateCard.selected")).toContainText("G5102S62");
  await expect(inspector).toContainText("seed5102");
  await expect(inspector).toContainText("untriaged");

  await Promise.all([
    page.waitForResponse((response) => response.url().includes("/api/reindex") && response.status() === 200),
    page.getByRole("button", { name: "Reindex" }).click(),
  ]);
  await expect(page.getByRole("button", { name: "G5102S62 diffusion candidate" })).toHaveAttribute("aria-pressed", "true");

  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-a2.png", fullPage: true });
});

test("clears candidate detail when selecting a texture without generated candidates", async ({ page }) => {
  await page.goto("/");

  await page.getByLabel("Search").fill("grass_block_top");
  await page.getByRole("button", { name: /grass_block_top/ }).click();
  await page.getByRole("button", { name: "G5101S74 archive candidate" }).click();
  await expect(page.locator(".inspector")).toContainText("G5101S74");

  await page.getByLabel("Search").fill("carved_pumpkin");
  await page.getByRole("button", { name: /^carved_pumpkin / }).click();

  await expect(page.getByRole("heading", { name: "Carved Pumpkin" })).toBeVisible();
  await expect(page.getByText("No local generated candidates are present for this texture.")).toBeVisible();
  await expect(page.locator(".inspector")).toContainText("No generated candidate selected.");
  await expect(page.locator(".candidateCard.selected")).toHaveCount(0);
});

async function layoutMetrics(page: import("@playwright/test").Page): Promise<{
  documentScrollHeight: number;
  previewBottom: number;
  previewTop: number;
  textureListScrollable: boolean;
  textureListScrollTop: number;
  viewportHeight: number;
}> {
  return await page.evaluate(() => {
    const textureList = document.querySelector(".textureList");
    const previewPane = document.querySelector(".previewPane");
    if (!(textureList instanceof HTMLElement) || !(previewPane instanceof HTMLElement)) {
      throw new Error("Missing texture lab layout elements");
    }
    const previewRect = previewPane.getBoundingClientRect();
    return {
      documentScrollHeight: Math.max(document.documentElement.scrollHeight, document.body.scrollHeight),
      previewBottom: Math.round(previewRect.bottom),
      previewTop: Math.round(previewRect.top),
      textureListScrollable: textureList.scrollHeight > textureList.clientHeight + 8,
      textureListScrollTop: Math.round(textureList.scrollTop),
      viewportHeight: window.innerHeight,
    };
  });
}

function meanRgb(image: RgbaImage): [number, number, number] {
  let red = 0;
  let green = 0;
  let blue = 0;
  const count = image.width * image.height;
  for (let index = 0; index < image.data.length; index += 4) {
    red += image.data[index]!;
    green += image.data[index + 1]!;
    blue += image.data[index + 2]!;
  }
  return [red / count, green / count, blue / count];
}

function normalizedAlphaCentroidX(image: RgbaImage): number {
  let alphaSum = 0;
  let weightedX = 0;
  for (let y = 0; y < image.height; y += 1) {
    for (let x = 0; x < image.width; x += 1) {
      const alpha = image.data[(y * image.width + x) * 4 + 3]!;
      alphaSum += alpha;
      weightedX += x * alpha;
    }
  }
  return weightedX / alphaSum / (image.width - 1);
}

function saturation([red, green, blue]: [number, number, number]): number {
  const max = Math.max(red, green, blue);
  const min = Math.min(red, green, blue);
  return max === 0 ? 0 : (max - min) / max;
}
