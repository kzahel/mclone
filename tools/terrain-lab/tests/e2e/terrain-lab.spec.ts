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
    testInfo.project.name === "phone-chrome" ? "stacked" : "side-by-side",
  );
  await expect(shell).toHaveAttribute("data-projection", "orthographic");
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
  await page.getByRole("button", { name: "Perspective", exact: true }).click();
  await expect(page).toHaveURL(/projection=perspective/u);
  await expect(shell).toHaveAttribute("data-projection", "perspective");
  await page.getByRole("button", { name: "Orthographic", exact: true }).click();
  await expect(page).toHaveURL(/projection=orthographic/u);
  await expect(shell).toHaveAttribute("data-projection", "orthographic");
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
    testInfo.project.name === "phone-chrome" ? "stacked" : "side-by-side",
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
  await expect.poll(
    async () => Number(await shell.getAttribute("data-landform-agreement")),
  ).toBeGreaterThan(0.9999);
  await expect(page.getByTestId("terrain-diagnostics")).toContainText(
    "planned streams available",
  );
  const stage = page.getByTestId("terrain-stage");
  const bounds = await stage.boundingBox();
  expect(bounds).not.toBeNull();
  const stacked = testInfo.project.name === "phone-chrome";
  const pointerX = bounds!.x + bounds!.width * (stacked ? 0.5 : 0.25);
  const pointerY = bounds!.y + bounds!.height * (stacked ? 0.25 : 0.5);
  await page.mouse.click(pointerX, pointerY);
  await expect(shell).toHaveAttribute("data-inspected-x", "2369");
  await expect(shell).toHaveAttribute("data-inspected-z", "-1977");
  await expect(page.getByTestId("point-receipt")).toContainText("planned stream");
  await expect(page.getByTestId("point-receipt")).toContainText("147, -126");
  await page.getByText("Production fields and revisions").click();
  await expect(page.getByTestId("point-receipt")).toContainText("gpu-preview-a5");
  await expect(page.getByTestId("point-receipt")).toContainText("Carve delta");
  await stage.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-structured-stream.png`,
  });
  const streamRevision = Number(await shell.getAttribute("data-render-revision"));
  await page.getByLabel("Diagnostic layer").selectOption("landforms");
  await expect(page).toHaveURL(/layer=landforms/u);
  await expect.poll(
    async () => Number(await shell.getAttribute("data-render-revision")),
  ).toBeGreaterThan(streamRevision);
  await stage.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-landforms.png`,
  });
  expect(pageErrors).toEqual([]);
});

