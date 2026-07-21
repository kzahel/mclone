import { expect, test } from "@playwright/test";

test("loads the checked cottage with guide and presentation controls", async ({ page }) => {
  const browserErrors: string[] = [];
  page.on("console", (message) => { if (message.type() === "error") browserErrors.push(message.text()); });
  page.on("pageerror", (error) => browserErrors.push(error.stack ?? error.message));

  await page.goto("/structures/?structure=farmstead-cottage-a-v2&camera=three-quarter&layer=14");
  const canvas = page.locator("canvas[data-structure='farmstead-cottage-a-v2']");
  await expect(canvas).toBeVisible();
  await expect(canvas).toHaveAttribute("data-viewer-status", "ready");
  await expect(page.locator(".viewerHeader h2")).toHaveText("Warm Oak Cottage");
  await expect(page.locator(".statusBadge").last()).toContainText("Runtime parity canary");
  await expect(page.locator("canvas")).toHaveCount(1);
  await expect(page.getByRole("slider", { name: "Build layer" })).toHaveValue("14");
  await expect(page.locator(".materialList li")).not.toHaveCount(0);
  await page.screenshot({ path: "/tmp/mclone-structure-lab-desktop-default.png", fullPage: true });

  const full = await canvas.screenshot();
  await page.getByRole("slider", { name: "Build layer" }).fill("5");
  await expect(canvas).toHaveAttribute("data-layer", "5");
  const sliced = await canvas.screenshot();
  expect(sliced.equals(full)).toBe(false);
  await expect(page).toHaveURL(/layer=5/);

  await page.getByRole("button", { name: "Front" }).click();
  await expect(canvas).toHaveAttribute("data-camera-preset", "front");
  await page.getByRole("button", { name: "Mirror" }).click();
  await expect(canvas).toHaveAttribute("data-mirror", "true");
  await page.getByRole("button", { name: "Bounds" }).click();
  await page.getByRole("button", { name: "Roof" }).click();
  await expect(page).toHaveURL(/hide=roof/);
  await page.getByRole("button", { name: "Dark" }).click();
  await expect(page.locator(".appShell")).toHaveAttribute("data-theme", "dark");
  await page.screenshot({ path: "/tmp/mclone-structure-lab-desktop.png", fullPage: true });
  expect(browserErrors).toEqual([]);
});

test("keeps the Structure Lab usable on a narrow phone", async ({ page }) => {
  const browserErrors: string[] = [];
  page.on("console", (message) => { if (message.type() === "error") browserErrors.push(message.text()); });
  page.on("pageerror", (error) => browserErrors.push(error.stack ?? error.message));
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/structures/?structure=farmstead-cottage-a-v2");
  const canvas = page.locator("canvas[data-structure='farmstead-cottage-a-v2']");
  await expect(canvas).toHaveAttribute("data-viewer-status", "ready");
  await expect(page.getByRole("slider", { name: "Build layer" })).toHaveValue("14");
  const dimensions = await page.evaluate(() => ({
    clientWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
  }));
  expect(dimensions.scrollWidth).toBe(dimensions.clientWidth);
  await page.screenshot({ path: "/tmp/mclone-structure-lab-mobile.png", fullPage: true });
  expect(browserErrors).toEqual([]);
});
