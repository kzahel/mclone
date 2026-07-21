import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const wwwRoot = resolve(repoRoot, "native/apps/mclone-web-client/www");
const json = process.argv.includes("--json");
const selfTest = process.argv.includes("--self-test");

const moduleRegistry = {
  "mclone-integrated-server-worker.ts": {
    family: "server-workers",
    baselineLines: 1202,
    workerEntry: true,
    workerConstruction: false,
    responsibilities: [
      "browser timer/yield mechanics",
      "generic persistence executor servicing",
      "server-job Worker servicing",
      "external SAB runner mailbox",
      "opaque Rust startup forwarding",
    ],
  },
  "mclone-remote-websocket-worker.ts": {
    family: "server-workers",
    baselineLines: 201,
    workerEntry: true,
    workerConstruction: false,
    responsibilities: [
      "browser WebSocket mechanics",
      "opaque worker-resident Rust actor execution",
    ],
  },
  "mclone-render-compiler-abi.d.ts": {
    family: "render-workers",
    baselineLines: 16,
    workerEntry: false,
    workerConstruction: false,
    responsibilities: ["TypeScript declarations for the locked external SAB ABI"],
  },
  "mclone-render-compiler-shared.ts": {
    family: "render-workers",
    baselineLines: 845,
    workerEntry: false,
    workerConstruction: false,
    responsibilities: [
      "asset fetch mechanics",
      "worker-safe SAB predicates",
    ],
  },
  "mclone-render-compiler-worker.ts": {
    family: "render-workers",
    baselineLines: 69,
    workerEntry: true,
    workerConstruction: false,
    responsibilities: [
      "browser Wasm module loading",
      "opaque worker-resident Rust render-actor forwarding",
    ],
  },
  "mclone-runner-shared-abi.d.ts": {
    family: "render-workers",
    baselineLines: 7,
    workerEntry: false,
    workerConstruction: false,
    responsibilities: ["TypeScript declarations for the locked runner/job SAB ABI"],
  },
  "mclone-server-job-worker.ts": {
    family: "server-workers",
    baselineLines: 164,
    workerEntry: true,
    workerConstruction: false,
    responsibilities: [
      "external SAB job mailbox",
      "message-transfer fallback",
      "domain-blind Rust actor forwarding",
    ],
  },
  "mclone-thread-smoke-worker.ts": {
    family: "threading-smoke",
    baselineLines: 47,
    workerEntry: true,
    workerConstruction: false,
    responsibilities: ["non-production shared-Wasm-memory capability proof"],
  },
  "mclone-web-app.ts": {
    family: "web-app",
    baselineLines: 2650,
    workerEntry: false,
    workerConstruction: false,
    responsibilities: [
      "rAF and browser presentation",
      "browser service assembly",
      "registered compiler-wake and async-adapter debt",
    ],
  },
  "mclone-web-input.ts": {
    family: "input-touch",
    baselineLines: 383,
    workerEntry: false,
    workerConstruction: false,
    responsibilities: ["DOM keyboard/mouse/game-input translation"],
  },
  "mclone-web-persistence-executor.ts": {
    family: "server-workers",
    baselineLines: 0,
    workerEntry: false,
    workerConstruction: false,
    responsibilities: [
      "generic IndexedDB namespace/store mapping",
      "opaque record transactions and browser error classification",
    ],
  },
  "mclone-web-smoke-observer.ts": {
    family: "threading-smoke",
    baselineLines: 0,
    workerEntry: false,
    workerConstruction: false,
    responsibilities: [
      "query-gated test-client diagnostic exposure",
      "smoke-only semantic command registry",
    ],
  },
  "mclone-web-touch.ts": {
    family: "input-touch",
    baselineLines: 503,
    workerEntry: false,
    workerConstruction: false,
    responsibilities: ["DOM touch/pointer translation and fullscreen gesture"],
  },
  "mclone-worker-transport.ts": {
    family: "worker-transports",
    baselineLines: 0,
    workerEntry: false,
    workerConstruction: true,
    responsibilities: [
      "generic browser Worker construction",
      "opaque polled event transport",
      "postMessage transfer mechanics",
    ],
  },
  "mclone-web-world-catalog.ts": {
    family: "catalog-settings",
    baselineLines: 414,
    workerEntry: false,
    workerConstruction: false,
    responsibilities: [
      "IndexedDB schema/transaction mechanics",
      "opaque Rust catalog record and UI projection transport",
    ],
  },
  "mclone-web-world-lease.ts": {
    family: "catalog-settings",
    baselineLines: 0,
    workerEntry: false,
    workerConstruction: false,
    responsibilities: ["generic world-keyed Web Lock lifetime mechanics"],
  },
};

