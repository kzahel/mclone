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
  await expect(page.locator(".summaryItem").first()).not.toContainText("0");
  await expect(page.locator(".summaryItem").filter({ hasText: "runtime" })).toContainText("3");
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
  await expect(page.locator(".resultCount")).toHaveText("3 figures");
  await expect(page.locator("[data-catalog-name='player']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='chicken']")).toBeVisible();
  await expect(page.locator("[data-catalog-name='upright_bear']")).toBeVisible();
  await expect(page.locator("[data-runtime-promoted='false']")).toHaveCount(0);
  await page.locator("[data-catalog-name='chicken']").click();
  await expect(page.locator(".promotionStatus")).toContainText("Runtime promoted");
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