test("zooms shared and canonical terrain to one-block texture detail", async ({
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
    "/terrain/?seed=-98765&x=-304&z=336&blocks=4&detail=auto"
      + "&panes=canonical%2Ccpu%2Cgpu&canonical=final&radius=0"
      + "&water=1&vegetation=1&stage=hydrology&view=map&layer=terrain",
  );
  await waitForCurrentComparison(page);
  await waitForCanonical(page, 1);
  const shell = page.locator(".appShell");

  await page.getByRole("button", { name: "Zoom in" }).click();
  await expect(page).toHaveURL(/blocks=2/u);
  await page.getByRole("button", { name: "Zoom in" }).click();
  await expect(page).toHaveURL(/blocks=1/u);
  await waitForCurrentComparison(page);
  await waitForCanonical(page, 1);
  await expect(shell).toHaveAttribute("data-effective-spacing", "1");
  await expect(shell).toHaveAttribute("data-cpu-target-ready", "true");
  await expect(shell).toHaveAttribute("data-gpu-target-ready", "true");
  expect(Number(await shell.getAttribute("data-visible-tiles"))).toBeLessThanOrEqual(4);

  await page.getByRole("button", { name: "Zoom in" }).click();
  await expect(page).toHaveURL(/blocks=1/u);
  await page.getByTestId("pane-workspace").screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-block-workspace.png`,
  });
  await page.getByRole("button", { name: "CPU LOD", exact: true }).click();
  await page.getByRole("button", { name: "GPU LOD", exact: true }).click();
  await expect(shell).toHaveAttribute("data-panes", "canonical");
  const canonicalStage = page.getByTestId("canonical-terrain-stage");
  await canonicalStage.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-canonical-block-map.png`,
  });

  await page.getByRole("button", { name: "3D terrain", exact: true }).click();
  await expect(page).toHaveURL(/view=3d/u);
  await settlePaint(page);
  await canonicalStage.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-canonical-block-3d.png`,
  });
  expect(pageErrors).toEqual([]);
});

test("switches the whole lab to worker-backed vanilla terrain", async ({
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
    "/terrain/?profile=overworld&seed=12345&x=0&z=0&blocks=512&detail=8"
      + "&panes=canonical%2Ccpu%2Cgpu&canonical=surface&radius=0"
      + "&water=1&vegetation=1&stage=hydrology&view=3d&layer=continentalness",
  );
  await waitForLane(page, "cpu");
  await waitForCanonical(page, 1);
  const shell = page.locator(".appShell");
  await expect(shell).toHaveAttribute("data-profile", "overworld");
  await expect(shell).toHaveAttribute("data-panes", "canonical,cpu");
  await expect(shell).toHaveAttribute("data-stage", "surface");
  await expect(shell).toHaveAttribute("data-request-gpu-tiles", "0");
  await expect(shell).toHaveAttribute("data-cpu-target-ready", "true");
  await expect(shell).toHaveAttribute("data-gpu-target-ready", "false");
  await expect(page.getByRole("button", { name: "GPU LOD" })).toHaveCount(0);
  await expect(page.getByLabel("Diagnostic layer")).toHaveValue("terrain");
  await expect(page.getByTestId("point-receipt")).toContainText(
    "Vanilla point receipts are not in the first pass",
  );
  const vanillaCanvas = page.locator(
    "canvas[aria-label='Worker-backed vanilla terrain preview']",
  );
  await expect(vanillaCanvas).toBeVisible();
  await page.getByTestId("pane-workspace").screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-vanilla-workspace.png`,
  });
  await page.getByLabel("Terrain profile").selectOption("mclone-overworld-v1");
  await expect(shell).toHaveAttribute("data-profile", "mclone-overworld-v1");
  await expect(page.getByRole("button", { name: "GPU LOD" })).toHaveCount(1);
  await waitForLane(page, "cpu");
  await settlePaint(page);
  expect(pageErrors).toEqual([]);
});

test("two-finger gestures pan and zoom procedural and real terrain", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "phone-chrome", "touch-only interaction contract");
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });
  await page.goto(
    "/terrain/?seed=-98765&x=-304&z=336&blocks=512&detail=auto"
      + "&panes=canonical%2Cgpu&canonical=surface&radius=0"
      + "&water=1&vegetation=1&stage=hydrology&view=map&layer=terrain",
  );
  await waitForLane(page, "gpu");
  await waitForCanonical(page, 1);
  const shell = page.locator(".appShell");
  const proceduralStage = page.getByTestId("terrain-stage");
  const canonicalStage = page.getByTestId("canonical-terrain-stage");

  await assertTouchPanZoom(page, proceduralStage);
  await expect(shell).toHaveAttribute("data-inspected-x", "");
  await waitForLane(page, "gpu");

  await assertTouchPanZoom(page, canonicalStage);
  await expect(shell).toHaveAttribute("data-inspected-x", "");
  await waitForCanonical(page, 1);

  await page.getByRole("button", { name: "3D terrain", exact: true }).click();
  await expect(page).toHaveURL(/view=3d/u);
  const initialYaw = Number(await shell.getAttribute("data-camera-yaw"));
  const initialPitch = Number(await shell.getAttribute("data-camera-pitch"));

  await assertTouchPanZoom(page, proceduralStage);
  await expect.poll(
    async () => Number(await shell.getAttribute("data-camera-yaw")),
  ).toBe(initialYaw);
  await expect.poll(
    async () => Number(await shell.getAttribute("data-camera-pitch")),
  ).toBe(initialPitch);

  await assertTouchPanZoom(page, canonicalStage);
  await expect.poll(
    async () => Number(await shell.getAttribute("data-camera-yaw")),
  ).toBe(initialYaw);
  await expect.poll(
    async () => Number(await shell.getAttribute("data-camera-pitch")),
  ).toBe(initialPitch);
  expect(pageErrors).toEqual([]);
});