const domainDebt = [
  {
    id: "server-job-worldgen-switch",
    file: "mclone-server-job-worker.ts",
    needle: 'case "worldgen":',
    maximum: 0,
    destination: "worker-resident Rust server-job actor",
  },
  {
    id: "server-job-light-switch",
    file: "mclone-server-job-worker.ts",
    needle: 'case "light-status":',
    maximum: 0,
    destination: "worker-resident Rust server-job actor",
  },
  {
    id: "server-job-worldgen-session",
    file: "mclone-server-job-worker.ts",
    needle: "new module.WebWorldgenJobSession()",
    maximum: 0,
    destination: "worker-resident Rust server-job actor",
  },
  {
    id: "server-job-light-entrypoint",
    file: "mclone-server-job-worker.ts",
    needle: "mclone_web_compute_light_status_job_frame(",
    maximum: 0,
    destination: "worker-resident Rust server-job actor",
  },
  {
    id: "render-main-broker-class",
    file: "mclone-render-compiler-shared.ts",
    needle: "export class RenderSectionWorkerCompiler",
    maximum: 0,
    destination: "Rust render-worker coordinator",
  },
  {
    id: "render-main-priority-selection",
    file: "mclone-render-compiler-shared.ts",
    needle: 'worldPriority === "active"',
    maximum: 0,
    destination: "Rust render-worker coordinator",
  },
  {
    id: "render-worker-world-sessions",
    file: "mclone-render-compiler-worker.ts",
    needle: "compilerSessions = new Map",
    maximum: 0,
    destination: "worker-resident Rust render actor",
  },
  {
    id: "render-worker-world-fork",
    file: "mclone-render-compiler-worker.ts",
    needle: "compilerTemplate.forkWorldSession()",
    maximum: 0,
    destination: "worker-resident Rust render actor",
  },
  {
    id: "render-worker-kind-selection",
    file: "mclone-render-compiler-worker.ts",
    needle: 'message.workKind === "far-lod"',
    maximum: 0,
    destination: "worker-resident Rust render actor",
  },
  {
    id: "integrated-worker-generation-profile-field",
    file: "mclone-integrated-server-worker.ts",
    needle: "generationProfile?:",
    maximum: 0,
    destination: "opaque Rust-authored server startup descriptor",
  },
  {
    id: "integrated-worker-generation-profile-application",
    file: "mclone-integrated-server-worker.ts",
    needle: "setWorldGenerationProfile",
    maximum: 0,
    destination: "worker-resident Rust integrated-server actor",
  },
  {
    id: "integrated-worker-topology-field",
    file: "mclone-integrated-server-worker.ts",
    needle: "worldTopology?:",
    maximum: 0,
    destination: "opaque Rust-authored server startup descriptor",
  },
  {
    id: "integrated-worker-behavior-field",
    file: "mclone-integrated-server-worker.ts",
    needle: "behaviorProfile?:",
    maximum: 0,
    destination: "opaque Rust-authored server startup descriptor",
  },
  {
    id: "integrated-worker-behavior-application",
    file: "mclone-integrated-server-worker.ts",
    needle: "setWorldBehaviorProfile",
    maximum: 0,
    destination: "worker-resident Rust integrated-server actor",
  },
  {
    id: "integrated-worker-authority-defaults",
    file: "mclone-integrated-server-worker.ts",
    needle: "setLocalPlayerIdentity",
    maximum: 0,
    destination: "worker-resident Rust integrated-server actor",
  },
  {
    id: "integrated-worker-pending-job-policy",
    file: "mclone-integrated-server-worker.ts",
    needle: "hasPendingServerJobs",
    maximum: 0,
    destination: "worker-resident Rust integrated-server actor",
  },
  {
    id: "integrated-worker-command-dispatch",
    file: "mclone-integrated-server-worker.ts",
    needle: "handleCommandFrame",
    maximum: 0,
    destination: "worker-resident Rust integrated-server actor",
  },
  {
    id: "integrated-worker-role-dispatch",
    file: "mclone-integrated-server-worker.ts",
    needle: "promoteObserverToPlayer",
    maximum: 0,
    destination: "worker-resident Rust integrated-server actor",
  },
  {
    id: "integrated-worker-completion-policy",
    file: "mclone-integrated-server-worker.ts",
    needle: 'kind: "command-result"',
    maximum: 0,
    destination: "worker-resident Rust integrated-server actor",
  },
  {
    id: "integrated-worker-domain-load-protocol",
    file: "mclone-integrated-server-worker.ts",
    needle: "indexedDbLoadRequests",
    maximum: 0,
    destination: "Rust record coordinator and generic IndexedDB executor",
  },
  {
    id: "integrated-worker-domain-dirty-projection",
    file: "mclone-integrated-server-worker.ts",
    needle: "indexedDbWorldMetadata",
    maximum: 0,
    destination: "Rust record coordinator and generic IndexedDB executor",
  },
  {
    id: "integrated-worker-domain-dimension-key",
    file: "mclone-integrated-server-worker.ts",
    needle: "dimensionKey",
    maximum: 0,
    destination: "Rust record coordinator and generic IndexedDB executor",
  },
  {
    id: "integrated-worker-domain-player-key",
    file: "mclone-integrated-server-worker.ts",
    needle: "playerKey",
    maximum: 0,
    destination: "Rust record coordinator and generic IndexedDB executor",
  },
  {
    id: "remote-worker-handshake-state",
    file: "mclone-remote-websocket-worker.ts",
    needle: "handshakeComplete",
    maximum: 0,
    destination: "worker-resident Rust remote socket actor",
  },
  {
    id: "remote-worker-protocol-decode",
    file: "mclone-remote-websocket-worker.ts",
    needle: "mclone_web_decode_remote_update_batch",
    maximum: 0,
    destination: "worker-resident Rust remote socket actor",
  },
  {
    id: "remote-worker-command-canonicalization",
    file: "mclone-remote-websocket-worker.ts",
    needle: "mclone_web_canonicalize_remote_command",
    maximum: 0,
    destination: "worker-resident Rust remote socket actor",
  },
  {
    id: "remote-worker-batch-accounting",
    file: "mclone-remote-websocket-worker.ts",
    needle: "MAX_UNCONSUMED_UPDATE_BYTES",
    maximum: 0,
    destination: "worker-resident Rust remote socket actor",
  },
  {
    id: "remote-worker-release-vocabulary",
    file: "mclone-remote-websocket-worker.ts",
    needle: "updates-drained",
    maximum: 0,
    destination: "opaque Rust-authored actor frames",
  },
  {
    id: "catalog-generation-profile-union",
    file: "mclone-web-world-catalog.ts",
    needle: "export type WebWorldGenerationProfile",
    maximum: 0,
    destination: "Rust-authored opaque persisted descriptor",
  },
  {
    id: "catalog-clear-summary-write",
    file: "mclone-web-world-catalog.ts",
    needle: ".add(summary)",
    maximum: 0,
    destination: "Rust-authored opaque persisted descriptor",
  },
  {
    id: "catalog-clear-summary-put",
    file: "mclone-web-world-catalog.ts",
    needle: ".put(summary)",
    maximum: 0,
    destination: "Rust-authored opaque persisted descriptor",
  },
  {
    id: "app-catalog-operation-vocabulary",
    file: "mclone-web-app.ts",
    needle: "catalogOperation",
    maximum: 0,
    destination: "Rust-owned catalog continuation",
  },
  {
    id: "app-catalog-operation-switch",
    file: "mclone-web-app.ts",
    needle: "executeWorldCatalogRequest",
    maximum: 0,
    destination: "Rust-owned catalog continuation",
  },
  {
    id: "app-catalog-generation-default",
    file: "mclone-web-app.ts",
    needle: "catalogGenerationProfile",
    maximum: 0,
    destination: "typed Rust catalog request",
  },
  {
    id: "app-session-operation-kind",
    file: "mclone-web-app.ts",
    needle: "sessionOperationKind",
    maximum: 0,
    destination: "Rust-owned pending scene-session start",
  },
  {
    id: "app-session-indexeddb-dispatch",
    file: "mclone-web-app.ts",
    needle: "startIndexedDbLocalWorld",
    maximum: 0,
    destination: "browser Rust scene-session adapter",
  },
  {
    id: "app-session-remote-dispatch",
    file: "mclone-web-app.ts",
    needle: "joinRemoteWebSocket",
    maximum: 0,
    destination: "browser Rust scene-session adapter",
  },
  {
    id: "app-streaming-runner-predicate",
    file: "mclone-web-app.ts",
    needle: "const runnerSettled =",
    maximum: 0,
    destination: "Rust-authored scene streaming readiness",
  },
  {
    id: "app-initial-presentation-counter",
    file: "mclone-web-app.ts",
    needle: "let stableFrames =",
    maximum: 0,
    destination: "Rust-owned initial-presentation latch",
  },
  {
    id: "integrated-ready-kind-rewrite",
    file: "mclone-integrated-server-worker.ts",
    needle: "ready.result.kind",
    maximum: 0,
    destination: "Rust-authored integrated-server ready envelope",
  },
  {
    id: "integrated-ready-request-rewrite",
    file: "mclone-integrated-server-worker.ts",
    needle: "ready.result.requestId",
    maximum: 0,
    destination: "Rust-authored integrated-server ready envelope",
  },
  {
    id: "integrated-ready-updates-rewrite",
    file: "mclone-integrated-server-worker.ts",
    needle: "ready.result.updates",
    maximum: 0,
    destination: "Rust-authored integrated-server ready envelope",
  },
  {
    id: "catalog-operation-helper",
    file: "mclone-web-world-catalog.ts",
    needle: "createIndexedDbCatalogWorld",
    maximum: 0,
    destination: "Rust-owned catalog continuation",
  },
  {
    id: "catalog-policy-callback",
    file: "mclone-web-world-catalog.ts",
    needle: "mclone_web_catalog_prepare_create_world",
    maximum: 0,
    destination: "Rust-owned catalog continuation",
  },
  {
    id: "app-lobby-scenario-id",
    file: "mclone-web-app.ts",
    needle: "scenarioId",
    maximum: 0,
    destination: "Rust-owned lobby coordinator",
  },
  {
    id: "app-lobby-world-role",
    file: "mclone-web-app.ts",
    needle: "operation.role",
    maximum: 0,
    destination: "Rust-owned lobby coordinator",
  },
  {
    id: "app-lobby-operation-kind",
    file: "mclone-web-app.ts",
    needle: "operation.kind",
    maximum: 0,
    destination: "opaque Rust lobby runtime ticket",
  },
  {
    id: "app-lobby-request-id",
    file: "mclone-web-app.ts",
    needle: "operation.requestId",
    maximum: 0,
    destination: "Rust-owned lobby operation ledger",
  },
  {
    id: "app-lobby-prepare-start",
    file: "mclone-web-app.ts",
    needle: "prepareLobbyWorldStart",
    maximum: 0,
    destination: "opaque Rust lobby runtime ticket",
  },
  {
    id: "app-lobby-discard-map",
    file: "mclone-web-app.ts",
    needle: "discardLobbyOperations",
    maximum: 0,
    destination: "shared lobby cancellation epoch",
  },
  {
    id: "catalog-content-version-policy",
    file: "mclone-web-world-catalog.ts",
    needle: "contentVersion",
    maximum: 0,
    destination: "Rust-owned content policy",
  },
  {
    id: "catalog-content-fingerprint-policy",
    file: "mclone-web-world-catalog.ts",
    needle: "fingerprint",
    maximum: 0,
    destination: "Rust-owned content policy",
  },
  {
    id: "catalog-lobby-validation-policy",
    file: "mclone-web-world-catalog.ts",
    needle: "ManagedWorldValidation",
    maximum: 0,
    destination: "Rust-owned content policy",
  },
  {
    id: "catalog-lobby-repair-policy",
    file: "mclone-web-world-catalog.ts",
    needle: "repairedIncompatible",
    maximum: 0,
    destination: "Rust-owned content policy",
  },
  {
    id: "app-render-compiler-wake",
    file: "mclone-web-app.ts",
    needle: "wakeRenderCompiler(",
    maximum: 0,
    destination: "Rust render-worker coordinator",
  },
  {
    id: "app-render-compiler-construction",
    file: "mclone-web-app.ts",
    needle: "createRenderCompiler(",
    maximum: 0,
    destination: "Rust render-worker coordinator",
  },
];

