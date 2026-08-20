import { expect, test, type Locator } from "@playwright/test";

const WITNESS_SHA256 =
  "4e69ce0b3c1814b8901f848752221fcc28ca0f18b714fb5b7135664de06536f8";

test("continental ecoregion atlas is direct, layered, and inspectable", async ({
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
    "/terrain/?profile=mclone-overworld-v1&seed=12345&x=0&z=0"
      + "&blocks=65536&panes=ecoregion&view=map"
      + "&ecoregionTopology=plane&ecoregionLayer=composed",
    { waitUntil: "networkidle" },
  );

  const shell = page.locator(".appShell");
  const stage = page.getByTestId("continental-ecoregion-stage");
  const canvas = stage.locator("canvas");
  await expect(shell).toHaveAttribute("data-ecoregion-ready", "true");
  await expect(stage).toHaveAttribute("data-render-ready", "true");
  await expect(shell).toHaveAttribute("data-ecoregion-witness", WITNESS_SHA256);
  await expect(shell).toHaveAttribute("data-ecoregion-topology", "plane");
  await expect(shell).toHaveAttribute("data-ecoregion-step", "256");
  await expect(shell).toHaveAttribute(
    "data-ecoregion-atlas-schema",
    "mclone-continental-ecoregion-atlas-v6",
  );
  await expect(shell).toHaveAttribute("data-ecoregion-exact-chunks", "0");
  await expect(shell).toHaveAttribute(
    "data-ecoregion-production-unchanged",
    "true",
  );
  await expect(page.getByTestId("lab-status")).toContainText("ready");
  const sampleCount = Number(await shell.getAttribute("data-ecoregion-samples"));
  const landFraction = Number(
    await shell.getAttribute("data-ecoregion-land-fraction"),
  );
  const oceanFraction = Number(
    await shell.getAttribute("data-ecoregion-ocean-fraction"),
  );
  expect(sampleCount).toBeGreaterThan(40_000);
  expect(sampleCount).toBeLessThanOrEqual(65_536);
  expect(landFraction).toBeGreaterThan(0.05);
  expect(oceanFraction).toBeGreaterThan(0.05);
  const checksum = await shell.getAttribute("data-ecoregion-checksum");
  expect(checksum).toMatch(/^[0-9a-f]{64}$/u);

  const legend = page.getByTestId("continental-ecoregion-legend");
  await expect(legend).toContainText("Composed regional plan");
  await expect(legend).toContainText("old forest core");
  await expect(page.getByTestId("continental-ecoregion-evidence"))
    .toContainText("0 chunks");

  await stage.scrollIntoViewIfNeeded();
  const bounds = await stage.boundingBox();
  expect(bounds).toBeTruthy();
  await page.mouse.click(
    bounds!.x + bounds!.width * 0.54,
    bounds!.y + Math.min(bounds!.height * 0.48, 350),
  );
  await expect(page.getByTestId("continental-ecoregion-inspector"))
    .toBeVisible();
  await page.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-ecoregion-ui.png`,
    fullPage: true,
  });
  await stage.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-ecoregion-65km.png`,
  });

  const composedVisual = await canvasVisualSignature(canvas);
  await page.getByLabel("Continental plan layer").selectOption("province");
  await expect(shell).toHaveAttribute("data-ecoregion-layer", "province");
  await expect(shell).toHaveAttribute("data-ecoregion-checksum", checksum ?? "");
  await expect.poll(() => canvasVisualSignature(canvas)).not.toBe(composedVisual);
  await expect(legend).toContainText("Physiographic provinces");
  await stage.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-ecoregion-provinces.png`,
  });

  const provinceVisual = await canvasVisualSignature(canvas);
  await page.getByLabel("Continental plan layer").selectOption("clearings");
  await expect(shell).toHaveAttribute("data-ecoregion-layer", "clearings");
  await expect(shell).toHaveAttribute("data-ecoregion-checksum", checksum ?? "");
  await expect.poll(() => canvasVisualSignature(canvas)).not.toBe(provinceVisual);
  await expect(legend).toContainText("Planned clearings");
  await stage.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-ecoregion-clearings.png`,
  });

  await page.getByLabel("Continental plan layer").selectOption("production-control");
  await expect(shell).toHaveAttribute("data-ecoregion-layer", "production-control");
  await expect(shell).toHaveAttribute("data-ecoregion-checksum", checksum ?? "");
  await stage.screenshot({
    path:
      `/tmp/mclone-terrain-lab-${testInfo.project.name}-ecoregion-production-control-65km.png`,
  });

  await page.goto(page.url(), { waitUntil: "networkidle" });
  await expect(shell).toHaveAttribute("data-ecoregion-ready", "true");
  await expect(shell).toHaveAttribute("data-ecoregion-checksum", checksum ?? "");
  expect(pageErrors).toEqual([]);
});

