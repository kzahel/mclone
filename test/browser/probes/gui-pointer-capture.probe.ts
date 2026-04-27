import { expect, test } from "@playwright/test";
import type { GpuTitleBootResult } from "../../../src/renderer/main";
import { FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

test("GUI pointer capture tolerates duplicate or synthetic pointerdown events", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(String(error)));

  await page.goto("/", { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  const result = (await page.evaluate(() => window.__mcloneReady)) as GpuTitleBootResult | { readonly ok: false; readonly reason: string };
  expect(result.ok, JSON.stringify(result)).toBe(true);
  await page.waitForFunction(() => window.__mcloneGui?.state.ready === true, undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });

  const box = await page.locator("#renderer").boundingBox();
  expect(box).not.toBeNull();
  await page.dispatchEvent("#renderer", "pointerdown", {
    pointerId: 42,
    pointerType: "mouse",
    button: 0,
    buttons: 1,
    clientX: box!.x + 10,
    clientY: box!.y + 10,
  });
  await page.dispatchEvent("#renderer", "pointerdown", {
    pointerId: 42,
    pointerType: "mouse",
    button: 0,
    buttons: 1,
    clientX: box!.x + 10,
    clientY: box!.y + 10,
  });
  await page.mouse.dblclick(box!.x + 10, box!.y + 10);

  expect(pageErrors).toEqual([]);
});