const copyFacts = [
  {
    id: "render-main-rust-to-sab",
    path: "native/apps/mclone-web-client/src/web_canvas.rs",
    needle: "view.copy_from(input_bytes);",
  },
  {
    id: "render-sab-to-worker-rust",
    path: "native/apps/mclone-web-client/src/web_canvas.rs",
    needle: "snapshot_input_bytes.to_vec()",
  },
  {
    id: "render-worker-rust-to-sab",
    path: "native/apps/mclone-web-client/src/web_render_worker_actor.rs",
    needle: "view.copy_from(packed);",
  },
  {
    id: "render-sab-to-main-rust",
    path: "native/apps/mclone-web-client/src/web_canvas.rs",
    needle: "Ok(view.to_vec())",
  },
  {
    id: "server-job-parent-rust-to-sab",
    path: "native/crates/mclone-server/src/wasm_job_worker.rs",
    needle: "request_view.copy_from(frame);",
  },
  {
    id: "server-job-worker-js-to-sab",
    path: "native/apps/mclone-web-client/www/mclone-server-job-worker.ts",
    needle: ".set(response);",
  },
  {
    id: "server-job-sab-to-parent-rust",
    path: "native/crates/mclone-server/src/wasm_job_worker.rs",
    needle: "Uint8Array::new(&frame).to_vec()",
  },
];

