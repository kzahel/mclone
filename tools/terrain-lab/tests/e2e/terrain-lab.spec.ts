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

  const diagnostics = page.locator("[data-testid='terrain-diagnostics']");
  await expect(diagnostics).not.toContainText("Mean height Δpending");
  const canvas = page.locator("canvas[aria-label='Live GPU terrain preview']");
  await expect(canvas).toBeVisible();
  await canvas.screenshot({
    path: `/tmp/mclone-terrain-lab-${testInfo.project.name}-initial.png`,
  });

  const shell = page.locator(".appShell");
  const initialRevision = Number(await shell.getAttribute("data-render-revision"));
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
