import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { chromium } from "@playwright/test";
import { createServer } from "vite";
import {
  parseStructureCatalog,
  parseStructureCatalogEntry,
  type StructureCatalogDocument,
  type StructureCatalogEntry,
} from "./catalog-model";
import { FIRST_PARTY_STRUCTURES } from "./first-party-structures";
import { repositoryRoot, structureLabRoot } from "./paths";

export interface BuildWebCatalogOptions {
  outRoot?: string;
  thumbnails?: boolean;
}

const previewRoot = path.join(repositoryRoot, "assets", "mclone", "structure-previews");
const defaultOutRoot = path.join(structureLabRoot, "dist", "catalog-public");

export async function buildWebCatalog(
  options: BuildWebCatalogOptions = {},
): Promise<StructureCatalogDocument> {
  const outRoot = path.resolve(options.outRoot ?? defaultOutRoot);
  assertSafeOutputRoot(outRoot);
  const entries: StructureCatalogEntry[] = [];
  for (const structure of FIRST_PARTY_STRUCTURES) {
    const receiptPath = path.join(
      previewRoot,
      "receipts",
      `${structure.runtimeStructureId}.preview.json`,
    );
    const receipt = parseStructureCatalogEntry(
      JSON.parse(await fs.readFile(receiptPath, "utf8")) as unknown,
      receiptPath,
    );
    if (receipt.structureId !== structure.runtimeStructureId) {
      throw new Error(`Preview '${receiptPath}' has unexpected id '${receipt.structureId}'`);
    }
    entries.push({
      ...receipt,
      meshPath: `catalog/${receipt.artifacts.meshPath}`,
      atlasPath: `catalog/${receipt.artifacts.atlasPath}`,
      receiptPath: `catalog/receipts/${receipt.structureId}.preview.json`,
      thumbnailPath: `catalog/thumbnails/${receipt.structureId}.png`,
      runtimeStatus: "parity-canary",
    });
  }
  entries.sort((left, right) => left.label.localeCompare(right.label));
  const catalogSha256 = sha256(entries.map((entry) => [
    entry.structureId,
    entry.source.semanticSha256,
    entry.artifacts.meshSha256,
    entry.artifacts.atlasSha256,
    entry.runtimeStatus,
  ].join("\0")).join("\n"));
  const catalog: StructureCatalogDocument = {
    schemaVersion: 1,
    catalogSha256,
    structures: entries,
    summary: {
      structures: entries.length,
      families: new Set(entries.flatMap((entry) => entry.family ? [entry.family.id] : [])).size,
      blocks: entries.reduce((total, entry) => total + entry.placedBlockCount, 0),
      runtimePromoted: entries.filter((entry) => entry.runtimeStatus === "promoted").length,
    },
  };
  parseStructureCatalog(catalog, "generated Structure Lab catalog");

  await fs.rm(outRoot, { force: true, recursive: true });
  await fs.mkdir(path.join(outRoot, "catalog"), { recursive: true });
  await fs.cp(previewRoot, path.join(outRoot, "catalog"), { recursive: true });
  await fs.mkdir(path.join(outRoot, "catalog", "thumbnails"), { recursive: true });
  await fs.writeFile(
    path.join(outRoot, "catalog", "catalog.v1.json"),
    `${JSON.stringify(catalog, null, 2)}\n`,
    "utf8",
  );
  if (options.thumbnails !== false) {
    await renderThumbnails(entries, outRoot);
  }
  return catalog;
}

async function renderThumbnails(
  entries: readonly StructureCatalogEntry[],
  outRoot: string,
): Promise<void> {
  const webRoot = path.join(structureLabRoot, "src", "web");
  const server = await createServer({
    root: webRoot,
    publicDir: outRoot,
    logLevel: "error",
    server: { host: "127.0.0.1", hmr: false, port: 0, strictPort: false },
  });
  await server.listen();
  const url = server.resolvedUrls?.local[0];
  if (!url) {
    await server.close();
    throw new Error("Vite did not report a URL for Structure Lab thumbnails");
  }
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ deviceScaleFactor: 1, viewport: { width: 420, height: 300 } });
    const errors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    page.on("pageerror", (error) => errors.push(error.stack ?? error.message));
    await page.goto(`${url}thumbnail.html`, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.structureLabThumbnailReady === true);
    for (const entry of entries) {
      await page.evaluate(async (catalogEntry) => {
        if (!window.structureLabRenderThumbnail) {
          throw new Error("Structure Lab thumbnail renderer is unavailable");
        }
        await window.structureLabRenderThumbnail(catalogEntry);
      }, entry);
      await page.locator("canvas").screenshot({ path: path.join(outRoot, entry.thumbnailPath) });
    }
    if (errors.length > 0) {
      throw new Error(`Structure Lab thumbnail errors:\n${errors.join("\n")}`);
    }
  } finally {
    await browser.close();
    await server.close();
  }
}

function assertSafeOutputRoot(outRoot: string): void {
  const allowed = [path.join(structureLabRoot, "dist"), os.tmpdir()];
  if (!allowed.some((parent) => isStrictChild(parent, outRoot))) {
    throw new Error(`Refusing to replace unsafe Structure Lab output '${outRoot}'`);
  }
}

function isStrictChild(parent: string, candidate: string): boolean {
  const relative = path.relative(path.resolve(parent), path.resolve(candidate));
  return relative !== "" && !relative.startsWith("..") && !path.isAbsolute(relative);
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

const entryPath = process.argv[1] === undefined
  ? undefined
  : pathToFileURL(path.resolve(process.argv[1])).href;
if (entryPath === import.meta.url) {
  const catalog = await buildWebCatalog();
  console.log(`Built ${catalog.summary.structures} Structure Lab catalog entries`);
}