const genericTransportForbidden = [
  "assetEpoch",
  "compile",
  "farLod",
  "requestId",
  "worldInstance",
  "worldPriority",
];

const requiredAbiLocks = [
  "native/apps/mclone-web-client/tests/render_compiler_abi_lock.rs",
  "native/apps/mclone-web-client/tests/runner_shared_abi_lock.rs",
  "native/apps/mclone-web-client/www/mclone-render-compiler-abi.js",
  "native/apps/mclone-web-client/www/mclone-runner-shared-abi.js",
];

function readSources() {
  return Object.fromEntries(
    readdirSync(wwwRoot)
      .filter((name) => name.endsWith(".ts"))
      .sort()
      .map((name) => [name, readFileSync(resolve(wwwRoot, name), "utf8")]),
  );
}

function lineCount(text) {
  if (text.length === 0) return 0;
  return text.split(/\r?\n/u).length - (text.endsWith("\n") ? 1 : 0);
}

function countOccurrences(text, needle) {
  let count = 0;
  let offset = 0;
  while (true) {
    const found = text.indexOf(needle, offset);
    if (found < 0) return count;
    count += 1;
    offset = found + needle.length;
  }
}

function inventoryFor(sources) {
  const errors = [];
  const actualModules = Object.keys(sources).sort();
  const registeredModules = Object.keys(moduleRegistry).sort();
  const unregistered = actualModules.filter((name) => !(name in moduleRegistry));
  const missing = registeredModules.filter((name) => !(name in sources));
  for (const name of unregistered) {
    errors.push(`unregistered authored TypeScript module: ${name}`);
  }
  for (const name of missing) {
    errors.push(`registered authored TypeScript module is missing: ${name}`);
  }

  const modules = [];
  const families = new Map();
  for (const name of actualModules) {
    const registration = moduleRegistry[name];
    if (!registration) continue;
    const text = sources[name];
    const lines = lineCount(text);
    const discoveredWorkerEntry = text.includes("DedicatedWorkerGlobalScope");
    const workerConstructionCount = countOccurrences(text, "new Worker(");
    if (discoveredWorkerEntry !== registration.workerEntry) {
      errors.push(
        `${name}: Worker-entry discovery was ${discoveredWorkerEntry}, expected ${registration.workerEntry}`,
      );
    }
    if (discoveredWorkerEntry && !text.includes(".onmessage")) {
      errors.push(`${name}: registered Worker entry has no onmessage handler`);
    }
    if (!registration.workerConstruction && workerConstructionCount > 0) {
      errors.push(`${name}: unapproved TypeScript Worker construction (${workerConstructionCount})`);
    }
    if (registration.workerConstruction && workerConstructionCount === 0) {
      errors.push(`${name}: registered Worker-construction site disappeared; refresh ownership`);
    }
    if (
      name !== "mclone-thread-smoke-worker.ts"
      && (text.includes("WebAssembly.Memory") || text.includes("sharedWasmMemory"))
    ) {
      errors.push(`${name}: production TypeScript introduced shared Wasm memory`);
    }
    const family = families.get(registration.family) ?? {
      family: registration.family,
      lines: 0,
      baselineLines: 0,
      modules: [],
    };
    family.lines += lines;
    family.baselineLines += registration.baselineLines;
    family.modules.push(name);
    families.set(registration.family, family);
    modules.push({
      name,
      family: registration.family,
      lines,
      baselineLines: registration.baselineLines,
      lineDelta: lines - registration.baselineLines,
      workerEntry: registration.workerEntry,
      workerConstructionCount,
      messageInterfaceCount: (text.match(/interface\s+\w*(?:Worker|Message|Request|Inbound|Outbound)\w*/gu) ?? []).length,
      responsibilities: registration.responsibilities,
    });
  }

  const debt = domainDebt.map((item) => {
    const count = countOccurrences(sources[item.file] ?? "", item.needle);
    if (count > item.maximum) {
      errors.push(
        `${item.file}: ${item.id} grew to ${count}, allowed maximum ${item.maximum} -> ${item.destination}`,
      );
    }
    return { ...item, count };
  });

  const genericTransport = sources["mclone-worker-transport.ts"] ?? "";
  for (const token of genericTransportForbidden) {
    if (genericTransport.includes(token)) {
      errors.push(
        `mclone-worker-transport.ts: generic transport contains domain token ${token}`,
      );
    }
  }

  const copies = copyFacts.map((fact) => {
    const text = readFileSync(resolve(repoRoot, fact.path), "utf8");
    const count = countOccurrences(text, fact.needle);
    if (count === 0) {
      errors.push(`${fact.path}: copy fact ${fact.id} disappeared; refresh the copy ledger`);
    }
    return { ...fact, count };
  });

  for (const path of requiredAbiLocks) {
    try {
      readFileSync(resolve(repoRoot, path), "utf8");
    } catch {
      errors.push(`required external-SAB ABI source/lock is missing: ${path}`);
    }
  }

  const totalLines = modules.reduce((sum, module) => sum + module.lines, 0);
  const baselineLines = modules.reduce((sum, module) => sum + module.baselineLines, 0);
  return {
    errors,
    report: {
      architecture: {
        productionWasmMemory: "private-per-worker",
        productionSharedTransport: "external-SharedArrayBuffer-mailboxes",
        sharedWasmMemoryProof: "mclone-thread-smoke-worker.ts only",
      },
      totals: {
        authoredTypeScriptLines: totalLines,
        baselineLines,
        lineDelta: totalLines - baselineLines,
        moduleCount: modules.length,
        workerEntryCount: modules.filter((module) => module.workerEntry).length,
        TypeScriptWorkerConstructionCount: modules.reduce(
          (sum, module) => sum + module.workerConstructionCount,
          0,
        ),
      },
      families: [...families.values()].sort((a, b) => a.family.localeCompare(b.family)),
      modules,
      domainDebt: debt,
      copyLedger: copies,
      abiLocks: requiredAbiLocks,
    },
  };
}

