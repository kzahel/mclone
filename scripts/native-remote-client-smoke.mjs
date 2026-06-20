import { spawn } from "node:child_process";
import { mkdir, readFile, stat } from "node:fs/promises";
import { dirname } from "node:path";

const screenshotPath = process.env.MCLONE_NATIVE_REMOTE_SMOKE_SCREENSHOT
  ?? "/tmp/mclone-native-remote-client-smoke.png";
const width = Number.parseInt(process.env.MCLONE_NATIVE_REMOTE_SMOKE_WIDTH ?? "960", 10);
const height = Number.parseInt(process.env.MCLONE_NATIVE_REMOTE_SMOKE_HEIGHT ?? "540", 10);
const renderDistance = Number.parseInt(process.env.MCLONE_NATIVE_REMOTE_SMOKE_RENDER_DISTANCE ?? "2", 10);
const timeoutMs = Number.parseInt(process.env.MCLONE_NATIVE_REMOTE_SMOKE_TIMEOUT_MS ?? "120000", 10);

run().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});

async function run() {
  await mkdir(dirname(screenshotPath), { recursive: true });

  const server = spawnCargo([
    "run",
    "--manifest-path",
    "native/Cargo.toml",
    "-p",
    "mclone-dedicated-server",
    "--",
    "--listen",
    "127.0.0.1:0",
    "--serve-once",
  ]);
  const serverLog = captureProcessLog(server, "server");
  let serverExited = false;
  const serverExit = observeExit(server);
  serverExit.then(() => {
    serverExited = true;
  });

  try {
    const remoteAddr = await waitForServerAddress(server, serverLog, timeoutMs);
    const client = spawnCargo([
      "run",
      "--manifest-path",
      "native/Cargo.toml",
      "-p",
      "mclone-native-client",
      "--",
      "--screenshot",
      screenshotPath,
      "--width",
      String(width),
      "--height",
      String(height),
      "--render-distance",
      String(renderDistance),
      "--remote-addr",
      remoteAddr,
      "--disable-lighting",
      "--screenshot-debug-pane",
      "true",
      "--screenshot-scripted-interaction",
      "true",
    ]);
    const clientLog = captureProcessLog(client, "client");
    await waitForExit(client, "native remote client", timeoutMs, clientLog);
    const clientReport = parseClientScreenshotReport(clientLog.stdout);
    await assertPngScreenshot(screenshotPath, width, height, clientReport);
    const serverExitResult = await withTimeout(
      serverExit,
      timeoutMs,
      "timed out waiting for dedicated smoke server to exit",
      serverLog,
    );
    assertExitResult(serverExitResult, "dedicated smoke server", serverLog);

    console.log(JSON.stringify({
      smoke: "native_remote_dedicated_client",
      remoteAddr,
      screenshotPath,
      width,
      height,
      renderDistance,
      clientReport,
    }, null, 2));
  } finally {
    if (!serverExited) {
      server.kill("SIGTERM");
    }
  }
}

function observeExit(child) {
  return new Promise((resolve) => {
    child.once("exit", (code, signal) => {
      resolve({ code, signal });
    });
  });
}