test("reviews 131 km against the current production control", async ({
  page,
}, testInfo) => {
  test.skip(
    testInfo.project.name === "phone-chrome",
    "the paired broad-control evidence is a desktop review composition",
  );
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });
  await page.goto(
    "/terrain/?profile=mclone-overworld-v1&seed=12345&x=0&z=0"
      + "&blocks=131072&panes=ecoregion&view=map"
      + "&ecoregionTopology=plane&ecoregionLayer=composed",
    { waitUntil: "networkidle" },
  );

  const shell = page.locator(".appShell");
  const stage = page.getByTestId("continental-ecoregion-stage");
  await expect(shell).toHaveAttribute("data-ecoregion-ready", "true");
  await expect(shell).toHaveAttribute("data-ecoregion-step", "512");
  await expect(shell).toHaveAttribute("data-ecoregion-exact-chunks", "0");
  const samples131 = Number(await shell.getAttribute("data-ecoregion-samples"));
  expect(samples131).toBeGreaterThan(40_000);
  expect(samples131).toBeLessThanOrEqual(65_536);
  const canvas = stage.locator("canvas");
  const candidateVisual = await canvasVisualSignature(canvas);
  const candidateChecksum = await shell.getAttribute("data-ecoregion-checksum");
  const controlChecksum = await shell.getAttribute(
    "data-ecoregion-control-checksum",
  );
  expect(controlChecksum).toMatch(/^[0-9a-f]{64}$/u);
  await stage.screenshot({
    path: "/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-131km.png",
  });

  await page.getByLabel("Continental plan layer").selectOption("transition");
  await expect(shell).toHaveAttribute("data-ecoregion-layer", "transition");
  await stage.screenshot({
    path: "/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-transitions-131km.png",
  });
  const transitionMedian = Number(
    await shell.getAttribute("data-ecoregion-transition-median"),
  );
  expect(transitionMedian).toBeGreaterThanOrEqual(800);
  expect(transitionMedian).toBeLessThanOrEqual(2_600);

  await page.getByLabel("Continental plan layer").selectOption("habitat");
  await expect(shell).toHaveAttribute("data-ecoregion-layer", "habitat");
  await expect(page.getByTestId("continental-ecoregion-legend"))
    .toContainText("riparian spine");
  await expect(page.getByTestId("continental-ecoregion-legend"))
    .toContainText("open range link");
  await stage.screenshot({
    path: "/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-habitat-routes-131km.png",
  });

  await page.getByLabel("Continental plan layer").selectOption(
    "production-control",
  );
  await expect(shell).toHaveAttribute(
    "data-ecoregion-control-revision",
    "mclone-overworld-v1-fields-21",
  );
  await expect(shell).toHaveAttribute("data-ecoregion-layer", "production-control");
  await expect(shell).toHaveAttribute("data-ecoregion-checksum", candidateChecksum ?? "");
  await expect(shell).toHaveAttribute(
    "data-ecoregion-control-checksum",
    controlChecksum ?? "",
  );
  await expect(shell).toHaveAttribute("data-ecoregion-control-exact-chunks", "0");
  const candidateComponents = Number(
    await shell.getAttribute("data-ecoregion-components"),
  );
  const controlComponents = Number(
    await shell.getAttribute("data-ecoregion-control-biome-components"),
  );
  expect(candidateComponents).toBeGreaterThan(10);
  expect(controlComponents).toBeGreaterThan(candidateComponents * 3);
  expect(Number(await shell.getAttribute("data-ecoregion-clearing-count")))
    .toBeGreaterThan(10);
  expect(Number(await shell.getAttribute("data-ecoregion-clearing-isolation-p90")))
    .toBeGreaterThan(1_000);
  expect(Number(await shell.getAttribute("data-ecoregion-recurrence-median")))
    .toBeGreaterThan(1_000);
  expect(Number(await shell.getAttribute("data-ecoregion-habitat-patches")))
    .toBeGreaterThan(10);
  const connectedHabitatFraction = Number(
    await shell.getAttribute("data-ecoregion-habitat-connected-fraction"),
  );
  expect(connectedHabitatFraction).toBeGreaterThan(0);
  expect(connectedHabitatFraction).toBeLessThanOrEqual(1);
  await expect.poll(() => canvasVisualSignature(canvas)).not.toBe(candidateVisual);
  await expect(page.getByTestId("continental-ecoregion-legend"))
    .toContainText("Current production · field 21");
  await expect(page.getByTestId("continental-ecoregion-evidence"))
    .toContainText("mclone-overworld-v1-fields-21");
  await expect(page.getByTestId("continental-ecoregion-evidence"))
    .toContainText("Regional recurrence median / p90");
  await page.getByTestId("continental-ecoregion-evidence").screenshot({
    path:
      "/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-metrics-131km.png",
  });
  await stage.screenshot({
    path:
      "/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-control-131km.png",
  });
  await page.screenshot({
    path:
      "/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-control-131km-ui.png",
    fullPage: true,
  });
  let previousControlVisual = await canvasVisualSignature(canvas);
  for (const [layer, label, filename] of [
    ["production-land", "Current production · land & ocean", "land"],
    ["production-climate", "Current production · climate", "climate"],
    ["production-biome", "Current production · biome recipe", "biome"],
    ["production-openness", "Current production · forest openness", "openness"],
    ["production-height", "Current production · surface height", "height"],
    ["production-water", "Current production · water", "water"],
  ] as const) {
    await page.getByLabel("Continental plan layer").selectOption(layer);
    await expect(shell).toHaveAttribute("data-ecoregion-layer", layer);
    await expect(shell).toHaveAttribute(
      "data-ecoregion-control-checksum",
      controlChecksum ?? "",
    );
    await expect.poll(() => canvasVisualSignature(canvas)).not.toBe(previousControlVisual);
    await expect(page.getByTestId("continental-ecoregion-legend"))
      .toContainText(label);
    await stage.screenshot({
      path:
        `/tmp/mclone-terrain-lab-desktop-chrome-ecoregion-production-${filename}-131km.png`,
    });
    previousControlVisual = await canvasVisualSignature(canvas);
  }
  const controlOpenComponents = Number(
    await shell.getAttribute("data-ecoregion-control-open-components"),
  );
  expect(controlOpenComponents).toBeGreaterThan(10);
  expect(Number(await shell.getAttribute("data-ecoregion-control-field-samples")))
    .toBe(samples131 * 5);
  expect(pageErrors).toEqual([]);
});

