import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { chromium } from "@playwright/test";
import { createServer } from "vite";
import {
  chooseDefaultClip,
  formatFigureLabel,
  parseAnimalCatalog,
  type AnimalCatalogClip,
  type AnimalCatalogDocument,
  type AnimalCatalogFigure,
} from "./catalog-model";
import { discoverCanonicalFigureSources } from "./discover-figures";
import { assertBoxOnlyFigure } from "./dsl";
import { loadFigureJsonDocument } from "./load";
import { clipDuration } from "./scene";
import { assetLabRoot } from "./vite-figure-path";

export interface BuildWebCatalogOptions {
  outRoot?: string;
  sourcePaths?: string[];
  thumbnails?: boolean;
}

interface PreparedCatalogFigure {
  entry: AnimalCatalogFigure;
  json: string;
}

const defaultOutRoot = path.join(assetLabRoot, "dist", "catalog-public");

export async function buildWebCatalog(
  options: BuildWebCatalogOptions = {},
): Promise<AnimalCatalogDocument> {
  const outRoot = path.resolve(options.outRoot ?? defaultOutRoot);
  assertSafeOutputRoot(outRoot);
  const sourcePaths = options.sourcePaths ?? await discoverCanonicalFigureSources();
  if (sourcePaths.length === 0) {
    throw new Error("The canonical Asset Lab catalogue has no figure sources");
  }

  const prepared: PreparedCatalogFigure[] = [];
  const names = new Set<string>();
  for (const sourcePath of sourcePaths) {
    const document = await loadFigureJsonDocument(sourcePath);
    const asset = document.asset;
    assertBoxOnlyFigure(asset);
    const directoryName = path.basename(path.dirname(sourcePath));
    if (asset.name !== directoryName) {
      throw new Error(
        `Canonical source '${sourcePath}' exports '${asset.name}', expected directory name '${directoryName}'`,
      );
    }
    if (!/^[a-z0-9][a-z0-9_-]*$/u.test(asset.name)) {
      throw new Error(`Figure '${asset.name}' is not safe for a catalogue artifact path`);
    }
    if (names.has(asset.name)) {
      throw new Error(`Duplicate canonical figure name '${asset.name}'`);
    }
    names.add(asset.name);

    const clips = Object.entries(asset.clips)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([name, clip]) => catalogClip(name, clip));
    if (clips.length === 0) {
      throw new Error(`Canonical figure '${asset.name}' has no animation clips`);
    }
    const semanticSha256 = sha256(document.json);
    prepared.push({
      entry: {
        clipCount: clips.length,
        clips,
        defaultClip: chooseDefaultClip(clips.map((clip) => clip.name)),
        jsonPath: `catalog/figures/${asset.name}.figure.json`,
        label: formatFigureLabel(asset.name),
        materialCount: Object.keys(asset.materials).length,
        name: asset.name,
        partCount: asset.parts.length,
        semanticBytes: Buffer.byteLength(document.json),
        semanticSha256,
        textureCount: Object.keys(asset.textures).length,
        thumbnailPath: `catalog/thumbnails/${asset.name}.png`,
      },
      json: document.json,
    });
  }
  prepared.sort((left, right) => left.entry.label.localeCompare(right.entry.label));

  const catalogSha256 = sha256(
    prepared.map(({ entry }) => `${entry.name}\0${entry.semanticSha256}\n`).join(""),
  );
  const catalog: AnimalCatalogDocument = {
    catalogSha256,
    figures: prepared.map(({ entry }) => entry),
    schemaVersion: 1,
    summary: {
      canonicalFigures: prepared.length,
      clips: prepared.reduce((total, { entry }) => total + entry.clipCount, 0),
      parts: prepared.reduce((total, { entry }) => total + entry.partCount, 0),
    },
  };
  parseAnimalCatalog(catalog, "generated catalogue");

  await fs.rm(outRoot, { force: true, recursive: true });
  await fs.mkdir(path.join(outRoot, "catalog", "figures"), { recursive: true });
  await fs.mkdir(path.join(outRoot, "catalog", "thumbnails"), { recursive: true });
  await Promise.all(prepared.map(({ entry, json }) =>
    fs.writeFile(path.join(outRoot, entry.jsonPath), json, "utf8")
  ));
  await fs.writeFile(
    path.join(outRoot, "catalog", "catalog.v1.json"),
    `${JSON.stringify(catalog, null, 2)}\n`,
    "utf8",
  );

  if (options.thumbnails !== false) {
    await renderThumbnails(prepared, outRoot);
  }
  return catalog;
}

