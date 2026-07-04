import { expect, test } from "@playwright/test";

test("indexes authored textures, generated candidates, and allowlisted images", async ({ request }) => {
  const indexResponse = await request.get("/api/index");
  await expect(indexResponse).toBeOK();
  const index = await indexResponse.json();

  expect(index.pack.name).toBe("mclone-default");
  expect(index.summary.authoredTextures).toBe(81);
  expect(index.summary.candidateCount).toBe(4);
  expect(index.summary.associatedCandidateCount).toBe(4);
  expect(index.summary.archivedCandidateCount).toBe(1);

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

  const grass = index.textures.find((texture: { name: string }) => texture.name === "grass_block_top");
  expect(grass?.images.currentExport.exists).toBe(true);
  expect(grass?.images.minecraftReference.exists).toBe(true);
  const imageResponse = await request.get(`/api/image?path=${encodeURIComponent(grass.images.currentExport.path)}`);
  await expect(imageResponse).toBeOK();
  expect(imageResponse.headers()["content-type"]).toBe("image/png");
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
  await expect(grassCard).toContainText("Ours · G5101S74");
  await expect(grassCard).toContainText("preview G5101S74");
  await expect(grassCard.locator("img").first()).toHaveAttribute("alt", "G5101S74 preview candidate");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-preview-selection.png", fullPage: true });

  await page.getByRole("button", { name: "Blocks" }).click();
  const grassTopFace = page.getByRole("button", { name: "grass-block top uses Grass Block Top", exact: true });
  await expect(grassTopFace).toContainText("Ours · G5101S74");
  await page.screenshot({ path: "/tmp/mclone-texture-lab-playwright-preview-blocks.png", fullPage: true });

  await page.getByRole("button", { name: "Detail" }).click();
  await page.getByRole("button", { name: "Clear Preview" }).click();
  await page.getByRole("button", { name: "Atlas" }).click();
  await expect(grassCard).not.toContainText("G5101S74");
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
  await expect(inspector).toContainText("G5101S74");
  await expect(inspector).toContainText("grass-top-tufts");
  await expect(inspector).toContainText("playwright-fixture-model");
  await expect(inspector).toContainText("photographic top-down macro grass");
  await expect(inspector).toContainText("playwright archive accepted");
  await expect(inspector).toContainText("diffusion-archive");
  await expect(page.locator(".paletteSwatches span")).toHaveCount(4);

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