test("moves the retained atlas while a pan rebuild is coalesced", async ({
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
    "/terrain/?profile=mclone-overworld-v1&seed=12345&x=0&z=0"
      + "&blocks=131072&panes=ecoregion&view=map"
      + "&ecoregionTopology=plane&ecoregionLayer=composed",
    { waitUntil: "networkidle" },
  );

  const shell = page.locator(".appShell");
  const stage = page.getByTestId("continental-ecoregion-stage");
  const canvas = stage.locator("canvas");
  await expect(shell).toHaveAttribute("data-ecoregion-ready", "true");
  const initialChecksum = await shell.getAttribute("data-ecoregion-checksum");
  const initialVisual = await canvasVisualSignature(canvas);
  const bounds = await stage.boundingBox();
  expect(bounds).toBeTruthy();
  const pointerX = bounds!.x + bounds!.width * 0.45;
  const pointerY = bounds!.y + Math.min(bounds!.height * 0.45, 320);
  await page.mouse.move(pointerX, pointerY);
  await page.mouse.down();
  for (let step = 1; step <= 8; step += 1) {
    await page.mouse.move(pointerX + step * 8, pointerY + step * 3);
    await page.waitForTimeout(25);
  }
  await expect(stage).toHaveAttribute("data-render-ready", "true");
  await expect(stage).toHaveAttribute("data-render-updating", "true");
  await expect(stage).toHaveAttribute("data-retained-frame-shifted", "true");
  await expect(shell).toHaveAttribute(
    "data-ecoregion-checksum",
    initialChecksum ?? "",
  );
  expect(await canvasVisualSignature(canvas)).not.toBe(initialVisual);
  await stage.screenshot({
    path:
      `/tmp/mclone-terrain-lab-${testInfo.project.name}-ecoregion-retained-pan.png`,
  });

  await page.mouse.up();
  await expect(stage).toHaveAttribute("data-render-updating", "false");
  await expect(stage).toHaveAttribute("data-retained-frame-shifted", "false");
  await expect.poll(() => shell.getAttribute("data-ecoregion-checksum"))
    .not.toBe(initialChecksum);
  expect(pageErrors).toEqual([]);
});

async function canvasVisualSignature(canvas: Locator): Promise<number> {
  return canvas.evaluate((element) => {
    const surface = element as HTMLCanvasElement;
    const context = surface.getContext("2d");
    if (!context) {
      return 0;
    }
    const pixels = context.getImageData(
      0,
      0,
      surface.width,
      surface.height,
    ).data;
    const stride = Math.max(4, Math.floor(pixels.length / 16_384 / 4) * 4);
    let hash = 2_166_136_261;
    for (let index = 0; index < pixels.length; index += stride) {
      hash ^= pixels[index]!;
      hash = Math.imul(hash, 16_777_619);
      hash ^= pixels[index + 1]!;
      hash = Math.imul(hash, 16_777_619);
      hash ^= pixels[index + 2]!;
      hash = Math.imul(hash, 16_777_619);
    }
    return hash >>> 0;
  });
}