function catalogClip(name: string, clip: Parameters<typeof clipDuration>[0] & object): AnimalCatalogClip {
  const durationSeconds = clipDuration(clip);
  if (durationSeconds <= 0) {
    throw new Error(`Animation clip '${name}' has no positive duration`);
  }
  return {
    durationSeconds,
    ...(clip.fps === undefined ? {} : { fps: clip.fps }),
    ...(clip.locomotion === undefined ? {} : { locomotionKind: clip.locomotion.kind }),
    loop: clip.loop === true,
    name,
  };
}

async function renderThumbnails(
  prepared: readonly PreparedCatalogFigure[],
  outRoot: string,
): Promise<void> {
  const server = await createServer({
    root: assetLabRoot,
    logLevel: "error",
    server: {
      host: "127.0.0.1",
      hmr: false,
      port: 0,
      strictPort: false,
    },
  });
  await server.listen();
  const url = server.resolvedUrls?.local[0];
  if (!url) {
    await server.close();
    throw new Error("Vite did not report a local URL for catalogue thumbnails");
  }

  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({
      deviceScaleFactor: 1,
      viewport: { width: 360, height: 300 },
    });
    const browserErrors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") {
        browserErrors.push(message.text());
      }
    });
    page.on("pageerror", (error) => browserErrors.push(error.stack ?? error.message));
    await page.goto(`${url}thumbnail.html`, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.assetLabThumbnailReady === true);

    for (const { entry, json } of prepared) {
      await page.evaluate(
        async ({ defaultClip, figureJson, sourceLabel }) => {
          if (!window.assetLabRenderThumbnail) {
            throw new Error("Thumbnail page did not expose its renderer");
          }
          await window.assetLabRenderThumbnail(figureJson, sourceLabel, defaultClip);
        },
        { defaultClip: entry.defaultClip, figureJson: json, sourceLabel: entry.name },
      );
      await page.locator("canvas").screenshot({
        path: path.join(outRoot, entry.thumbnailPath),
      });
    }
    if (browserErrors.length > 0) {
      throw new Error(`Catalogue thumbnail browser errors:\n${browserErrors.join("\n")}`);
    }
  } finally {
    await browser.close();
    await server.close();
  }
}

function assertSafeOutputRoot(outRoot: string): void {
  const allowedParents = [path.join(assetLabRoot, "dist"), os.tmpdir()];
  if (!allowedParents.some((parent) => isStrictChild(parent, outRoot))) {
    throw new Error(`Refusing to replace unsafe catalogue output root '${outRoot}'`);
  }
}

function isStrictChild(parent: string, candidate: string): boolean {
  const relative = path.relative(path.resolve(parent), path.resolve(candidate));
  return relative !== "" && !relative.startsWith("..") && !path.isAbsolute(relative);
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

async function main(): Promise<void> {
  const catalog = await buildWebCatalog();
  console.log(
    `Built ${catalog.summary.canonicalFigures} canonical figures and ${catalog.summary.clips} clips under ${defaultOutRoot}`,
  );
}

const entryPath = process.argv[1] === undefined ? undefined : pathToFileURL(path.resolve(process.argv[1])).href;
if (entryPath === import.meta.url) {
  await main();
}
