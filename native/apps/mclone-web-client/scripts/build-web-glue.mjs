import { spawnSync } from "node:child_process";
import { cp, readdir, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, extname, join, relative, resolve } from "node:path";

const scriptDir = dirname(fileURLToPath(import.meta.url));
export const appRoot = resolve(scriptDir, "..");
export const nativeRoot = resolve(appRoot, "../..");
export const repoRoot = resolve(nativeRoot, "..");
export const webSourceRoot = join(appRoot, "www");
export const stagedWebRoot = join(nativeRoot, "target", "mclone-web-client-www");
export const webEmitTsconfig = join(appRoot, "tsconfig.web.json");

/**
 * Builds the browser-loadable native web root from authored www/ sources.
 *
 * Static assets and residual JS are copied as-is. Authored TypeScript is emitted to matching
 * .js filenames by tsconfig.web.json so the browser URL graph stays stable.
 *
 * @param {{ log?: boolean }} [options]
 */
export async function buildWebGlue(options = {}) {
  const log = options.log ?? true;
  if (log) {
    console.log(`staging native web glue: ${relative(repoRoot, stagedWebRoot)}`);
  }

  await rm(stagedWebRoot, { recursive: true, force: true });
  await cp(webSourceRoot, stagedWebRoot, {
    recursive: true,
    filter: (source) => extname(source) !== ".ts",
  });

  const result = spawnSync(
    "pnpm",
    ["exec", "tsc", "-p", webEmitTsconfig],
    {
      cwd: repoRoot,
      env: process.env,
      stdio: "inherit",
    },
  );
  if (result.status !== 0) {
    throw new Error(`native web glue TypeScript emit failed with status ${result.status ?? "unknown"}`);
  }

  const leakedTs = await findFiles(stagedWebRoot, (path) => extname(path) === ".ts");
  if (leakedTs.length > 0) {
    throw new Error(
      `native web glue staging leaked TypeScript sources:\n${leakedTs
        .map((path) => `  ${relative(stagedWebRoot, path)}`)
        .join("\n")}`,
    );
  }

  return stagedWebRoot;
}

/**
 * @param {string} root
 * @param {(path: string) => boolean} predicate
 * @returns {Promise<string[]>}
 */
async function findFiles(root, predicate) {
  /** @type {string[]} */
  const matches = [];
  const entries = await readdir(root, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      matches.push(...await findFiles(path, predicate));
    } else if (entry.isFile() && predicate(path)) {
      matches.push(path);
    }
  }
  return matches;
}

const invokedDirectly = process.argv[1]
  ? resolve(process.argv[1]) === fileURLToPath(import.meta.url)
  : false;

if (invokedDirectly) {
  buildWebGlue().catch((error) => {
    console.error(error instanceof Error ? error.stack ?? error.message : String(error));
    process.exitCode = 1;
  });
}
