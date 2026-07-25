import type { CanonicalWorkerResult } from "./canonical-worker-protocol";

export const CANONICAL_RAW_CACHE_MAX_CHUNKS = 1_024;

export class CanonicalRawCache {
  readonly #entries = new Map<string, CanonicalWorkerResult>();
  #rawBytes = 0;

  clear(): void {
    this.#entries.clear();
    this.#rawBytes = 0;
  }

  get(key: string): CanonicalWorkerResult | undefined {
    const result = this.#entries.get(key);
    if (!result) {
      return undefined;
    }
    this.#entries.delete(key);
    this.#entries.set(key, result);
    return result;
  }

  set(key: string, result: CanonicalWorkerResult): void {
    const previous = this.#entries.get(key);
    if (previous) {
      this.#rawBytes -= canonicalResultRawBytes(previous);
      this.#entries.delete(key);
    }
    this.#entries.set(key, result);
    this.#rawBytes += canonicalResultRawBytes(result);
    while (this.#entries.size > CANONICAL_RAW_CACHE_MAX_CHUNKS) {
      const oldestKey = this.#entries.keys().next().value as string | undefined;
      if (oldestKey === undefined) {
        break;
      }
      const oldest = this.#entries.get(oldestKey);
      this.#entries.delete(oldestKey);
      if (oldest) {
        this.#rawBytes -= canonicalResultRawBytes(oldest);
      }
    }
  }

  get size(): number {
    return this.#entries.size;
  }

  get rawBytes(): number {
    return Math.max(0, this.#rawBytes);
  }
}

function canonicalResultRawBytes(result: CanonicalWorkerResult): number {
  return result.blocks.byteLength + result.biomes.byteLength;
}
