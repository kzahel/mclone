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
    "/terrain/?seed=-98765&x=-304&z=336&blocks=512&detail=auto"
      + "&panes=canonical%2Ccpu%2Cgpu&canonical=final&radius=1"
      + "&water=1&vegetation=1&stage=hydrology&view=3d&layer=terrain",
  );
  await waitForCurrentComparison(page);
  await waitForCanonical(page, 9);
  await expect(page.locator("[data-testid='lab-status']")).toContainText("ready");

  const sourceControls = page.getByTestId("preview-source-controls");
  await expect(sourceControls).toBeVisible();
  await expect(sourceControls).toContainText("Same coordinates, independent readiness");
  const diagnostics = page.locator("[data-testid='terrain-diagnostics']");
  await expect(diagnostics).not.toContainText("Base mean Δpending");
  const shell = page.locator(".appShell");
  await expect(shell).toHaveAttribute("data-panes", "canonical,cpu,gpu");
  await expect(shell).toHaveAttribute("data-canonical-published", "9");
  await expect(shell).toHaveAttribute("data-canonical-requested", "9");
  await expect(shell).toHaveAttribute("data-canonical-complete", "true");
  await expect(shell).toHaveAttribute(
    "data-compare-layout",
    "stacked",
  );
  expect(Number(await shell.getAttribute("data-vertex-count"))).toBeGreaterThan(49_152);
  await expect(shell).toHaveAttribute("data-target-ready", "true");
  expect(Number(await shell.getAttribute("data-resident-tiles"))).toBeGreaterThan(1);
  expect(Number(await shell.getAttribute("data-effective-spacing"))).toBeGreaterThanOrEqual(1);
  expect(Number(await shell.getAttribute("data-base-mean-error"))).toBeLessThanOrEqual(0.01);
  expect(Number(await shell.getAttribute("data-base-p95-error"))).toBeLessThanOrEqual(0.01);
  expect(Number(await shell.getAttribute("data-ocean-agreement"))).toBeGreaterThanOrEqual(0.999);
  expect(Number(await shell.getAttribute("data-channel-agreement"))).toBeGreaterThanOrEqual(0.999);
  expect(Number(await shell.getAttribute("data-continentalness-error"))).toBeLessThanOrEqual(0.001);
  const canvas = page.locator("canvas[aria-label='Live GPU terrain preview']");
  const canonicalCanvas = page.locator(
    "canvas[aria-label='Canonical textured terrain preview']",
  );
  await expect(canvas).toBeVisible();
  await expect(canonicalCanvas).toBeVisible();
  await page.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-workspace.png`,
    fullPage: true,
  });
  await canonicalCanvas.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-canonical.png`,
  });
  await canvas.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-initial.png`,
  });
  const compareStageBox = await page.getByTestId("terrain-stage").boundingBox();
  expect(compareStageBox).not.toBeNull();

  await page.getByRole("button", { name: "Water shown", exact: true }).click();
  await expect(page).toHaveURL(/water=0/u);
  await expect(shell).toHaveAttribute("data-canonical-published", "9");
  await expect(shell).toHaveAttribute("data-canonical-complete", "true");
  await page.getByRole("button", { name: "Water hidden", exact: true }).click();
  await expect(page).toHaveURL(/water=1/u);

  await page.getByRole("button", { name: "GPU LOD", exact: true }).click();
  await waitForLane(page, "cpu");
  await expect(shell).toHaveAttribute("data-panes", "canonical,cpu");
  await expect(shell).toHaveAttribute("data-compare-layout", "single");
  const singleStageBox = await page.getByTestId("terrain-stage").boundingBox();
  expect(singleStageBox).not.toBeNull();
  if (testInfo.project.name === "phone-chrome") {
    expect(compareStageBox!.height).toBeGreaterThanOrEqual(singleStageBox!.height * 1.9);
  }
  await canvas.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-cpu-final.png`,
  });

  await page.getByRole("button", { name: "GPU LOD", exact: true }).click();
  await waitForCurrentComparison(page);
  await expect(shell).toHaveAttribute("data-panes", "canonical,cpu,gpu");
  await expect(shell).toHaveAttribute(
    "data-compare-layout",
    "stacked",
  );

  const initialRevision = Number(await shell.getAttribute("data-render-revision"));
  const initialYaw = Number(await shell.getAttribute("data-camera-yaw"));
  const initialPitch = Number(await shell.getAttribute("data-camera-pitch"));
  const initialUrl = page.url();
  const stage = page.getByTestId("terrain-stage");
  const layoutStageBox = await stage.boundingBox();
  const sourceControlsBox = await sourceControls.boundingBox();
  const viewportControlsBox = await page.getByTestId("viewport-controls").boundingBox();
  expect(layoutStageBox).not.toBeNull();
  expect(sourceControlsBox).not.toBeNull();
  expect(viewportControlsBox).not.toBeNull();
  expect(sourceControlsBox!.y + sourceControlsBox!.height)
    .toBeLessThanOrEqual(viewportControlsBox!.y + 1);
  expect(viewportControlsBox!.y + viewportControlsBox!.height)
    .toBeLessThanOrEqual(layoutStageBox!.y + 1);
  await stage.scrollIntoViewIfNeeded();
  const stageBox = await stage.boundingBox();
  expect(stageBox).not.toBeNull();
  const viewportHeight = page.viewportSize()?.height ?? 900;
  const orbitStartY = Math.max(
    100,
    Math.min(stageBox!.y + 200, viewportHeight - 100),
  );
  await page.mouse.move(stageBox!.x + stageBox!.width * 0.5, orbitStartY);
  await page.mouse.down();
  await page.mouse.move(
    stageBox!.x + stageBox!.width * 0.65,
    orbitStartY - 80,
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
  await expect(page).toHaveURL(/blocks=1024/u);
  await expect(page).toHaveURL(/detail=auto/u);
  await expect.poll(
    async () => Number(await shell.getAttribute("data-render-revision")),
  ).toBeGreaterThan(errorRevision);
  await waitForCurrentComparison(page);

  await stage.scrollIntoViewIfNeeded();
  await page.evaluate(() => window.scrollBy(0, 80));
  const mapStageBox = await stage.boundingBox();
  expect(mapStageBox).not.toBeNull();
  const beforeWheelScroll = await page.evaluate(() => window.scrollY);
  const beforeWheelBlocks = new URL(page.url()).searchParams.get("blocks");
  const pointerX = mapStageBox!.x + mapStageBox!.width * 0.5;
  const pointerY = Math.max(
    1,
    Math.min(
      mapStageBox!.y + Math.min(mapStageBox!.height * 0.1, 100),
      testInfo.project.name === "phone-chrome" ? 780 : 940,
    ),
  );
  await page.mouse.move(pointerX, pointerY);
  await page.mouse.click(pointerX, pointerY);
  await expect(shell).not.toHaveAttribute("data-inspected-x", "");
  await expect(page.getByTestId("point-receipt")).not.toContainText("Tap terrain");
  await expect(page.locator(".inspectionMarker")).toBeVisible();
  await page.mouse.wheel(0, 120);
  await expect.poll(() => new URL(page.url()).searchParams.get("blocks"))
    .not.toBe(beforeWheelBlocks);
  expect(await page.evaluate(() => window.scrollY)).toBe(beforeWheelScroll);

  const beforeGrabZ = Number(new URL(page.url()).searchParams.get("z"));
  await page.mouse.down();
  await page.mouse.move(pointerX, pointerY + 80);
  await page.mouse.up();
  await expect.poll(() => Number(new URL(page.url()).searchParams.get("z")))
    .toBeLessThan(beforeGrabZ);
  await waitForCurrentComparison(page);

  await page.getByLabel("Terrain resolution").selectOption("1");
  await expect(page).toHaveURL(/detail=1/u);
  await waitForCurrentComparison(page);
  await expect(shell).toHaveAttribute("data-requested-spacing", "1");
  expect(Number(await shell.getAttribute("data-effective-spacing"))).toBeGreaterThan(1);
  await page.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-map-error.png`,
    fullPage: true,
  });

  await page.evaluate(() => {
    const race: string[] = [];
    const shell = document.querySelector(".appShell");
    const record = (): void => {
      const state = `${shell?.getAttribute("data-cpu-target-ready")}/${
        shell?.getAttribute("data-gpu-target-ready")
      }`;
      if (race.at(-1) !== state) {
        race.push(state);
      }
    };
    record();
    const observer = new MutationObserver(record);
    if (shell) {
      observer.observe(shell, {
        attributes: true,
        attributeFilter: ["data-cpu-target-ready", "data-gpu-target-ready"],
      });
    }
    Object.assign(window, { terrainLabRaceStates: race, terrainLabRaceObserver: observer });
  });
  const beforeStressRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByRole("button", { name: "Run stress race" }).click();
  await expect(shell).toHaveAttribute("data-cache-enabled", "false");
  await expect(shell).toHaveAttribute("data-panes", "cpu,gpu");
  await expect(page).toHaveURL(/blocks=2048/u);
  await expect(page).toHaveURL(/detail=2/u);
  await expect(page).toHaveURL(/source=split/u);
  await expect(page).toHaveURL(/view=map/u);
  await waitForCurrentComparison(page, beforeStressRevision);
  await expect(shell).toHaveAttribute("data-cpu-target-ready", "true");
  await expect(shell).toHaveAttribute("data-gpu-target-ready", "true");
  await expect(page.getByTestId("lab-status")).toContainText("ready");
  await expect(shell).toHaveAttribute("data-cache-hits", "0");
  const raceStates = await page.evaluate(() => {
    const holder = window as typeof window & {
      terrainLabRaceObserver?: MutationObserver;
      terrainLabRaceStates?: string[];
    };
    holder.terrainLabRaceObserver?.disconnect();
    return holder.terrainLabRaceStates ?? [];
  });
  expect(raceStates.some((entry) => entry === "true/false" || entry === "false/true")).toBe(true);
  await settlePaint(page);
  await canvas.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-stress-race.png`,
  });

  expect(pageErrors).toEqual([]);
});