function runSelfTest(sources) {
  const failures = [];

  const withSyntheticWorker = {
    ...sources,
    "mclone-synthetic-worker.ts":
      "const scope = self as DedicatedWorkerGlobalScope;\nscope.onmessage = () => {};\n",
  };
  if (!inventoryFor(withSyntheticWorker).errors.some((error) => error.includes("unregistered"))) {
    failures.push("synthetic unregistered Worker module was not rejected");
  }

  const withDebtGrowth = {
    ...sources,
    "mclone-server-job-worker.ts": `${sources["mclone-server-job-worker.ts"]}\ncase "worldgen":\n`,
  };
  if (!inventoryFor(withDebtGrowth).errors.some((error) => error.includes("server-job-worldgen-switch"))) {
    failures.push("synthetic domain-debt growth was not rejected");
  }

  const withSharedHeap = {
    ...sources,
    "mclone-web-app.ts": `${sources["mclone-web-app.ts"]}\n// sharedWasmMemory\n`,
  };
  if (!inventoryFor(withSharedHeap).errors.some((error) => error.includes("shared Wasm memory"))) {
    failures.push("synthetic production shared-Wasm-memory introduction was not rejected");
  }

  const withTransportDomain = {
    ...sources,
    "mclone-worker-transport.ts": `${sources["mclone-worker-transport.ts"]}\n// worldPriority\n`,
  };
  if (!inventoryFor(withTransportDomain).errors.some((error) => error.includes("domain token"))) {
    failures.push("synthetic generic-transport domain vocabulary was not rejected");
  }

  return failures;
}

