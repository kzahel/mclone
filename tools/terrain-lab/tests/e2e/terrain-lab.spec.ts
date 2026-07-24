import { expect, test } from "@playwright/test";

test("generates terrain, round-trips controls, and completes comparison", async ({
  page,
}, testInfo) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });

  await page.goto(
    "/terrain/?seed=-98765&x=-304&z=336&spacing=32&source=split&view=3d&layer=terrain",
  );
  await expect(page.locator("[data-testid='lab-status']")).toContainText("ready");
  await waitForCurrentComparison(page);

  const sourceControls = page.getByTestId("preview-source-controls");
  await expect(sourceControls).toBeVisible();
  await expect(sourceControls).toContainText("exact same world coordinates");
  const diagnostics = page.locator("[data-testid='terrain-diagnostics']");
  await expect(diagnostics).not.toContainText("Base mean Δpending");
  const shell = page.locator(".appShell");
  await expect(shell).toHaveAttribute("data-compare-layout", "side-by-side");
  await expect(shell).toHaveAttribute("data-vertex-count", "49152");
  expect(Number(await shell.getAttribute("data-base-mean-error"))).toBeLessThanOrEqual(0.01);
  expect(Number(await shell.getAttribute("data-base-p95-error"))).toBeLessThanOrEqual(0.01);
  expect(Number(await shell.getAttribute("data-ocean-agreement"))).toBeGreaterThanOrEqual(0.999);
  expect(Number(await shell.getAttribute("data-continentalness-error"))).toBeLessThanOrEqual(0.001);
  const canvas = page.locator("canvas[aria-label='Live GPU terrain preview']");
  await expect(canvas).toBeVisible();
  await canvas.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-initial.png`,
  });

  const initialRevision = Number(await shell.getAttribute("data-render-revision"));
  const initialYaw = Number(await shell.getAttribute("data-camera-yaw"));
  const initialPitch = Number(await shell.getAttribute("data-camera-pitch"));
  const initialUrl = page.url();
  const stage = page.getByTestId("terrain-stage");
  const stageBox = await stage.boundingBox();
  const sourceControlsBox = await sourceControls.boundingBox();
  expect(stageBox).not.toBeNull();
  expect(sourceControlsBox).not.toBeNull();
  expect(sourceControlsBox!.y + sourceControlsBox!.height).toBeLessThanOrEqual(stageBox!.y + 1);
  await page.mouse.move(stageBox!.x + stageBox!.width * 0.5, stageBox!.y + stageBox!.height * 0.5);
  await page.mouse.down();
  await page.mouse.move(
    stageBox!.x + stageBox!.width * 0.65,
    stageBox!.y + stageBox!.height * 0.42,
  );
  await page.mouse.up();
  await expect.poll(
    async () => Number(await shell.getAttribute("data-camera-yaw")),
  ).not.toBe(initialYaw);
  await expect.poll(
    async () => Number(await shell.getAttribute("data-camera-pitch")),
  ).toBeLessThan(initialPitch);
  expect(page.url()).toBe(initialUrl);
  await page.getByRole("button", { name: "Reset 3D camera" }).click();
  await expect.poll(
    async () => Number(await shell.getAttribute("data-camera-yaw")),
  ).toBeCloseTo(Math.PI / 4, 5);

  await page.getByLabel("Diagnostic layer").selectOption("error");
  await expect(page).toHaveURL(/layer=error/u);
  await expect.poll(
    async () => Number(await shell.getAttribute("data-render-revision")),
  ).toBeGreaterThan(initialRevision);
  await waitForCurrentComparison(page);

  const errorRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByRole("button", { name: "Map", exact: true }).click();
  await page.getByRole("button", { name: "Zoom out" }).click();
  await expect(page).toHaveURL(/view=map/u);
  await expect(page).toHaveURL(/spacing=64/u);
  await expect.poll(
    async () => Number(await shell.getAttribute("data-render-revision")),
  ).toBeGreaterThan(errorRevision);
  await waitForCurrentComparison(page);
  await page.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-map-error.png`,
    fullPage: true,
  });

  expect(pageErrors).toEqual([]);
});

async function waitForCurrentComparison(page: import("@playwright/test").Page): Promise<void> {
  await page.waitForFunction(() => {
    const shell = document.querySelector(".appShell");
    const render = Number(shell?.getAttribute("data-render-revision") ?? "0");
    const comparison = Number(shell?.getAttribute("data-comparison-revision") ?? "0");
    const diagnostics = document.querySelector("[data-testid='terrain-diagnostics']");
    return render > 0
      && comparison === render
      && !diagnostics?.textContent?.includes("pending");
  });
}
