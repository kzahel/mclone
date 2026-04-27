#!/usr/bin/env node
import { build, preview } from "vite";

process.env.VITE_LAN_RELOAD = "1";

const watcher = await build({
  build: { watch: {} },
});

let firstBuildDone = false;

watcher.on("event", async (event) => {
  if (event.code === "ERROR") {
    console.error("[dev:lan] build error:", event.error?.message ?? event.error);
    return;
  }
  if (event.code === "BUNDLE_END") {
    console.log(`[dev:lan] rebuild done in ${event.duration}ms`);
  }
  if (event.code === "END" && !firstBuildDone) {
    firstBuildDone = true;
    const server = await preview();
    server.printUrls();
  }
});

const shutdown = async () => {
  try {
    await watcher.close?.();
  } catch {}
  process.exit(0);
};
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
