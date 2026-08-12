import { expect, test } from "@playwright/test";

test("browses hash-checked figures with animation and camera controls", async ({ page }) => {
  const browserErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      browserErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => browserErrors.push(error.stack ?? error.message));

  await page.goto("/animals/?figure=king_cobra&clip=slither");
  const canvas = page.locator("canvas[data-figure='king_cobra']");
  await expect(canvas).toBeVisible();
  await expect(page.locator(".viewerHeader h2")).toHaveText("King Cobra");
  await expect(page).toHaveURL(/figure=king_cobra/);
  await expect(page.locator(".catalogRow.selected")).toHaveAttribute("data-catalog-name", "king_cobra");
  await expect(page.locator(".summaryItem").first()).toContainText("202");
  await expect(page.locator(".summaryItem").filter({ hasText: "runtime" })).toContainText("6");
  await expect(page.locator(".promotionStatus")).toContainText("Asset Lab only");

  const firstTime = Number(await canvas.getAttribute("data-time"));
  await page.waitForTimeout(220);
  const secondTime = Number(await canvas.getAttribute("data-time"));
  expect(secondTime).toBeGreaterThan(firstTime);

  await page.getByRole("button", { name: "Pause animation" }).click();
  const pausedTime = Number(await canvas.getAttribute("data-time"));
  await page.waitForTimeout(180);
  expect(Number(await canvas.getAttribute("data-time"))).toBeCloseTo(pausedTime, 3);

  const beforeCamera = await canvas.screenshot();
  await page.getByRole("button", { name: "Front" }).click();
  await expect(canvas).toHaveAttribute("data-camera-preset", "front");
  await page.waitForTimeout(100);
  const afterCamera = await canvas.screenshot();
  expect(afterCamera.equals(beforeCamera)).toBe(false);

  const timeline = page.getByRole("slider", { name: "Animation time" });
  await timeline.focus();
  await page.keyboard.press("Home");
  await expect(canvas).toHaveAttribute("data-time", "0.0000");
  await page.keyboard.press("ArrowRight");
  await expect(canvas).toHaveAttribute("data-time", "0.0010");
  await page.getByRole("button", { name: "Play animation" }).click();

  const promotionFilter = page.getByRole("combobox", { name: "Runtime status" });
  await promotionFilter.selectOption("runtime");
  await expect(page.locator(".resultCount")).toHaveText("6 figures");
  await expect(page.locator("[data-catalog-name='player']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='cow']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='chicken']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='mallard_duck']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='upright_bear']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='deer']")).toBeVisible();
  await expect(page.locator("[data-runtime-promoted='false']")).toHaveCount(0);
  await page.locator("[data-catalog-name='chicken']").click();
  await expect(page.locator(".promotionStatus")).toContainText("Live gameplay asset");
  await expect(page.locator(".promotionStatus")).toContainText("mclone:chicken");
  await expect(page.locator(".promotionStatus")).toContainText("assets/mclone/figures/chicken.figure.json");
  await promotionFilter.selectOption("asset-lab");
  await expect(page.locator("[data-catalog-name='chicken']")).toHaveCount(0);
  await expect(page.locator("[data-catalog-name='king_cobra']")).toBeVisible();
  await promotionFilter.selectOption("all");

  const search = page.getByRole("searchbox", { name: "Search" });
  await search.fill("owl");
  const owlRow = page.locator("[data-catalog-name='owl']");
  await expect(owlRow).toBeVisible();
  await owlRow.focus();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/figure=owl/);
  await expect(page.locator("canvas[data-figure='owl']")).toBeVisible();
  await expect(page.locator("canvas")).toHaveCount(1);

  await page.getByRole("button", { name: "Dark" }).click();
  await expect(page.locator(".appShell")).toHaveAttribute("data-theme", "dark");
  await page.screenshot({ path: "/tmp/mclone-animal-catalogue-desktop.png", fullPage: true });
  expect(browserErrors).toEqual([]);
});

