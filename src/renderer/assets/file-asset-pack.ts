import type { AssetPack } from "./asset-pack";

export interface FileAssetPackHost {
  isFile(path: URL): boolean;

  readText(path: URL): Promise<string>;

  readBytes(path: URL): Promise<Uint8Array>;

  isNotFound(error: unknown): boolean;
}

export class FileAssetPack implements AssetPack {
  public constructor(
    private readonly root: URL,
    private readonly host: FileAssetPackHost,
  ) {}

  public has(path: string): boolean {
    try {
      return this.host.isFile(this.resolve(path));
    } catch {
      return false;
    }
  }

  public async readText(path: string): Promise<string | undefined> {
    try {
      return await this.host.readText(this.resolve(path));
    } catch (error) {
      if (this.host.isNotFound(error)) {
        return undefined;
      }
      throw error;
    }
  }

  public async readBytes(path: string): Promise<Uint8Array | undefined> {
    try {
      return await this.host.readBytes(this.resolve(path));
    } catch (error) {
      if (this.host.isNotFound(error)) {
        return undefined;
      }
      throw error;
    }
  }

  public async readBlob(path: string, contentType?: string): Promise<Blob | undefined> {
    const bytes = await this.readBytes(path);
    if (bytes === undefined) {
      return undefined;
    }

    const copy = new Uint8Array(bytes.byteLength);
    copy.set(bytes);
    return new Blob([copy.buffer], { type: contentType });
  }

  private resolve(path: string): URL {
    const normalized = normalizeAssetPath(path);
    return new URL(normalized, this.root);
  }
}

function normalizeAssetPath(path: string): string {
  const normalized = path.replace(/^\/+/, "");
  if (normalized.length === 0 || normalized.split("/").includes("..")) {
    throw new Error(`Invalid asset path ${path}`);
  }

  return normalized;
}
