import { expect, test, type Page } from "./remote-world-host-fixture";
import { type BootResult } from "../../src/renderer/main.ts";
import {
  createGeneratedWorldSmokeSearchParams,
  validateGeneratedWorldSmokeResult,
} from "../../src/renderer/generated-world-smoke-scenario.ts";

const REMOTE_SMOKE_SCREENSHOT_PATH = "/tmp/mclone-browser-remote-smoke.png";
const WORKER_SMOKE_SCREENSHOT_PATH = "/tmp/mclone-browser-worker-smoke.png";

function createSmokeParams(extra: Record<string, string>): URLSearchParams {
  return createGeneratedWorldSmokeSearchParams(extra);
}

function createRemoteSmokeUrl(remoteWorldHostUrl: string): string {
  return `/smoke.html?${createSmokeParams({
    worldTransport: "remote",
    worldHostUrl: remoteWorldHostUrl,
  }).toString()}`;
}

function createWorkerSmokeUrl(): string {
  return `/smoke.html?${createSmokeParams({
    worldTransport: "worker",
    worldStorageMode: "none",
  }).toString()}`;
}

async function bootPage(page: Page, url: string): Promise<BootResult> {
  await page.goto(url, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined");
  return (await page.evaluate(() => window.__mcloneReady)) as BootResult;
}

function expectRenderedSmokeResult(
  result: BootResult,
  expectedTransport: "worker" | "remote",
): asserts result is Extract<BootResult, { ok: true }> {
  expect(result.ok, JSON.stringify(result)).toBe(true);
  if (!result.ok) {
    return;
  }

  expect(result.worldTransport).toBe(expectedTransport);
  expect(result.meshTransport).toBe("worker");
  expect(result.adapterInfo.length).toBeGreaterThan(0);
  expect(["bgra8unorm", "rgba8unorm"]).toContain(result.format);
  expect(validateGeneratedWorldSmokeResult(result, {
    expectedWorldTransport: expectedTransport,
    requirePlayerInput: true,
  })).toEqual([]);
}

test.setTimeout(30_000);

test("WebGPU boot succeeds against the remote Node host with two browser clients", async ({ browser, remoteWorldHostUrl }) => {
  const context = await browser.newContext();
  const firstPage = await context.newPage();
  const secondPage = await context.newPage();
  const pageErrors: string[] = [];
  firstPage.on("pageerror", (err) => pageErrors.push(`first: ${String(err)}`));
  secondPage.on("pageerror", (err) => pageErrors.push(`second: ${String(err)}`));

  const [firstResult, secondResult] = await Promise.all([
    bootPage(firstPage, createRemoteSmokeUrl(remoteWorldHostUrl)),
    bootPage(secondPage, createRemoteSmokeUrl(remoteWorldHostUrl)),
  ]);
  await firstPage.locator("#renderer").screenshot({ path: REMOTE_SMOKE_SCREENSHOT_PATH });

  expectRenderedSmokeResult(firstResult, "remote");
  expectRenderedSmokeResult(secondResult, "remote");

  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
  expect(firstResult.saveId).toBe(secondResult.saveId);
  expect(firstResult.sessionId).not.toBe(secondResult.sessionId);
  expect(firstResult.playerId).not.toBe(secondResult.playerId);
});

test("WebGPU boot succeeds against the worker integrated server", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(String(err)));

  const result = await bootPage(page, createWorkerSmokeUrl());
  await page.locator("#renderer").screenshot({ path: WORKER_SMOKE_SCREENSHOT_PATH });

  expectRenderedSmokeResult(result, "worker");
  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
});