test("reviews static semantic props without polluting the creature catalogue", async ({ page }) => {
  const browserErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      browserErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => browserErrors.push(error.stack ?? error.message));

  await page.goto("/animals/?view=props&figure=mallard_nest");
  await expect(page.getByRole("heading", { name: "Semantic Prop Review" })).toBeVisible();
  await expect(page.locator(".resultCount")).toHaveText("2 props");
  await expect(page.locator("canvas[data-figure='mallard_nest']")).toBeVisible();
  await expect(page.getByText("Static semantic asset · no animation clips")).toBeVisible();
  await expect(page.locator(".classificationSection")).toContainText("World prop");
  await expect(page.locator(".classificationSection")).toContainText("Ground");
  await expect(page.locator(".promotionStatus")).toContainText("mclone:mallard_nest");
  await expect(page.locator(".promotionStatus")).toContainText("Live gameplay");
  await expect(page.locator(".promotionStatus")).toContainText("ordinary live gameplay instantiation");
  await expect(page.locator(".summaryItem").filter({ hasText: "live" })).toContainText("2");
  await expect(page.getByRole("combobox", { name: "Group" })).toHaveCount(0);
  await expect(page.locator("[data-catalog-name='chicken']")).toHaveCount(0);
  await page.screenshot({ path: "/tmp/mclone-semantic-props-nest.png", fullPage: true });

  await page.locator("[data-catalog-name='mallard_feather']").click();
  await expect(page.locator("canvas[data-figure='mallard_feather']")).toBeVisible();
  await expect(page.locator(".classificationSection")).toContainText("Item center");
  await page.getByRole("button", { name: "Top" }).click();
  await expect(page).toHaveURL(/camera=top/);
  await page.screenshot({ path: "/tmp/mclone-semantic-props-feather.png", fullPage: true });

  await page.goto("/animals/?figure=mallard_nest");
  await expect(page.getByRole("heading", { name: "Creature Catalogue" })).toBeVisible();
  await expect(page.locator(".resultCount")).toHaveText("202 figures");
  await expect(page.locator("[data-catalog-name='mallard_nest']")).toHaveCount(0);
  await expect(page.locator("canvas[data-figure='chicken']")).toBeVisible();
  expect(browserErrors).toEqual([]);
});

test("keeps the complete catalogue usable at a mobile viewport", async ({ page }) => {
  const browserErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      browserErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => browserErrors.push(error.stack ?? error.message));
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/animals/?figure=bee&clip=hover");
  await expect(page.locator("canvas[data-figure='bee']")).toBeVisible();
  await expect(page.locator(".catalogRow.selected")).toHaveAttribute("data-catalog-name", "bee");
  const dimensions = await page.evaluate(() => ({
    clientWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
  }));
  expect(dimensions.scrollWidth).toBe(dimensions.clientWidth);
  await page.screenshot({ path: "/tmp/mclone-animal-catalogue-mobile.png", fullPage: true });
  expect(browserErrors).toEqual([]);
});

