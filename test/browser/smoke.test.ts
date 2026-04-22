import { test, expect } from "@playwright/test";
import type { BootResult } from "../../src/renderer/main.ts";

test("WebGPU boot succeeds on system Chrome", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(String(err)));

  await page.goto("/");

  const result = (await page.evaluate(() => window.__mcloneReady)) as BootResult;

  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
  expect(result.ok, JSON.stringify(result)).toBe(true);
  if (result.ok) {
    expect(result.adapterInfo.length).toBeGreaterThan(0);
    expect(["bgra8unorm", "rgba8unorm"]).toContain(result.format);
  }
});
