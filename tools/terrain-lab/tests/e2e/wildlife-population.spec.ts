import { expect, test } from "@playwright/test";

test("wildlife population is deterministic, inspectable, and responsive", async ({
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
    "/terrain/?profile=mclone-overworld-v1&seed=-98765&x=0&z=-2048"
      + "&blocks=1024&panes=wildlife&view=map",
    { waitUntil: "networkidle" },
  );
  const shell = page.locator(".appShell");
  const stage = page.getByTestId("wildlife-population-stage");
  const canvas = stage.locator("canvas");
  await expect(shell).toHaveAttribute("data-wildlife-ready", "true");
  await expect(stage).toHaveAttribute("data-render-ready", "true");
  await expect(page.getByTestId("lab-status")).toContainText("ready");
  const legend = page.getByTestId("wildlife-map-legend");
  await expect(legend).toBeVisible();
  await expect(legend).toContainText("Cell habitat");
  await expect(legend).toContainText("Wetland margin");
  await expect(legend).toContainText("squirrel");
  await expect(legend).toContainText("chicken");
  await expect(legend).toContainText("Desired density tint");
  await expect(legend).toContainText("Encounter result");
  expect(Number(await shell.getAttribute("data-wildlife-cells"))).toBeGreaterThan(100);
  expect(Number(await shell.getAttribute("data-wildlife-occupied"))).toBeGreaterThan(10);
  expect(Number(await shell.getAttribute("data-wildlife-animals"))).toBeGreaterThan(25);
  const checksum = await shell.getAttribute("data-wildlife-checksum");
  expect(checksum).toMatch(/^[0-9a-f]{16}$/u);

  await stage.scrollIntoViewIfNeeded();
  const bounds = await stage.boundingBox();
  expect(bounds).not.toBeNull();
  await page.mouse.click(
    bounds!.x + bounds!.width * 0.52,
    bounds!.y + bounds!.height * 0.46,
  );
  await expect(page.getByTestId("wildlife-cell-inspector")).toBeVisible();
  await expect(page.getByTestId("wildlife-population-evidence"))
    .not.toContainText("Tap the wildlife map");
  await page.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-wildlife-ui.png`,
    fullPage: true,
  });
  await canvas.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-wildlife.png`,
  });

  await stage.press("ArrowRight");
  await expect(page).toHaveURL(/x=-?\d+/u);
  await expect(shell).toHaveAttribute("data-wildlife-ready", "true");
  await expect.poll(() => shell.getAttribute("data-wildlife-checksum"))
    .not.toBe(checksum);

  await page.goto(
    "/terrain/?profile=mclone-overworld-v1&seed=-98765&x=0&z=-2048"
      + "&blocks=1024&panes=wildlife&view=map",
    { waitUntil: "networkidle" },
  );
  await expect(shell).toHaveAttribute("data-wildlife-ready", "true");
  await expect(shell).toHaveAttribute("data-wildlife-checksum", checksum ?? "");
  expect(pageErrors).toEqual([]);
});

test("wildlife adapters follow the selected terrain profile", async ({ page }, testInfo) => {
  const checksums = new Set<string>();
  for (const profile of [
    "mclone-overworld-v1",
    "mclone-overworld-v2",
    "mclone-overworld-v3",
  ]) {
    await page.goto(
      `/terrain/?profile=${profile}&seed=-98765&x=0&z=-2048`
        + "&blocks=1024&panes=wildlife&view=map",
      { waitUntil: "networkidle" },
    );
    const stage = page.getByTestId("wildlife-population-stage");
    await expect(stage).toHaveAttribute("data-render-ready", "true");
    await expect(stage).toHaveAttribute("data-profile", profile);
    await expect(stage).toHaveAttribute("data-adapter", profile);
    await expect(stage).toHaveAttribute("data-adapter-status", "available");
    checksums.add((await stage.getAttribute("data-checksum")) ?? "");
    await stage.screenshot({
      path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-wildlife-${profile}.png`,
    });
  }
  expect(checksums.size).toBe(3);

  await page.goto(
    "/terrain/?profile=overworld&seed=-98765&x=0&z=-2048"
      + "&blocks=1024&panes=wildlife&view=map",
    { waitUntil: "networkidle" },
  );
  const stage = page.getByTestId("wildlife-population-stage");
  await expect(stage).toHaveAttribute("data-render-ready", "true");
  await expect(stage).toHaveAttribute(
    "data-adapter-status",
    "requires-published-blocks",
  );
  await expect(stage).toHaveAttribute("data-occupied-cells", "0");
  await expect(page.getByTestId("wildlife-map-legend"))
    .toContainText("Evidence unavailable");
  await stage.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-wildlife-overworld.png`,
  });
});