test("publishes an 81-chunk real-terrain footprint progressively", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "phone-chrome", "phone-sized exact-footprint proof");
  test.slow();
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });
  await page.goto(
    "/terrain/?seed=-98765&x=-304&z=336&blocks=512&detail=auto"
      + "&panes=canonical&canonical=final&radius=0"
      + "&water=1&vegetation=1&stage=hydrology&view=3d&layer=terrain",
  );
  await waitForCanonical(page, 1);
  const shell = page.locator(".appShell");
  await page.evaluate(() => {
    const counts: number[] = [];
    const element = document.querySelector(".appShell");
    const record = (): void => {
      const count = Number(element?.getAttribute("data-canonical-published") ?? "0");
      if (counts.at(-1) !== count) {
        counts.push(count);
      }
    };
    record();
    const observer = new MutationObserver(record);
    if (element) {
      observer.observe(element, {
        attributes: true,
        attributeFilter: ["data-canonical-published"],
      });
    }
    Object.assign(window, {
      terrainLabCanonicalCounts: counts,
      terrainLabCanonicalObserver: observer,
    });
  });

  const footprint = page.getByLabel("Exact chunk footprint");
  await expect(footprint.locator("option")).toHaveCount(9);
  await footprint.selectOption("4");
  await expect(page).toHaveURL(/radius=4/u);
  await expect(shell).toHaveAttribute("data-canonical-requested", "81");
  await waitForCanonical(page, 81);
  const counts = await page.evaluate(() => {
    const holder = window as typeof window & {
      terrainLabCanonicalCounts?: number[];
      terrainLabCanonicalObserver?: MutationObserver;
    };
    holder.terrainLabCanonicalObserver?.disconnect();
    return holder.terrainLabCanonicalCounts ?? [];
  });
  expect(counts.some((count) => count > 0 && count < 81)).toBe(true);
  await expect(footprint).toHaveValue("4");
  await page.getByTestId("canonical-terrain-stage").screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-canonical-81.png`,
  });
  expect(pageErrors).toEqual([]);
});

test("keeps a 9x9 real footprint resident and paces pan admission", async ({
  page,
}, testInfo) => {
  test.slow();
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });
  await page.goto(
    "/terrain/?seed=-98765&x=-304&z=336&blocks=512&detail=auto"
      + "&panes=canonical&canonical=final&radius=4"
      + "&water=1&vegetation=1&stage=hydrology&view=map&layer=terrain",
  );
  await waitForCanonical(page, 81);
  const shell = page.locator(".appShell");
  const centerX = page.getByLabel("Center X");
  const initialEpoch = await shell.getAttribute("data-canonical-epoch");
  expect(Number(await shell.getAttribute("data-canonical-worker-mesh-ms")))
    .toBeGreaterThan(0);
  expect(Number(await shell.getAttribute("data-canonical-main-decode-ms")))
    .toBeGreaterThan(0);
  expect(Number(await shell.getAttribute("data-canonical-cache-raw-bytes")))
    .toBeGreaterThan(0);
  expect(Number(await shell.getAttribute("data-canonical-resident-raw-bytes")))
    .toBe(0);

  await centerX.fill("-303");
  await centerX.press("Enter");
  await expect(page).toHaveURL(/x=-303/u);
  await expect(shell).toHaveAttribute("data-canonical-epoch", initialEpoch!);
  await expect(shell).toHaveAttribute("data-canonical-published", "81");

  await centerX.fill("-288");
  await centerX.press("Enter");
  await expect(page).toHaveURL(/x=-288/u);
  await expect(shell).toHaveAttribute("data-canonical-resident-hits", "72");
  await waitForCanonical(page, 81);
  await expect(shell).toHaveAttribute("data-canonical-admission-frames", "9");
  await expect(shell).toHaveAttribute("data-canonical-max-frame-admissions", "1");
  expect(Number(await shell.getAttribute("data-canonical-worker-mesh-ms")))
    .toBeGreaterThan(0);
  expect(Number(await shell.getAttribute("data-canonical-main-decode-ms")))
    .toBeGreaterThan(0);
  expect(Number(await shell.getAttribute("data-canonical-max-admission-ms")))
    .toBeGreaterThan(0);
  const meshTargets = Number(
    await shell.getAttribute("data-canonical-mesh-target-chunks"),
  );
  expect(meshTargets).toBeGreaterThan(9);
  expect(meshTargets).toBeLessThan(45);

  await page.evaluate(() => {
    const counts: number[] = [];
    const element = document.querySelector(".appShell");
    const record = (): void => {
      const count = Number(element?.getAttribute("data-canonical-published") ?? "0");
      if (counts.at(-1) !== count) {
        counts.push(count);
      }
    };
    record();
    const observer = new MutationObserver(record);
    if (element) {
      observer.observe(element, {
        attributes: true,
        attributeFilter: ["data-canonical-published"],
      });
    }
    Object.assign(window, {
      terrainLabCanonicalPanCounts: counts,
      terrainLabCanonicalPanObserver: observer,
    });
  });
  await centerX.fill("-304");
  await centerX.press("Enter");
  await expect(page).toHaveURL(/x=-304/u);
  await expect(shell).toHaveAttribute("data-canonical-resident-hits", "72");
  await waitForCanonical(page, 81);
  const counts = await page.evaluate(() => {
    const holder = window as typeof window & {
      terrainLabCanonicalPanCounts?: number[];
      terrainLabCanonicalPanObserver?: MutationObserver;
    };
    holder.terrainLabCanonicalPanObserver?.disconnect();
    return holder.terrainLabCanonicalPanCounts ?? [];
  });
  expect(Math.min(...counts)).toBeGreaterThanOrEqual(72);
  expect(counts.some((count) => count > 72 && count < 81)).toBe(true);
  await expect(shell).toHaveAttribute("data-canonical-cache-hits", "0");
  await expect(shell).toHaveAttribute("data-canonical-warm-hits", "9");
  await expect(shell).toHaveAttribute("data-canonical-warm-chunks", "9");
  await expect(shell).toHaveAttribute("data-canonical-worker-mesh-ms", "0");
  await expect(shell).toHaveAttribute("data-canonical-main-decode-ms", "0");
  await expect(shell).toHaveAttribute("data-canonical-admission-frames", "9");
  await expect(shell).toHaveAttribute("data-canonical-max-frame-admissions", "1");
  await page.getByTestId("canonical-terrain-stage").screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-canonical-responsive-pan.png`,
  });

  const cacheOnEpoch = Number(await shell.getAttribute("data-canonical-epoch"));
  await page.getByRole("button", { name: "Exact cache off", exact: true }).click();
  await expect(shell).toHaveAttribute("data-canonical-cache-enabled", "false");
  await expect.poll(
    async () => Number(await shell.getAttribute("data-canonical-epoch")),
  ).toBeGreaterThan(cacheOnEpoch);
  await waitForCanonical(page, 81);
  await expect(shell).toHaveAttribute("data-canonical-resident-hits", "0");
  await expect(shell).toHaveAttribute("data-canonical-cache-hits", "0");
  await expect(shell).toHaveAttribute("data-canonical-warm-hits", "0");
  await centerX.fill("-288");
  await centerX.press("Enter");
  await expect(page).toHaveURL(/x=-288/u);
  await expect(shell).toHaveAttribute("data-canonical-resident-hits", "72");
  await waitForCanonical(page, 81);
  await expect(shell).toHaveAttribute("data-canonical-cache-hits", "0");
  await expect(shell).toHaveAttribute("data-canonical-admission-frames", "9");
  expect(pageErrors).toEqual([]);
});

