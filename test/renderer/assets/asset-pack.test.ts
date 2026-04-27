import { createHash } from "node:crypto";
import { afterEach, describe, expect, test, vi } from "vitest";
import {
  loadZipBytes,
  type AssetPackByteCache,
  type AssetPackByteCacheRecord,
  type AssetPackManifest,
} from "../../../src/renderer/assets/asset-pack";

class MemoryAssetPackByteCache implements AssetPackByteCache {
  public readonly deletedUrls: string[] = [];
  private readonly records = new Map<string, AssetPackByteCacheRecord>();

  public constructor(records: readonly AssetPackByteCacheRecord[] = []) {
    for (const record of records) {
      this.records.set(record.zipUrl, record);
    }
  }

  public async load(zipUrl: string): Promise<AssetPackByteCacheRecord | undefined> {
    return this.records.get(zipUrl);
  }

  public async save(record: AssetPackByteCacheRecord): Promise<void> {
    this.records.set(record.zipUrl, record);
  }

  public async delete(zipUrl: string): Promise<void> {
    this.deletedUrls.push(zipUrl);
    this.records.delete(zipUrl);
  }

  public get(zipUrl: string): AssetPackByteCacheRecord | undefined {
    return this.records.get(zipUrl);
  }
}

function bytes(values: readonly number[]): ArrayBuffer {
  const buffer = new Uint8Array(values);
  return buffer.buffer.slice(buffer.byteOffset, buffer.byteOffset + buffer.byteLength);
}

function hash(buffer: ArrayBuffer): string {
  return createHash("sha256").update(Buffer.from(buffer)).digest("hex");
}

function manifestFor(buffer: ArrayBuffer): AssetPackManifest {
  return {
    sha256: hash(buffer),
    size: buffer.byteLength,
  };
}

function responseFor(buffer: ArrayBuffer): Response {
  return new Response(buffer.slice(0), {
    status: 200,
    headers: {
      "content-length": buffer.byteLength.toString(),
    },
  });
}

describe("asset pack zip byte cache", () => {
  const originalFetch = globalThis.fetch;

  afterEach(() => {
    globalThis.fetch = originalFetch;
    vi.restoreAllMocks();
  });

  test("uses a manifest-matched cached zip without fetching", async () => {
    const zipUrl = "http://127.0.0.1/reference/minecraft-1.17.1/extracted.zip";
    const zipBytes = bytes([1, 2, 3, 4]);
    const cache = new MemoryAssetPackByteCache([{
      zipUrl,
      sha256: hash(zipBytes),
      size: zipBytes.byteLength,
      bytes: zipBytes,
    }]);
    globalThis.fetch = vi.fn(async () => {
      throw new Error("unexpected network fetch");
    }) as typeof fetch;

    const loaded = await loadZipBytes(zipUrl, manifestFor(zipBytes), undefined, cache);

    expect(new Uint8Array(loaded)).toEqual(new Uint8Array(zipBytes));
    expect(globalThis.fetch).not.toHaveBeenCalled();
  });

  test("persists a verified network download for the next loader realm", async () => {
    const zipUrl = "http://127.0.0.1/reference/minecraft-1.17.1/extracted.zip";
    const zipBytes = bytes([5, 6, 7, 8]);
    const manifest = manifestFor(zipBytes);
    const cache = new MemoryAssetPackByteCache();
    globalThis.fetch = vi.fn(async () => responseFor(zipBytes)) as typeof fetch;

    const firstLoad = await loadZipBytes(zipUrl, manifest, undefined, cache);
    const secondLoad = await loadZipBytes(zipUrl, manifest, undefined, cache);

    expect(new Uint8Array(firstLoad)).toEqual(new Uint8Array(zipBytes));
    expect(new Uint8Array(secondLoad)).toEqual(new Uint8Array(zipBytes));
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
    expect(cache.get(zipUrl)?.sha256).toBe(manifest.sha256);
  });

  test("invalidates stale cached bytes before downloading the current manifest hash", async () => {
    const zipUrl = "http://127.0.0.1/reference/minecraft-1.17.1/extracted.zip";
    const staleBytes = bytes([0, 0, 0]);
    const currentBytes = bytes([9, 10, 11]);
    const cache = new MemoryAssetPackByteCache([{
      zipUrl,
      sha256: hash(staleBytes),
      size: staleBytes.byteLength,
      bytes: staleBytes,
    }]);
    globalThis.fetch = vi.fn(async () => responseFor(currentBytes)) as typeof fetch;

    const loaded = await loadZipBytes(zipUrl, manifestFor(currentBytes), undefined, cache);

    expect(new Uint8Array(loaded)).toEqual(new Uint8Array(currentBytes));
    expect(cache.deletedUrls).toEqual([zipUrl]);
    expect(globalThis.fetch).toHaveBeenCalledWith(zipUrl, { cache: "default" });
    expect(cache.get(zipUrl)?.sha256).toBe(hash(currentBytes));
  });

  test("treats malformed cached metadata as a miss", async () => {
    const zipUrl = "http://127.0.0.1/reference/minecraft-1.17.1/extracted.zip";
    const currentBytes = bytes([12, 13, 14]);
    const cache = new MemoryAssetPackByteCache([{
      zipUrl,
      size: currentBytes.byteLength,
      bytes: currentBytes,
    } as AssetPackByteCacheRecord]);
    globalThis.fetch = vi.fn(async () => responseFor(currentBytes)) as typeof fetch;

    const loaded = await loadZipBytes(zipUrl, manifestFor(currentBytes), undefined, cache);

    expect(new Uint8Array(loaded)).toEqual(new Uint8Array(currentBytes));
    expect(cache.deletedUrls).toEqual([zipUrl]);
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
  });
});
