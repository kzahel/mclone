import { chromium } from "@playwright/test";

const url = process.argv[2] ?? "https://mclone.kzahel.com/?mode=debug";
const out = process.argv[3] ?? "/tmp/mclone-smoke.png";
const webGpuLaunchArgs = [
  "--enable-unsafe-webgpu",
  ...(process.platform === "darwin" ? ["--use-angle=metal"] : []),
];

const browser = await chromium.launch({
  channel: "chrome",
  args: webGpuLaunchArgs,
});
const ctx = await browser.newContext({ viewport: { width: 800, height: 600 } });
const page = await ctx.newPage();

const logs = [];
page.on("console", (msg) => logs.push(`[${msg.type()}] ${msg.text()}`));
page.on("pageerror", (err) => logs.push(`[pageerror] ${err.message}`));
page.on("requestfailed", (req) =>
  logs.push(`[reqfail] ${req.url()} -- ${req.failure()?.errorText ?? ""}`),
);
const notOk = [];
page.on("response", (res) => {
  if (res.status() >= 400) notOk.push(`[${res.status()}] ${res.url()}`);
});

await page
  .goto(url, { waitUntil: "domcontentloaded", timeout: 20000 })
  .catch((e) => logs.push(`[goto] ${e.message}`));
await page.waitForTimeout(20000);
await page.screenshot({ path: out, fullPage: false });

const domControlCount = await page.locator("button, input, select, textarea, details, form").count();
const runtimeState = await page.evaluate(() => window.__mcloneDebug?.state ?? window.__mcloneGui?.state ?? null);
console.log("=== gpu runtime state ===");
console.log(JSON.stringify(runtimeState, null, 2));
console.log(`DOM controls: ${domControlCount}`);
console.log("=== logs (last 60) ===");
console.log(logs.slice(-60).join("\n"));
console.log("=== not-ok responses ===");
console.log(notOk.join("\n"));
console.log(`=== screenshot: ${out} ===`);

await browser.close();
