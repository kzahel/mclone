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
  expect(index.summary.authoredTextures).toBe(81);
  expect(index.summary.candidateCount).toBe(4);
  expect(index.summary.associatedCandidateCount).toBe(4);
  expect(index.summary.archivedCandidateCount).toBe(1);
  expect(index.summary.curatedSelectionCount).toBe(0);
  expect(index.summary.frozenTextureCount).toBe(2);
  expect(index.curation.selectedCount).toBe(0);
  expect(index.curation.manifestPath).toContain("generated-assets/texture-lab-playwright/curation/selections.v1.json");

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
  expect(pointedDripstone?.images.minecraftReference.exists).toBe(true);
  expect(pointedDripstone?.images.minecraftReference.path).toContain("reference-composites/pointed_dripstone-up-down-thickness.png");
  const pointedReferenceResponse = await request.get(
    `/api/image?path=${encodeURIComponent(pointedDripstone.images.minecraftReference.path)}`,
  );
  await expect(pointedReferenceResponse).toBeOK();
  const pointedReference = decodePng(await pointedReferenceResponse.body());
  expect(`${pointedReference.width}x${pointedReference.height}`).toBe("32x80");

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
  await expect(page.getByRole("heading", { name: "Andesite" })).toBeVisible();
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-dark-mode.png", fullPage: true });

  await page.getByRole("button", { name: "Switch to light mode" }).click();
  await expect(shell).toHaveAttribute("data-theme", "light");
  await expect(page.getByRole("button", { name: "Switch to dark mode" })).toHaveText("Dark");

  await page.emulateMedia({ colorScheme: "light" });
  await expect(shell).toHaveAttribute("data-theme", "light");
});

test("keeps the texture list in its own scroll pane", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Andesite" })).toBeVisible();

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

test("shows atlas and block bundle overview comparisons", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Andesite" })).toBeVisible();

  await page.getByRole("button", { name: "Atlas" }).click();
  await expect(page.getByRole("heading", { name: "Texture Atlas" })).toBeVisible();
  expect(await page.locator(".atlasCard").count()).toBeGreaterThan(20);

  const stoneCard = page.getByRole("button", { name: "Stone atlas comparison", exact: true });
  await expect(stoneCard).toBeVisible();
  await expect(stoneCard).toContainText("Ours");
  await expect(stoneCard).toContainText("Minecraft");
  await expect(stoneCard.locator("img")).toHaveCount(2);
  await stoneCard.click();
  await expect(stoneCard).toHaveAttribute("aria-pressed", "true");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-atlas.png", fullPage: true });

  await page.getByLabel("Search").fill("pointed_dripstone");
  const pointedCard = page.getByRole("button", { name: "Pointed Dripstone atlas comparison", exact: true });
  await expect(pointedCard).toBeVisible();
  await expect(pointedCard).not.toContainText("Missing");
  await expect(pointedCard.locator("img")).toHaveCount(2);
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-pointed-dripstone-reference.png", fullPage: true });
  await page.getByLabel("Search").fill("");

  await page.getByRole("button", { name: "Blocks" }).click();
  await expect(page.getByRole("heading", { name: "Block Bundles" })).toBeVisible();
  expect(await page.locator(".blockBundleCard").count()).toBeGreaterThan(0);
  await expect(page.getByRole("button", { name: "stone all uses Stone", exact: true })).toBeVisible();
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-blocks.png", fullPage: true });
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
  await expect(grassCard).toContainText("frozen G5101S74");
  await expect(grassCard).not.toContainText("preview G5101S74");
});

test("supports texture filtering, candidate selection, inspector details, keyboard activation, and reindex preservation", async ({
  page,
}) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Andesite" })).toBeVisible();
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

  await page.getByRole("button", { name: "Select for Pack" }).click();
  await expect(page.getByText("Selected candidate for grass_block_top")).toBeVisible();
  await expect(archiveCard).toContainText("Pack");
  await expect(page.getByText("pack G5101S74")).toBeVisible();

  await page.getByRole("button", { name: "Request Freeze" }).click();
  await expect(page.locator(".statusBanner")).toContainText("Freeze request written:");
  await expect(page.locator(".statusBanner")).toContainText("freeze-requests");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-freeze-request.png", fullPage: true });

  await page.getByRole("button", { name: "Apply Pack" }).click();
  await expect(page.getByText("Applied 1 selection to generated pack")).toBeVisible();
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-curation-pack.png", fullPage: true });

  await page.getByRole("button", { name: "Clear Pack" }).click();
  await expect(page.getByText("Cleared pack selection for grass_block_top")).toBeVisible();

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

  await page.getByLabel("Search").fill("andesite");
  await page.getByRole("button", { name: /andesite/ }).click();

  await expect(page.getByRole("heading", { name: "Andesite" })).toBeVisible();
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

function saturation([red, green, blue]: [number, number, number]): number {
  const max = Math.max(red, green, blue);
  const min = Math.min(red, green, blue);
  return max === 0 ? 0 : (max - min) / max;
}