const sources = readSources();
const { errors, report } = inventoryFor(sources);
const selfTestFailures = selfTest ? runSelfTest(sources) : [];

if (json) {
  console.log(JSON.stringify({ ...report, errors, selfTestFailures }, null, 2));
} else {
  console.log("native-web Worker ownership inventory");
  console.log(
    `  ${report.totals.authoredTypeScriptLines} authored TypeScript lines across ${report.totals.moduleCount} modules`,
  );
  console.log(
    `  ${report.totals.workerEntryCount} Worker entries; ${report.totals.TypeScriptWorkerConstructionCount} TypeScript construction sites`,
  );
  console.log(
    `  memory=${report.architecture.productionWasmMemory}; transport=${report.architecture.productionSharedTransport}`,
  );
  for (const family of report.families) {
    const delta = family.lines - family.baselineLines;
    console.log(`  ${family.family}: ${family.lines} lines (${delta >= 0 ? "+" : ""}${delta})`);
  }
  console.log("  registered domain-aware debt:");
  for (const item of report.domainDebt) {
    console.log(`    ${item.id}: ${item.count}/${item.maximum} -> ${item.destination}`);
  }
  console.log(`  copy ledger: ${report.copyLedger.length} explicit copy facts present`);
  if (selfTest) {
    console.log(`  self-test: ${selfTestFailures.length === 0 ? "passed" : "failed"}`);
  }
}

for (const error of errors) {
  console.error(`web Worker ownership error: ${error}`);
}
for (const failure of selfTestFailures) {
  console.error(`web Worker ownership self-test error: ${failure}`);
}
if (errors.length > 0 || selfTestFailures.length > 0) {
  process.exitCode = 1;
}
