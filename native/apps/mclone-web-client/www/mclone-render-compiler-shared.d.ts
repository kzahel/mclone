export type RenderCompileDoorbell = Record<string, any>;

export interface RenderSectionWorkerCompilerOptions {
  workerUrl?: URL;
  bindgenJsUrl?: URL;
  bindgenWasmUrl?: URL;
  workerName?: string;
}

export function fetchAssetPack(assetPackUrl: URL): Promise<Uint8Array>;

export class RenderSectionWorkerCompiler {
  constructor(assetPack: Uint8Array, options?: RenderSectionWorkerCompilerOptions);
  compileWithDoorbell(doorbell: RenderCompileDoorbell): Promise<any>;
  pendingJobCount(): number;
  terminate(): void;
}

export function byteLengthOf(value: unknown): number;
export function isSharedArrayBuffer(value: unknown): value is SharedArrayBuffer;
export function renderCompilerSharedMemorySupported(): boolean;
