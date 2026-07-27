import { expect, test, type Locator } from "@playwright/test";

const SUITE_SHA256 =
  "14250ea1a92a72246abfd256d3ffb2caca021a5805a4be692299bfecc5017f8e";

test("semantic terrain is pannable, exact, and responsive", async ({
  page,
}, testInfo) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });
  const url = new URL("/terrain/", testInfo.project.use.baseURL as string);
  url.searchParams.set("profile", "mclone-overworld-v1");
  url.searchParams.set("panes", "semantic");
  url.searchParams.set("seed", "12345");
  url.searchParams.set("x", "1024");
  url.searchParams.set("z", "-768");
  url.searchParams.set("blocks", "6144");
  url.searchParams.set("view", "3d");
  url.searchParams.set("semanticSubstrate", "quiet");
  url.searchParams.set("semanticFeatures", "combined");
  url.searchParams.set("semanticTopology", "torus");
  url.searchParams.set("semanticCorrection", "local");
  url.searchParams.set("semanticGuides", "1");
  await page.goto(url.href, { waitUntil: "networkidle" });

  const shell = page.locator(".appShell");
  const stage = page.locator("[data-testid='semantic-terrain-stage']");
  await expect(shell).toHaveAttribute("data-semantic-ready", "true");
  await expect(shell).toHaveAttribute("data-semantic-suite", SUITE_SHA256);
  await expect(shell).toHaveAttribute(
    "data-semantic-production-unchanged",
    "true",
  );
  await expect(stage).toHaveAttribute("data-render-ready", "true");
  await expect(stage).toHaveAttribute("data-vertical-datum", "64");
  await expect(stage).toHaveAttribute("data-vertical-span", "192");
  const initialChecksum = await shell.getAttribute("data-semantic-checksum");
  expect(initialChecksum).toMatch(/^[0-9a-f]{64}$/u);

  const bounds = await stage.boundingBox();
  expect(bounds).toBeTruthy();
  if (testInfo.project.name === "phone-chrome") {
    expect(bounds!.height).toBeGreaterThan(bounds!.width * 2.5);
  } else {
    expect(bounds!.height).toBeGreaterThan(700);
  }

  const canvas = stage.locator("canvas");
  const initialVisual = await canvasVisualSignature(canvas);
  const pointerX = bounds!.x + bounds!.width * 0.35;
  const pointerY = bounds!.y + Math.min(bounds!.height * 0.1, 180);
  await page.mouse.move(pointerX, pointerY);
  await page.mouse.down();
  await page.mouse.move(pointerX + 42, pointerY + 18);
  await expect(stage).toHaveAttribute("data-render-ready", "true");
  await expect.poll(() => canvasVisualSignature(canvas)).not.toBe(initialVisual);
  await page.mouse.up();
  const orbitVisual = await canvasVisualSignature(canvas);

  await page.keyboard.down("Shift");
  await page.mouse.move(pointerX, pointerY);
  await page.mouse.down();
  let sawReconstructedFrame = false;
  let sawPannedVisual = false;
  for (let step = 1; step <= 12; step += 1) {
    await page.mouse.move(pointerX + step * 5, pointerY + step * 2);
    await page.waitForTimeout(25);
    expect(await stage.getAttribute("data-render-ready")).toBe("true");
    sawReconstructedFrame ||=
      await shell.getAttribute("data-semantic-checksum") !== initialChecksum;
    sawPannedVisual ||= await canvasVisualSignature(canvas) !== orbitVisual;
  }
  await page.mouse.up();
  await page.keyboard.up("Shift");
  expect(sawReconstructedFrame).toBe(true);
  expect(sawPannedVisual).toBe(true);
  await expect(stage).toHaveAttribute("data-render-updating", "false");

  const beforeZoomBlocks = new URL(page.url()).searchParams.get("blocks");
  const beforeZoomVisual = await canvasVisualSignature(canvas);
  await page.mouse.move(pointerX, pointerY);
  await page.mouse.wheel(0, -260);
  expect(await stage.getAttribute("data-render-ready")).toBe("true");
  await expect.poll(() =>
    new URL(page.url()).searchParams.get("blocks")
  ).not.toBe(beforeZoomBlocks);
  await expect(stage).toHaveAttribute("data-render-updating", "false");
  await expect.poll(() => canvasVisualSignature(canvas)).not.toBe(beforeZoomVisual);
  await expect(stage).toHaveAttribute("data-vertical-datum", "64");
  await expect(stage).toHaveAttribute("data-vertical-span", "192");

  await page.getByRole("button", { name: "Map", exact: true }).click();
  await expect(page).toHaveURL(/view=map/u);

  const lifted = new URL(page.url());
  lifted.searchParams.set("x", String(1024 + 6144));
  lifted.searchParams.set("z", String(-768 - 6144));
  lifted.searchParams.set("blocks", "6144");
  await page.goto(lifted.href, { waitUntil: "networkidle" });
  await expect(shell).toHaveAttribute("data-semantic-ready", "true");
  await expect(shell).toHaveAttribute(
    "data-semantic-checksum",
    initialChecksum!,
  );
  expect(pageErrors).toEqual([]);
});

async function canvasVisualSignature(
  canvas: Locator,
): Promise<number> {
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
