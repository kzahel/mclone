// Main-thread browser helpers for asset fetch and shared-buffer capability
// reporting. Request identity, scheduling, lifecycle, worker publication,
// asset generations, and diagnostics are owned by Rust.

export async function fetchAssetPack(assetPackUrl: URL): Promise<Uint8Array> {
  const response = await fetch(assetPackUrl);
  if (!response.ok) {
    throw new Error(
      `failed to fetch ${assetPackUrl.pathname}: ${response.status} ${response.statusText}`,
    );
  }
  return new Uint8Array(await response.arrayBuffer());
}

export function renderCompilerSharedMemorySupported(): boolean {
  return typeof SharedArrayBuffer === "function"
    && typeof Atomics === "object"
    && typeof Atomics.load === "function"
    && typeof Atomics.store === "function"
    && typeof Atomics.notify === "function";
}

export function isSharedArrayBuffer(value: unknown): value is SharedArrayBuffer {
  return typeof SharedArrayBuffer === "function" && value instanceof SharedArrayBuffer;
}

export function byteLengthOf(value: unknown): number {
  if (ArrayBuffer.isView(value) || value instanceof ArrayBuffer || isSharedArrayBuffer(value)) {
    return value.byteLength;
  }
  return 0;
}
