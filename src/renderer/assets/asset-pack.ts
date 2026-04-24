import { unzip, type ZipEntry } from "unzipit";
import type { LoadingProgressSink } from "../loading-progress";

export const DEFAULT_ASSET_PACK_ZIP_URL = "/reference/minecraft-1.17.1/extracted.zip";
export const DEFAULT_ASSET_PACK_MANIFEST_URL = `${DEFAULT_ASSET_PACK_ZIP_URL}.json`;

export interface AssetPack {
  has(path: string): boolean;

  readText(path: string): Promise<string | undefined>;

  readBytes(path: string): Promise<Uint8Array | undefined>;

  readBlob(path: string, contentType?: string): Promise<Blob | undefined>;
}

interface AssetPackManifest {
  readonly version?: string;
  readonly zip?: string;
  readonly sha256?: string;
  readonly size?: number;
  readonly fileCount?: number;
}

class ZipAssetPack implements AssetPack {
  public constructor(private readonly entries: Readonly<Record<string, ZipEntry>>) {}

  public has(path: string): boolean {
    return this.getEntry(path) !== undefined;
  }

  public async readText(path: string): Promise<string | undefined> {
    return this.getEntry(path)?.text();
  }

  public async readBytes(path: string): Promise<Uint8Array | undefined> {
    const buffer = await this.getEntry(path)?.arrayBuffer();
    return buffer === undefined ? undefined : new Uint8Array(buffer);
  }

  public async readBlob(path: string, contentType?: string): Promise<Blob | undefined> {
    return this.getEntry(path)?.blob(contentType);
  }

  private getEntry(path: string): ZipEntry | undefined {
    const normalized = normalizeAssetPath(path);
    return this.entries[normalized] ?? this.entries[`/${normalized}`];
  }
}

let browserAssetPackPromise: Promise<AssetPack> | undefined;

function normalizeAssetPath(path: string): string {
  return path.replace(/^\/+/, "");
}

function resolveUrl(value: string, baseUrl: string): string {
  const base = typeof globalThis.location?.href === "string" ? globalThis.location.href : "http://127.0.0.1/";
  return new URL(value, new URL(baseUrl, base)).toString();
}

function bytesToHex(bytes: ArrayBuffer): string {
  return [...new Uint8Array(bytes)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

async function sha256Hex(buffer: ArrayBuffer): Promise<string> {
  if (!globalThis.crypto?.subtle) {
    throw new Error("crypto.subtle is unavailable; cannot verify asset pack hash");
  }

  return bytesToHex(await globalThis.crypto.subtle.digest("SHA-256", buffer));
}

async function fetchJsonIfPresent(url: string): Promise<AssetPackManifest | undefined> {
  const response = await fetch(url, { cache: "no-cache" });
  if (response.status === 404) {
    return undefined;
  }

  if (!response.ok) {
    throw new Error(`Unable to load asset pack manifest ${url}: ${response.status} ${response.statusText}`);
  }

  return response.json() as Promise<AssetPackManifest>;
}

async function readResponseBytes(
  response: Response,
  onProgress?: LoadingProgressSink,
): Promise<ArrayBuffer> {
  const total = Number.parseInt(response.headers.get("content-length") ?? "", 10);
  if (!response.body || !Number.isFinite(total) || total <= 0) {
    const buffer = await response.arrayBuffer();
    onProgress?.({
      stage: "Downloading asset pack",
      current: buffer.byteLength,
      total: buffer.byteLength,
    });
    return buffer;
  }

  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let received = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }

    chunks.push(value);
    received += value.byteLength;
    onProgress?.({
      stage: "Downloading asset pack",
      current: received,
      total,
      detail: `${(received / (1024 * 1024)).toFixed(1)} / ${(total / (1024 * 1024)).toFixed(1)} MiB`,
    });
  }

  const bytes = new Uint8Array(received);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }

  return bytes.buffer;
}

async function fetchZipBytes(url: string, cache: RequestCache, onProgress?: LoadingProgressSink): Promise<ArrayBuffer> {
  const response = await fetch(url, { cache });
  if (!response.ok) {
    throw new Error(`Unable to load asset pack ${url}: ${response.status} ${response.statusText}`);
  }

  return readResponseBytes(response, onProgress);
}

async function verifyZipBytes(buffer: ArrayBuffer, manifest: AssetPackManifest, zipUrl: string): Promise<boolean> {
  if (manifest.size !== undefined && buffer.byteLength !== manifest.size) {
    return false;
  }

  if (manifest.sha256 !== undefined) {
    const actualHash = await sha256Hex(buffer);
    if (actualHash.toLowerCase() !== manifest.sha256.toLowerCase()) {
      return false;
    }
  }

  if (buffer.byteLength === 0) {
    throw new Error(`Asset pack ${zipUrl} was empty`);
  }

  return true;
}

async function loadZipBytes(zipUrl: string, manifest: AssetPackManifest, onProgress?: LoadingProgressSink): Promise<ArrayBuffer> {
  const firstBuffer = await fetchZipBytes(zipUrl, "default", onProgress);
  if (await verifyZipBytes(firstBuffer, manifest, zipUrl)) {
    return firstBuffer;
  }

  const reloadedBuffer = await fetchZipBytes(zipUrl, "reload", onProgress);
  if (await verifyZipBytes(reloadedBuffer, manifest, zipUrl)) {
    return reloadedBuffer;
  }

  throw new Error(`Asset pack ${zipUrl} failed SHA-256/size verification after cache reload`);
}

export async function loadBrowserAssetPack(
  manifestUrl = DEFAULT_ASSET_PACK_MANIFEST_URL,
  onProgress?: LoadingProgressSink,
): Promise<AssetPack> {
  onProgress?.({ stage: "Loading asset pack manifest", fraction: 0 });
  const manifest = await fetchJsonIfPresent(manifestUrl) ?? {};
  const zipUrl = resolveUrl(manifest.zip ?? DEFAULT_ASSET_PACK_ZIP_URL, manifestUrl);
  const bytes = await loadZipBytes(zipUrl, manifest, onProgress);
  onProgress?.({ stage: "Indexing asset pack", fraction: 0.9 });
  const { entries } = await unzip(new Blob([bytes], { type: "application/zip" }));
  onProgress?.({
    stage: "Indexed asset pack",
    current: Object.keys(entries).length,
    total: Object.keys(entries).length,
    fraction: 1,
  });
  return new ZipAssetPack(entries);
}

export function getBrowserAssetPack(onProgress?: LoadingProgressSink): Promise<AssetPack> {
  browserAssetPackPromise ??= loadBrowserAssetPack(DEFAULT_ASSET_PACK_MANIFEST_URL, onProgress);
  return browserAssetPackPromise;
}