test("filters and inspects typed creature classifications", async ({ page }) => {
  const browserErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      browserErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => browserErrors.push(error.stack ?? error.message));

  await page.goto("/animals/?figure=gargoyle&clip=stone_stalk");
  await expect(page.locator("canvas[data-figure='gargoyle']")).toBeVisible();

  const groupFilter = page.getByRole("combobox", { name: "Group" });
  await groupFilter.selectOption("monster");
  await expect(page.locator(".resultCount")).toHaveText("15 figures");
  await expect(page.locator("[data-catalog-name='skeleton']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='cutout_skeleton']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='slime']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='gargoyle']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='book_mimic']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='ceiling_angler']")).toBeVisible();

  const classification = page.locator(".classificationSection");
  await expect(classification).toContainText("Fantasy");
  await expect(classification).toContainText("Monster");
  await expect(classification).toContainText("Construct");
  await expect(classification).toContainText("Land");
  await expect(classification).toContainText("Air");
  await expect(classification).toContainText("Hostile");
  await page.screenshot({
    path: "/tmp/mclone-creature-catalogue-classification.png",
    fullPage: true,
  });

  const bodyPlanFilter = page.getByRole("combobox", { name: "Body plan" });
  await bodyPlanFilter.selectOption("blob");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='slime']")).toBeVisible();
  await bodyPlanFilter.selectOption("all");

  const dispositionFilter = page.getByRole("combobox", { name: "Disposition" });
  await dispositionFilter.selectOption("hostile");
  await expect(page.locator(".resultCount")).toHaveText("15 figures");
  await expect(page.locator("[data-catalog-name='witch']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='werewolf']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='grave_crawler']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='book_mimic']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='ceiling_angler']")).toBeVisible();
  await dispositionFilter.selectOption("all");
  await groupFilter.selectOption("all");

  const search = page.getByRole("searchbox", { name: "Search" });
  await search.fill("scary");
  await expect(page.locator(".resultCount")).toHaveText("15 figures");
  await expect(page.locator("[data-catalog-name='witch']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='werewolf']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='grave_crawler']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='book_mimic']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='ceiling_angler']")).toBeVisible();
  await page.locator("[data-catalog-name='witch']").click();
  await expect(page.locator("canvas[data-figure='witch']")).toBeVisible();
  await page.screenshot({
    path: "/tmp/mclone-scary-four/catalogue-scary-filter.png",
    fullPage: true,
  });

  await search.fill("undead");
  await expect(page.locator(".resultCount")).toHaveText("6 figures");
  await expect(page.locator("[data-catalog-name='skeleton']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='cutout_skeleton']")).toBeVisible();

  await search.fill("alpha-cutout");
  await expect(page.locator(".resultCount")).toHaveText("2 figures");
  await page.locator("[data-catalog-name='cutout_skeleton']").click();
  await expect(page.locator("canvas[data-figure='cutout_skeleton']")).toBeVisible();
  await expect(classification).toContainText("Alpha cutout");
  await page.screenshot({
    path: "/tmp/mclone-creature-catalogue-alpha-cutout.png",
    fullPage: true,
  });

  await search.fill("alpha-blend");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await page.locator("[data-catalog-name='ghost_translucent']").click();
  await expect(page.locator("canvas[data-figure='ghost_translucent']")).toBeVisible();
  const rendering = page.locator(".renderingSection");
  await expect(rendering).toContainText("Blend");
  await expect(rendering).toContainText("Additive");
  await page.screenshot({
    path: "/tmp/mclone-creature-catalogue-alpha-blend.png",
    fullPage: true,
  });

  await search.fill("alpha-dither");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await page.locator("[data-catalog-name='ghost_dither']").click();
  await expect(page.locator("canvas[data-figure='ghost_dither']")).toBeVisible();
  await expect(rendering).toContainText("Mask");

  await search.fill("harvest");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await page.locator("[data-catalog-name='scarecrow']").click();
  await expect(page.locator("canvas[data-figure='scarecrow']")).toBeVisible();
  await expect(classification).toContainText("Construct");
  await expect(rendering).toContainText("Mask");
  await page.screenshot({
    path: "/tmp/mclone-scary-three/catalogue-scarecrow.png",
    fullPage: true,
  });

  await search.fill("dungeon");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='mimic']")).toBeVisible();
  await search.fill("mimic");
  await expect(page.locator(".resultCount")).toHaveText("2 figures");
  await expect(page.locator("[data-catalog-name='mimic']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='book_mimic']")).toBeVisible();
  await search.fill("ceiling angler");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='ceiling_angler']")).toBeVisible();
  await search.fill("decay");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='zombie']")).toBeVisible();
  await search.fill("mandrake");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='mandrake']")).toBeVisible();
  await search.fill("cactus");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='walking_cactus']")).toBeVisible();
  await search.fill("scholar");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='owl_scholar']")).toBeVisible();
  await search.fill("jackalope");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='jackalope']")).toBeVisible();
  await search.fill("sunflower");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='walking_sunflower']")).toBeVisible();
  await search.fill("mossback");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='mossback_tortoise']")).toBeVisible();
  await search.fill("owlbear");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='owlbear']")).toBeVisible();
  await search.fill("cockatrice");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='cockatrice']")).toBeVisible();
  await search.fill("dryad");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='young_dryad']")).toBeVisible();
  await search.fill("hybrid");
  await expect(page.locator(".resultCount")).toHaveText("6 figures");
  await expect(page.locator("[data-catalog-name='centaur']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='chimera']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='hippocampus']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='griffin']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='harpy']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='ammit']")).toBeVisible();
  await page.locator("[data-catalog-name='centaur']").click();
  await expect(classification).toContainText("Humanoid");
  await expect(classification).toContainText("Biped");
  await expect(classification).toContainText("Quadruped");
  await page.locator("[data-catalog-name='chimera']").click();
  await expect(classification).toContainText("Serpentine");
  await page.locator("[data-catalog-name='hippocampus']").click();
  await expect(classification).toContainText("Swimmer");
  await expect(classification).toContainText("Water");
  await page.locator("[data-catalog-name='griffin']").click();
  await expect(classification).toContainText("Winged");
  await expect(classification).toContainText("Air");
  await page.locator("[data-catalog-name='harpy']").click();
  await expect(classification).toContainText("Humanoid");
  await expect(classification).toContainText("Biped");
  await page.locator("[data-catalog-name='ammit']").click();
  await expect(classification).toContainText("Quadruped");
  await expect(classification).toContainText("Water");

  await search.fill("");
  await groupFilter.selectOption("plant");
  await expect(page.locator(".resultCount")).toHaveText("6 figures");
  await expect(page.locator("[data-catalog-name='walking_banyan']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='maw_orchid']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='mandrake']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='walking_cactus']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='walking_sunflower']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='young_dryad']")).toBeVisible();

  await groupFilter.selectOption("fungus");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await expect(page.locator("[data-catalog-name='lantern_mycelium']")).toBeVisible();
  await page.locator("[data-catalog-name='lantern_mycelium']").click();
  await expect(page.locator("canvas[data-figure='lantern_mycelium']")).toBeVisible();
  await expect(classification).toContainText("Fungus");
  await expect(classification).toContainText("Rooted");
  await expect(classification).toContainText("Colony");
  await expect(rendering).toContainText("Blend");
  await expect(rendering).toContainText("Additive");

  await groupFilter.selectOption("all");
  await bodyPlanFilter.selectOption("rooted");
  await expect(page.locator(".resultCount")).toHaveText("7 figures");
  await bodyPlanFilter.selectOption("colony");
  await expect(page.locator(".resultCount")).toHaveText("1 figure");
  await page.screenshot({
    path: "/tmp/mclone-living-growths/catalogue-colony-filter.png",
    fullPage: true,
  });
  expect(browserErrors).toEqual([]);
});

