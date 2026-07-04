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
  const imageResponse = await request.get(`/api/image?path=${encodeURIComponent(grass.images.currentExport.path)}`);
  await expect(imageResponse).toBeOK();
  expect(imageResponse.headers()["content-type"]).toBe("image/png");
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
