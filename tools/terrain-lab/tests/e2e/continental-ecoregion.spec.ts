import { expect, test, type Locator } from "@playwright/test";

const WITNESS_SHA256 =
  "612a102fcc909274f029f1a31a6cdfaa348b53536adcb87893e91308ec1e4405";

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

  await page.goto(page.url(), { waitUntil: "networkidle" });
  await expect(shell).toHaveAttribute("data-ecoregion-ready", "true");
  await expect(shell).toHaveAttribute("data-ecoregion-checksum", checksum ?? "");
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
