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

export interface AssetPackManifest {
  readonly version?: string;
  readonly zip?: string;
  readonly sha256?: string;
  readonly size?: number;
  readonly fileCount?: number;
}

export interface AssetPackByteCacheRecord {
  readonly zipUrl: string;
  readonly sha256: string;
  readonly size?: number;
  readonly bytes: Blob | ArrayBuffer;
  readonly savedAtMs?: number;
}

export interface AssetPackByteCache {
  load(zipUrl: string): Promise<AssetPackByteCacheRecord | undefined>;

  save(record: AssetPackByteCacheRecord): Promise<void>;

  delete(zipUrl: string): Promise<void>;
}

const ASSET_PACK_CACHE_DATABASE_NAME = "mclone-asset-pack-cache";
const ASSET_PACK_CACHE_DATABASE_VERSION = 1;
const ASSET_PACK_CACHE_STORE = "assetPacks";

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
let browserAssetPackByteCache: AssetPackByteCache | undefined;
let warnedAssetPackByteCache = false;

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

function waitForTransaction(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"));
    transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
  });
}

function requestToPromise<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

function openAssetPackCacheDatabase(factory: IDBFactory): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = factory.open(ASSET_PACK_CACHE_DATABASE_NAME, ASSET_PACK_CACHE_DATABASE_VERSION);
    request.onerror = () => reject(request.error ?? new Error(`Unable to open ${ASSET_PACK_CACHE_DATABASE_NAME}`));
    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(ASSET_PACK_CACHE_STORE)) {
        database.createObjectStore(ASSET_PACK_CACHE_STORE, { keyPath: "zipUrl" });
      }
    };
    request.onsuccess = () => resolve(request.result);
  });
}

class IndexedDbAssetPackByteCache implements AssetPackByteCache {
  private databasePromise: Promise<IDBDatabase> | undefined;

  public constructor(private readonly factory: IDBFactory) {}

  public async load(zipUrl: string): Promise<AssetPackByteCacheRecord | undefined> {
    const database = await this.getDatabase();
    const transaction = database.transaction(ASSET_PACK_CACHE_STORE, "readonly");
    const record = await requestToPromise(transaction.objectStore(ASSET_PACK_CACHE_STORE).get(zipUrl)) as AssetPackByteCacheRecord | undefined;
    await waitForTransaction(transaction);
    return record;
  }

  public async save(record: AssetPackByteCacheRecord): Promise<void> {
    const database = await this.getDatabase();
    const transaction = database.transaction(ASSET_PACK_CACHE_STORE, "readwrite");
    transaction.objectStore(ASSET_PACK_CACHE_STORE).put({
      ...record,
      bytes: record.bytes instanceof Blob ? record.bytes : new Blob([record.bytes], { type: "application/zip" }),
      savedAtMs: record.savedAtMs ?? Date.now(),
    } satisfies AssetPackByteCacheRecord);
    await waitForTransaction(transaction);
  }

  public async delete(zipUrl: string): Promise<void> {
    const database = await this.getDatabase();
    const transaction = database.transaction(ASSET_PACK_CACHE_STORE, "readwrite");
    transaction.objectStore(ASSET_PACK_CACHE_STORE).delete(zipUrl);
    await waitForTransaction(transaction);
  }

  private getDatabase(): Promise<IDBDatabase> {
    this.databasePromise ??= openAssetPackCacheDatabase(this.factory);
    return this.databasePromise;
  }
}

function getBrowserAssetPackByteCache(): AssetPackByteCache | undefined {
  if (typeof indexedDB === "undefined") {
    return undefined;
  }

  browserAssetPackByteCache ??= new IndexedDbAssetPackByteCache(indexedDB);
  return browserAssetPackByteCache;
}