test("publishes, shifts, and restores a 31x31 real footprint", async ({
  page,
}, testInfo) => {
  test.slow();
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      pageErrors.push(message.text());
    }
  });
  await page.goto(
    "/terrain/?seed=-98765&profile=mclone-overworld-v1"
      + "&x=-304&z=336&blocks=512&detail=auto"
      + "&panes=canonical&canonical=final&radius=0"
      + "&water=1&vegetation=1&stage=hydrology&view=map&layer=terrain",
  );
  await waitForCanonical(page, 1);
  const shell = page.locator(".appShell");
  const footprint = page.getByLabel("Exact chunk footprint");
  await expect(footprint.locator("option")).toHaveCount(9);
  await page.evaluate(() => {
    const counts: number[] = [];
    const element = document.querySelector(".appShell");
    const record = (): void => {
      const count = Number(element?.getAttribute("data-canonical-published") ?? "0");
      if (counts.at(-1) !== count) {
        counts.push(count);
      }
    };
    record();
    const observer = new MutationObserver(record);
    if (element) {
      observer.observe(element, {
        attributes: true,
        attributeFilter: ["data-canonical-published"],
      });
    }
    Object.assign(window, {
      terrainLabCanonicalLargeCounts: counts,
      terrainLabCanonicalLargeObserver: observer,
    });
  });

  await footprint.selectOption("15");
  await expect(page).toHaveURL(/radius=15/u);
  await expect(shell).toHaveAttribute("data-canonical-requested", "961");
  await waitForCanonical(page, 961, 180_000);
  const initialCounts = await page.evaluate(() => {
    const holder = window as typeof window & {
      terrainLabCanonicalLargeCounts?: number[];
      terrainLabCanonicalLargeObserver?: MutationObserver;
    };
    holder.terrainLabCanonicalLargeObserver?.disconnect();
    return holder.terrainLabCanonicalLargeCounts ?? [];
  });
  expect(initialCounts.some((count) => count > 1 && count < 961)).toBe(true);
  await expect(footprint).toHaveValue("15");
  await expect(shell).toHaveAttribute("data-canonical-cached-chunks", "961");
  expect(Number(await shell.getAttribute("data-canonical-resident-raw-bytes")))
    .toBe(0);
  expect(Number(await shell.getAttribute("data-canonical-cache-raw-bytes")))
    .toBeGreaterThan(0);
  expect(Number(await shell.getAttribute("data-canonical-mesh-used-bytes")))
    .toBeGreaterThan(0);
  const trackedBytes = Number(await shell.getAttribute("data-canonical-tracked-bytes"));
  const trackedParts = Number(
    await shell.getAttribute("data-canonical-resident-raw-bytes"),
  ) + Number(await shell.getAttribute("data-canonical-cache-raw-bytes"))
    + Number(await shell.getAttribute("data-canonical-mesh-used-bytes"));
  expect(trackedBytes).toBe(trackedParts);

  const centerX = page.getByLabel("Center X");
  await centerX.fill("-288");
  await centerX.press("Enter");
  await expect(page).toHaveURL(/x=-288/u);
  await expect(shell).toHaveAttribute("data-canonical-resident-hits", "930");
  await waitForCanonical(page, 961, 120_000);
  await expect(shell).toHaveAttribute("data-canonical-admission-frames", "31");
  await expect(shell).toHaveAttribute("data-canonical-max-frame-admissions", "1");
  await expect(shell).toHaveAttribute("data-canonical-warm-chunks", "31");
  expect(Number(await shell.getAttribute("data-canonical-cached-chunks")))
    .toBeLessThanOrEqual(1_024);

  await centerX.fill("-304");
  await centerX.press("Enter");
  await expect(page).toHaveURL(/x=-304/u);
  await expect(shell).toHaveAttribute("data-canonical-resident-hits", "930");
  await waitForCanonical(page, 961, 120_000);
  await expect(shell).toHaveAttribute("data-canonical-cache-hits", "0");
  await expect(shell).toHaveAttribute("data-canonical-warm-hits", "31");
  await expect(shell).toHaveAttribute("data-canonical-warm-chunks", "31");
  await expect(shell).toHaveAttribute("data-canonical-worker-mesh-ms", "0");
  await expect(shell).toHaveAttribute("data-canonical-main-decode-ms", "0");
  await expect(shell).toHaveAttribute("data-canonical-admission-frames", "31");
  await expect(shell).toHaveAttribute("data-canonical-max-frame-admissions", "1");
  expect(Number(await shell.getAttribute("data-canonical-cached-chunks")))
    .toBeLessThanOrEqual(1_024);
  await page.getByTestId("canonical-terrain-stage").screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-canonical-961.png`,
  });
  expect(pageErrors).toEqual([]);
});

async function waitForCanonical(
  page: import("@playwright/test").Page,
  requestedChunks: number,
  timeout = 30_000,
): Promise<void> {
  await page.waitForFunction((requested) => {
    const shell = document.querySelector(".appShell");
    return shell?.getAttribute("data-canonical-complete") === "true"
      && Number(shell.getAttribute("data-canonical-published")) === requested
      && Number(shell.getAttribute("data-canonical-requested")) === requested;
  }, requestedChunks, { timeout });
}

async function assertTouchPanZoom(
  page: import("@playwright/test").Page,
  stage: import("@playwright/test").Locator,
): Promise<void> {
  await stage.scrollIntoViewIfNeeded();
  const bounds = await stage.boundingBox();
  expect(bounds).not.toBeNull();
  const before = new URL(page.url());
  const beforeBlocks = Number(before.searchParams.get("blocks"));
  const beforeX = Number(before.searchParams.get("x"));
  const beforeZ = Number(before.searchParams.get("z"));
  const beforeScroll = await page.evaluate(() => window.scrollY);
  const centerX = bounds!.x + bounds!.width * 0.5;
  const centerY = bounds!.y + Math.min(bounds!.height * 0.45, 260);

  await dispatchTwoFingerGesture(
    page,
    [
      { x: centerX - 36, y: centerY },
      { x: centerX + 36, y: centerY },
    ],
    [
      { x: centerX - 42, y: centerY + 28 },
      { x: centerX + 92, y: centerY + 28 },
    ],
  );

  await expect.poll(
    () => Number(new URL(page.url()).searchParams.get("blocks")),
  ).toBeLessThan(beforeBlocks);
  await expect.poll(() => {
    const current = new URL(page.url());
    return Number(current.searchParams.get("x")) !== beforeX
      || Number(current.searchParams.get("z")) !== beforeZ;
  }).toBe(true);
  expect(await page.evaluate(() => window.scrollY)).toBe(beforeScroll);
}

async function dispatchTwoFingerGesture(
  page: import("@playwright/test").Page,
  start: Array<{ x: number; y: number }>,
  end: Array<{ x: number; y: number }>,
): Promise<void> {
  const session = await page.context().newCDPSession(page);
  try {
    await session.send("Input.dispatchTouchEvent", {
      type: "touchStart",
      touchPoints: touchPoints(start),
    });
    for (let step = 1; step <= 4; step += 1) {
      const amount = step / 4;
      await session.send("Input.dispatchTouchEvent", {
        type: "touchMove",
        touchPoints: touchPoints(start.map((point, index) => ({
          x: point.x + (end[index]!.x - point.x) * amount,
          y: point.y + (end[index]!.y - point.y) * amount,
        }))),
      });
    }
    await session.send("Input.dispatchTouchEvent", {
      type: "touchEnd",
      touchPoints: [],
    });
  } finally {
    await session.detach();
  }
}

function touchPoints(points: Array<{ x: number; y: number }>): Array<{
  x: number;
  y: number;
  id: number;
  radiusX: number;
  radiusY: number;
}> {
  return points.map((point, id) => ({
    ...point,
    id,
    radiusX: 4,
    radiusY: 4,
  }));
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