test("runs one-shot actions, holds their final pose, and follows nextClip", async ({ page }) => {
  const browserErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      browserErrors.push(message.text());
    }
  });
  page.on("pageerror", (error) => browserErrors.push(error.stack ?? error.message));

  await page.goto("/animals/?figure=roly_poly&clip=crawl");
  const canvas = page.locator("canvas[data-figure='roly_poly']");
  await expect(canvas).toBeVisible();
  await expect(page.getByRole("button", { name: "Roll up" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Unroll" })).toBeVisible();
  await expect(page.locator(".clipSelect optgroup[label='Locomotion'] option")).toHaveText(["Crawl"]);
  await expect(page.locator(".clipSelect optgroup[label='Actions'] option")).toHaveText([
    "Roll up",
    "Unroll",
  ]);
  const motionFilter = page.getByRole("combobox", { name: "Motion" });
  await motionFilter.selectOption("action");
  await expect(page.locator("[data-catalog-name='roly_poly']")).toBeVisible();
  await motionFilter.selectOption("all");

  await page.getByRole("button", { name: "Roll up" }).click();
  await expect(page).toHaveURL(/clip=roll_up/);
  await expect(canvas).toHaveAttribute("data-clip", "roll_up");
  await expect(canvas).toHaveAttribute("data-playing", "false", { timeout: 2_000 });
  await expect(canvas).toHaveAttribute("data-time", "0.7200");
  await expect(page.getByRole("button", { name: "Play animation" })).toBeVisible();
  await page.screenshot({
    path: "/tmp/mclone-roly-poly-action-catalogue.png",
    fullPage: true,
  });

  await page.getByRole("button", { name: "Roll up" }).click();
  await expect(canvas).toHaveAttribute("data-playing", "true");
  expect(Number(await canvas.getAttribute("data-time"))).toBeLessThan(0.25);
  await expect(canvas).toHaveAttribute("data-playing", "false", { timeout: 2_000 });

  await page.getByRole("button", { name: "Unroll" }).click();
  await expect(page).toHaveURL(/clip=crawl/, { timeout: 2_000 });
  await expect(canvas).toHaveAttribute("data-clip", "crawl");
  await expect(canvas).toHaveAttribute("data-playing", "true");
  expect(browserErrors).toEqual([]);
});
