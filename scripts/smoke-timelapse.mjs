import { chromium } from "@playwright/test";

const url = process.argv[2] ?? "https://mclone.kzahel.com/?mode=debug";

const browser = await chromium.launch({
  channel: "chrome",
  args: ["--enable-unsafe-webgpu"],
});
const ctx = await browser.newContext({ viewport: { width: 800, height: 600 } });
const page = await ctx.newPage();

const logs = [];
page.on("console", (msg) => logs.push(`[${msg.type()}] ${msg.text()}`));
page.on("pageerror", (err) => logs.push(`[pageerror] ${err.message}\n${err.stack ?? ""}`));
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

// Let scene init complete.
await page.waitForTimeout(15000);

// Take screenshots at 0s, 1s, 3s, 6s after first render.
for (const t of [0, 1000, 3000, 6000]) {
  if (t > 0) await page.waitForTimeout(t);
  const out = `/tmp/mclone-smoke-t${t}.png`;
  await page.screenshot({ path: out, fullPage: false });
  console.log(`[${t}ms] ${out}`);
}

const overlay = await page
  .locator("#debug-overlay")
  .textContent()
  .catch(() => null);
console.log("=== overlay ===");
console.log(overlay);
console.log("=== logs (last 40) ===");
console.log(logs.slice(-40).join("\n"));
console.log("=== not-ok responses ===");
console.log(notOk.join("\n"));

await browser.close();
