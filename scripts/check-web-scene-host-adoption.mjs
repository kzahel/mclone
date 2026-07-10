import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const enforce = !process.argv.includes("--baseline");
const json = process.argv.includes("--json");

const sources = {
  rust: {
    path: "native/apps/mclone-web-client/src/{web_canvas,web_scene_host}.rs",
    text: ["web_canvas.rs", "web_scene_host.rs"]
      .map((name) => readFileSync(
        resolve(repoRoot, "native/apps/mclone-web-client/src", name),
        "utf8",
      ))
      .join("\n"),
  },
  ts: {
    path: "native/apps/mclone-web-client/www/mclone-web-app.ts",
    text: readFileSync(resolve(repoRoot, "native/apps/mclone-web-client/www/mclone-web-app.ts"), "utf8"),
  },
};

// Baselines remain the historical pre-cutover inventory. Enforcement is the
// default after Slice 5; --baseline exists only for archaeology.
const patterns = [
  {
    id: "combined-rust-owner",
    source: "rust",
    needle: "struct WebChunkRenderSession",
    baseline: 1,
    destination: "McloneSceneHost + WebFrameDriver",
  },
  {
    id: "combined-ts-owner",
    source: "ts",
    needle: "class WebChunkApp",
    baseline: 1,
    destination: "WebFrameDriver",
  },
  {
    id: "settings-policy",
    source: "rust",
    needle: "apply_client_experience_settings_effects(",
    baseline: 2,
    destination: "McloneSceneHost",
  },
  {
    id: "session-policy",
    source: "rust",
    needle: "apply_client_session_effects(",
    baseline: 2,
    destination: "McloneSceneHost",
  },
  {
    id: "session-transition-policy",
    source: "rust",
    needle: "apply_client_session_transition_effects(",
    baseline: 5,
    destination: "McloneSceneHost",
  },
  {
    id: "runtime-install-policy",
    source: "rust",
    needle: "install_started_runtime_with_descriptor(",
    baseline: 3,
    destination: "neutral scene runtime service",
  },
  {
    id: "camera-semantics",
    source: "rust",
    needle: "EngineCameraInput {",
    baseline: 1,
    destination: "McloneSceneHost",
  },
  {
    id: "render-admission",
    source: "rust",
    needle: ".sync_render_sections_with_budget(",
    baseline: 1,
    destination: "McloneSceneHost",
  },
  {
    id: "frame-assembly",
    source: "rust",
    needle: "render_chunk_report_with_cache_update(",
    baseline: 3,
    destination: "McloneSceneHost + WebFrameDriver presentation",
  },
  {
    id: "async-session-dispatch",
    source: "ts",
    needle: "handleSessionStartAction(",
    baseline: 2,
    destination: "typed platform operation executor",
  },
  {
    id: "async-catalog-dispatch",
    source: "ts",
    needle: "handleWorldCatalogRequest(",
    baseline: 2,
    destination: "typed platform operation executor",
  },
  {
    id: "async-session-restart",
    source: "ts",
    needle: "restartSessionFromUiAction(",
    baseline: 4,
    destination: "typed platform operation executor",
  },
  {
    id: "compiler-wake-relay",
    source: "ts",
    needle: "startAndPostCompileTiming(",
    baseline: 2,
    destination: "browser render-compiler service",
  },
];

const countOccurrences = (text, needle) => {
  let count = 0;
  let offset = 0;
  while (true) {
    const found = text.indexOf(needle, offset);
    if (found < 0) return count;
    count += 1;
    offset = found + needle.length;
  }
};

const inventory = patterns.map((pattern) => ({
  ...pattern,
  path: sources[pattern.source].path,
  count: countOccurrences(sources[pattern.source].text, pattern.needle),
}));
const lineCounts = Object.fromEntries(Object.entries(sources).map(([id, source]) => [
  source.path,
  source.text.split(/\r?\n/u).length - 1,
]));

if (json) {
  console.log(JSON.stringify({ mode: enforce ? "enforce" : "baseline", lineCounts, inventory }, null, 2));
} else {
  console.log(`web scene-host adoption source inventory (${enforce ? "enforce" : "baseline/warning"})`);
  for (const [path, lines] of Object.entries(lineCounts)) {
    console.log(`  ${path}: ${lines} lines`);
  }
  for (const item of inventory) {
    const drift = item.count === item.baseline ? "baseline" : `baseline ${item.baseline}`;
    console.log(`  ${item.id}: ${item.count} (${drift}) -> ${item.destination}`);
  }
}

if (enforce) {
  const remaining = inventory.filter((item) => item.count > 0);
  if (remaining.length > 0) {
    console.error("web scene-host cutover still contains app-local policy owners:");
    for (const item of remaining) {
      console.error(`  ${item.path}: ${item.id} (${item.count} x ${JSON.stringify(item.needle)})`);
    }
    process.exitCode = 1;
  }
} else {
  const drifted = inventory.filter((item) => item.count !== item.baseline);
  if (drifted.length > 0) {
    console.warn("warning: pre-cutover source inventory drifted; review and update the tactical evidence");
  }
  console.warn("warning: baseline mode does not enforce the landed Slice 5 boundary");
}
