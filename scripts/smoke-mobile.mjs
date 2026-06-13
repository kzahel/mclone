import { chromium, devices } from "@playwright/test";

const url = process.argv[2] ?? "https://mclone.kzahel.com/?mode=debug";
const outBefore = "/tmp/mclone-smoke-mobile-before.png";
const outDuring = "/tmp/mclone-smoke-mobile-during.png";
const webGpuLaunchArgs = [
  "--enable-unsafe-webgpu",
  ...(process.platform === "darwin" ? ["--use-angle=metal"] : []),
];

const browser = await chromium.launch({
  channel: "chrome",
  args: webGpuLaunchArgs,
});
const ctx = await browser.newContext({ ...devices["iPhone 14 Pro"] });
const page = await ctx.newPage();

const logs = [];
page.on("console", (msg) => logs.push(`[${msg.type()}] ${msg.text()}`));
page.on("pageerror", (err) => logs.push(`[pageerror] ${err.message}`));

await page.goto(url, { waitUntil: "domcontentloaded", timeout: 20000 }).catch((e) => logs.push(`[goto] ${e.message}`));
await page.waitForTimeout(15000);
await page.screenshot({ path: outBefore, fullPage: false });

// Simulate a touch drag on the left half (virtual joystick).
const viewport = page.viewportSize() ?? { width: 390, height: 844 };
const startX = viewport.width * 0.25;
const startY = viewport.height * 0.7;

await page.touchscreen.tap(startX, startY);
await page.evaluate(
  ([x, y]) => {
    const touch = new Touch({
      identifier: 1,
      target: document.getElementById("renderer"),
      clientX: x,
      clientY: y,
      pageX: x,
      pageY: y,
      screenX: x,
      screenY: y,
      radiusX: 1,
      radiusY: 1,
    });
    const start = new TouchEvent("touchstart", {
      bubbles: true,
      cancelable: true,
      touches: [touch],
      targetTouches: [touch],
      changedTouches: [touch],
    });
    document.getElementById("renderer").dispatchEvent(start);
    const moveTouch = new Touch({
      identifier: 1,
      target: document.getElementById("renderer"),
      clientX: x + 40,
      clientY: y - 30,
      pageX: x + 40,
      pageY: y - 30,
      screenX: x + 40,
      screenY: y - 30,
      radiusX: 1,
      radiusY: 1,
    });
    const move = new TouchEvent("touchmove", {
      bubbles: true,
      cancelable: true,
      touches: [moveTouch],
      targetTouches: [moveTouch],
      changedTouches: [moveTouch],
    });
    document.getElementById("renderer").dispatchEvent(move);
  },
  [startX, startY],
);

await page.waitForTimeout(200);
await page.screenshot({ path: outDuring, fullPage: false });

const domControlCount = await page.locator("button, input, select, textarea, details, form").count();
const runtimeState = await page.evaluate(() => window.__mcloneDebug?.state ?? window.__mcloneGui?.state ?? null);
console.log("=== gpu runtime state ===");
console.log(JSON.stringify(runtimeState, null, 2));
console.log(`DOM controls: ${domControlCount}`);
console.log("=== logs (last 15) ===");
console.log(logs.slice(-15).join("\n"));
console.log(`=== before: ${outBefore}`);
console.log(`=== during: ${outDuring}`);

await browser.close();