function spawnCargo(args) {
  return spawn("cargo", args, {
    cwd: process.cwd(),
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function captureProcessLog(child, label) {
  const log = { stdout: "", stderr: "" };
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    log.stdout += chunk;
    process.stdout.write(prefixLines(label, chunk));
  });
  child.stderr.on("data", (chunk) => {
    log.stderr += chunk;
    process.stderr.write(prefixLines(label, chunk));
  });
  return log;
}

function prefixLines(label, chunk) {
  return String(chunk)
    .split(/(?<=\n)/)
    .map((line) => line.length === 0 ? line : `[${label}] ${line}`)
    .join("");
}

function waitForServerAddress(server, log, timeout) {
  const ready = /mclone dedicated server listening on ([^\s]+) seed=\S+ protocol \d+/;
  const existing = ready.exec(log.stdout);
  if (existing) {
    return Promise.resolve(existing[1]);
  }

  return withTimeout(new Promise((resolve, reject) => {
    const onData = () => {
      const match = ready.exec(log.stdout);
      if (match) {
        cleanup();
        resolve(match[1]);
      }
    };
    const onExit = (code, signal) => {
      cleanup();
      reject(new Error(
        `dedicated server exited before reporting listen address: code=${String(code)} signal=${String(signal)}\n${formatProcessLog(log)}`,
      ));
    };
    const cleanup = () => {
      server.stdout.off("data", onData);
      server.off("exit", onExit);
    };

    server.stdout.on("data", onData);
    server.once("exit", onExit);
    onData();
  }), timeout, "timed out waiting for dedicated server listen address", log);
}

function waitForExit(child, label, timeout, log) {
  return withTimeout(new Promise((resolve, reject) => {
    child.once("exit", (code, signal) => {
      try {
        assertExitResult({ code, signal }, label, log);
        resolve();
      } catch (error) {
        reject(error);
      }
    });
  }), timeout, `timed out waiting for ${label} to exit`, log);
}

function assertExitResult(result, label, log) {
  if (result.code !== 0) {
    throw new Error(
      `${label} failed: code=${String(result.code)} signal=${String(result.signal)}\n${formatProcessLog(log)}`,
    );
  }
}

function withTimeout(promise, timeout, message, log) {
  let timer;
  const timeoutPromise = new Promise((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`${message}\n${formatProcessLog(log)}`));
    }, timeout);
  });
  return Promise.race([promise, timeoutPromise]).finally(() => clearTimeout(timer));
}

function parseClientScreenshotReport(stdout) {
  const report = /headless full-frame screenshot saved to (.+) \((\d+)x(\d+), (\d+) bytes, (\d+) sections, (\d+) drawn sections, (\d+) GUI commands\)/.exec(stdout);
  if (!report) {
    throw new Error(`native client did not print screenshot report:\n${stdout}`);
  }
  const parsed = {
    path: report[1],
    width: Number.parseInt(report[2], 10),
    height: Number.parseInt(report[3], 10),
    byteLen: Number.parseInt(report[4], 10),
    sectionCount: Number.parseInt(report[5], 10),
    drawnSectionCount: Number.parseInt(report[6], 10),
    guiCommandCount: Number.parseInt(report[7], 10),
  };
  if (parsed.sectionCount <= 0 || parsed.drawnSectionCount <= 0) {
    throw new Error(`remote screenshot rendered no chunk sections:\n${JSON.stringify(parsed, null, 2)}`);
  }
  return parsed;
}

async function assertPngScreenshot(path, expectedWidth, expectedHeight, report) {
  const info = await stat(path);
  if (info.size < 1024) {
    throw new Error(`screenshot is unexpectedly small: ${info.size} bytes`);
  }
  if (report.byteLen < expectedWidth * expectedHeight * 4) {
    throw new Error(`screenshot report byte length is unexpectedly small: ${report.byteLen}`);
  }

  const bytes = await readFile(path);
  const png = parsePngHeader(bytes);
  if (png.width !== expectedWidth || png.height !== expectedHeight) {
    throw new Error(
      `screenshot dimensions mismatch: got ${png.width}x${png.height}, expected ${expectedWidth}x${expectedHeight}`,
    );
  }
}

function parsePngHeader(bytes) {
  const signature = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  for (let index = 0; index < signature.length; index += 1) {
    if (bytes[index] !== signature[index]) {
      throw new Error("screenshot is not a PNG");
    }
  }
  const type = bytes.subarray(12, 16).toString("ascii");
  if (type !== "IHDR") {
    throw new Error(`PNG first chunk is ${type}, expected IHDR`);
  }
  return {
    width: bytes.readUInt32BE(16),
    height: bytes.readUInt32BE(20),
  };
}

function formatProcessLog(log) {
  const stdout = tail(log.stdout);
  const stderr = tail(log.stderr);
  return `--- stdout ---\n${stdout}\n--- stderr ---\n${stderr}`;
}

function tail(text) {
  const lines = String(text).trimEnd().split("\n");
  return lines.slice(-80).join("\n");
}