test("reconstructs one planned stream for both LOD lanes", async ({
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
    "/terrain/?seed=-98765&x=2369&z=-1977&blocks=128&detail=1"
      + "&panes=cpu%2Cgpu&canonical=final&radius=1&water=1&vegetation=1"
      + "&stage=structured&view=map&layer=streams",
  );
  await waitForCurrentComparison(page);
  const shell = page.locator(".appShell");
  await expect(shell).toHaveAttribute("data-channel-agreement", "1");
  await expect(page.getByTestId("terrain-diagnostics")).toContainText(
    "planned streams available",
  );
  const stage = page.getByTestId("terrain-stage");
  const bounds = await stage.boundingBox();
  expect(bounds).not.toBeNull();
  const stacked = bounds!.width <= bounds!.height;
  const pointerX = bounds!.x + bounds!.width * (stacked ? 0.5 : 0.25);
  const pointerY = bounds!.y + bounds!.height * (stacked ? 0.25 : 0.5);
  await page.mouse.click(pointerX, pointerY);
  await expect(shell).toHaveAttribute("data-inspected-x", "2369");
  await expect(shell).toHaveAttribute("data-inspected-z", "-1977");
  await expect(page.getByTestId("point-receipt")).toContainText("planned stream");
  await expect(page.getByTestId("point-receipt")).toContainText("147, -126");
  await stage.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-structured-stream.png`,
  });
  expect(pageErrors).toEqual([]);
});

async function waitForCanonical(
  page: import("@playwright/test").Page,
  requestedChunks: number,
): Promise<void> {
  await page.waitForFunction((requested) => {
    const shell = document.querySelector(".appShell");
    return shell?.getAttribute("data-canonical-complete") === "true"
      && Number(shell.getAttribute("data-canonical-published")) === requested
      && Number(shell.getAttribute("data-canonical-requested")) === requested;
  }, requestedChunks);
}

async function waitForLane(
  page: import("@playwright/test").Page,
  lane: "cpu" | "gpu",
): Promise<void> {
  await page.waitForFunction((requestedLane) => {
    const shell = document.querySelector(".appShell");
    const render = Number(shell?.getAttribute("data-render-revision") ?? "0");
    return render > 0
      && shell?.getAttribute(`data-${requestedLane}-target-ready`) === "true";
  }, lane);
}

async function waitForCurrentComparison(
  page: import("@playwright/test").Page,
  afterRevision = 0,
): Promise<void> {
  await page.waitForFunction((previousRevision) => {
    const shell = document.querySelector(".appShell");
    const render = Number(shell?.getAttribute("data-render-revision") ?? "0");
    const comparison = Number(shell?.getAttribute("data-comparison-revision") ?? "0");
    return render > previousRevision && comparison === render;
  }, afterRevision);
}

async function settlePaint(page: import("@playwright/test").Page): Promise<void> {
  await page.evaluate(() => new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  }));
}
