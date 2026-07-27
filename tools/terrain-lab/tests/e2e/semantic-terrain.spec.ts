import { expect, test } from "@playwright/test";

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
  const initialChecksum = await shell.getAttribute("data-semantic-checksum");
  expect(initialChecksum).toMatch(/^[0-9a-f]{64}$/u);

  const bounds = await stage.boundingBox();
  expect(bounds).toBeTruthy();
  if (testInfo.project.name === "phone-chrome") {
    expect(bounds!.height).toBeGreaterThan(bounds!.width * 2.5);
  } else {
    expect(bounds!.height).toBeGreaterThan(700);
  }

  await page.getByRole("button", { name: "Map", exact: true }).click();
  await expect(page).toHaveURL(/view=map/u);

  const lifted = new URL(page.url());
  lifted.searchParams.set("x", String(1024 + 6144));
  lifted.searchParams.set("z", String(-768 - 6144));
  await page.goto(lifted.href, { waitUntil: "networkidle" });
  await expect(shell).toHaveAttribute("data-semantic-ready", "true");
  await expect(shell).toHaveAttribute(
    "data-semantic-checksum",
    initialChecksum!,
  );
  expect(pageErrors).toEqual([]);
});