function warnAssetPackByteCache(action: string, error: unknown): void {
  if (warnedAssetPackByteCache) {
    return;
  }

  warnedAssetPackByteCache = true;
  console.warn(`Unable to ${action} persistent asset pack cache; falling back to HTTP cache`, error);
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

function cacheableManifestHash(manifest: AssetPackManifest): string | undefined {
  const hash = manifest.sha256?.trim().toLowerCase();
  return hash === "" ? undefined : hash;
}

async function cachedRecordBytes(record: AssetPackByteCacheRecord): Promise<ArrayBuffer> {
  if (record.bytes instanceof ArrayBuffer) {
    return record.bytes.slice(0);
  }

  return record.bytes.arrayBuffer();
}

async function deleteCachedZipBytes(cache: AssetPackByteCache, zipUrl: string): Promise<void> {
  try {
    await cache.delete(zipUrl);
  } catch (error) {
    warnAssetPackByteCache("delete from", error);
  }
}

async function loadCachedZipBytes(
  zipUrl: string,
  manifest: AssetPackManifest,
  cache: AssetPackByteCache,
  onProgress?: LoadingProgressSink,
): Promise<ArrayBuffer | undefined> {
  const manifestHash = cacheableManifestHash(manifest);
  if (manifestHash === undefined) {
    return undefined;
  }

  onProgress?.({ stage: "Checking cached asset pack", fraction: 0 });

  let record: AssetPackByteCacheRecord | undefined;
  try {
    record = await cache.load(zipUrl);
  } catch (error) {
    warnAssetPackByteCache("read from", error);
    return undefined;
  }

  if (record === undefined) {
    return undefined;
  }

  const recordHash = typeof record.sha256 === "string" ? record.sha256.toLowerCase() : undefined;
  if (recordHash !== manifestHash || (manifest.size !== undefined && record.size !== manifest.size)) {
    await deleteCachedZipBytes(cache, zipUrl);
    return undefined;
  }

  try {
    const bytes = await cachedRecordBytes(record);
    if (await verifyZipBytes(bytes, manifest, zipUrl)) {
      onProgress?.({
        stage: "Loaded cached asset pack",
        current: bytes.byteLength,
        total: bytes.byteLength,
        fraction: 1,
      });
      return bytes;
    }
  } catch {
    // Fall through and replace the corrupt cache entry from the network.
  }

  await deleteCachedZipBytes(cache, zipUrl);
  return undefined;
}

async function saveCachedZipBytes(
  zipUrl: string,
  manifest: AssetPackManifest,
  bytes: ArrayBuffer,
  cache: AssetPackByteCache,
): Promise<void> {
  const manifestHash = cacheableManifestHash(manifest);
  if (manifestHash === undefined) {
    return;
  }

  try {
    await cache.save({
      zipUrl,
      sha256: manifestHash,
      size: manifest.size,
      bytes: new Blob([bytes.slice(0)], { type: "application/zip" }),
      savedAtMs: Date.now(),
    });
  } catch (error) {
    warnAssetPackByteCache("write to", error);
  }
}

export async function loadZipBytes(
  zipUrl: string,
  manifest: AssetPackManifest,
  onProgress?: LoadingProgressSink,
  cache = getBrowserAssetPackByteCache(),
): Promise<ArrayBuffer> {
  if (cache !== undefined) {
    const cachedBuffer = await loadCachedZipBytes(zipUrl, manifest, cache, onProgress);
    if (cachedBuffer !== undefined) {
      return cachedBuffer;
    }
  }

  const firstBuffer = await fetchZipBytes(zipUrl, "default", onProgress);
  if (await verifyZipBytes(firstBuffer, manifest, zipUrl)) {
    if (cache !== undefined) {
      await saveCachedZipBytes(zipUrl, manifest, firstBuffer, cache);
    }
    return firstBuffer;
  }

  const reloadedBuffer = await fetchZipBytes(zipUrl, "reload", onProgress);
  if (await verifyZipBytes(reloadedBuffer, manifest, zipUrl)) {
    if (cache !== undefined) {
      await saveCachedZipBytes(zipUrl, manifest, reloadedBuffer, cache);
    }
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
