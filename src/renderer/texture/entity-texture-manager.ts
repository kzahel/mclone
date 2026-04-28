import { ResourceLocation } from "../../core/resource-location";
import type { AssetPack } from "../assets/asset-pack";
import { DEFAULT_COW_TEXTURE, DEFAULT_PIG_TEXTURE, DEFAULT_PLAYER_SKIN } from "../entity/renderable-entity";
import { NativeImage } from "./native-image";
import { decodePngNativeImage } from "./png-native-image-decoder";

const TEXTURE_FORMAT: GPUTextureFormat = "rgba8unorm";
const MISSING_TEXTURE_SIZE = 64;
const OVERLAY_TEXTURE_SIZE = 16;
const OPAQUE_WHITE_RGBA = NativeImage.combine(255, 255, 255, 255);
const MISSING_MAGENTA_RGBA = NativeImage.combine(255, 255, 0, 255);
const MISSING_BLACK_RGBA = NativeImage.combine(255, 0, 0, 0);

interface EntityTextureResource {
  readonly texture: GPUTexture;
  readonly view: GPUTextureView;
}

type TextureLocation = string | ResourceLocation | { toString(): string };

function toResourceLocation(location: TextureLocation): ResourceLocation {
  return location instanceof ResourceLocation ? location : new ResourceLocation(location.toString());
}

export function resolveEntityTexturePath(location: TextureLocation): string {
  const resourceLocation = toResourceLocation(location);
  const path = resourceLocation.getPath();
  if (path.startsWith("textures/") && path.endsWith(".png")) {
    return `assets/${resourceLocation.getNamespace()}/${path}`;
  }

  return `assets/${resourceLocation.getNamespace()}/textures/${path.endsWith(".png") ? path : `${path}.png`}`;
}

export class EntityTextureManager {
  private readonly textures = new Map<string, EntityTextureResource>();
  private readonly ownedResources = new Set<EntityTextureResource>();
  private fallback: EntityTextureResource;
  private closed = false;

  private constructor(
    private readonly device: GPUDevice,
    fallback: EntityTextureResource,
    private readonly overlay: EntityTextureResource,
  ) {
    this.fallback = fallback;
    this.ownedResources.add(fallback);
    this.ownedResources.add(overlay);
  }

  public static createFallback(device: GPUDevice): EntityTextureManager {
    return new EntityTextureManager(device, createMissingTextureResource(device), createOverlayTextureResource(device));
  }

  public static async create(
    device: GPUDevice,
    assetPack: AssetPack,
    initialLocations: readonly TextureLocation[] = [DEFAULT_PLAYER_SKIN, DEFAULT_COW_TEXTURE, DEFAULT_PIG_TEXTURE],
  ): Promise<EntityTextureManager> {
    const manager = EntityTextureManager.createFallback(device);
    for (const location of initialLocations) {
      await manager.load(assetPack, location);
    }
    manager.fallback = manager.textures.get(DEFAULT_PLAYER_SKIN.toString()) ?? manager.fallback;
    return manager;
  }

  public async load(assetPack: AssetPack, location: TextureLocation): Promise<void> {
    this.checkOpen();
    const key = toResourceLocation(location).toString();
    if (this.textures.has(key)) {
      return;
    }

    const bytes = await assetPack.readBytes(resolveEntityTexturePath(location));
    if (bytes === undefined) {
      return;
    }

    const image = await decodePngNativeImage(bytes);
    try {
      const resource = uploadImageTexture(this.device, image);
      this.textures.set(key, resource);
      this.ownedResources.add(resource);
    } finally {
      image.close();
    }
  }

  public getTextureView(location: TextureLocation | undefined): GPUTextureView {
    if (location === undefined) {
      return this.fallback.view;
    }

    return this.textures.get(toResourceLocation(location).toString())?.view ?? this.fallback.view;
  }

  public getOverlayTextureView(): GPUTextureView {
    return this.overlay.view;
  }

  public close(): void {
    if (this.closed) {
      return;
    }

    this.closed = true;
    for (const resource of this.ownedResources) {
      resource.texture.destroy();
    }
    this.ownedResources.clear();
    this.textures.clear();
  }

  private checkOpen(): void {
    if (this.closed) {
      throw new Error("EntityTextureManager is closed");
    }
  }
}

function createTextureResource(device: GPUDevice, width: number, height: number): EntityTextureResource {
  const texture = device.createTexture({
    size: { width, height },
    format: TEXTURE_FORMAT,
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
  });
  return {
    texture,
    view: texture.createView(),
  };
}

function uploadImageTexture(device: GPUDevice, image: NativeImage): EntityTextureResource {
  const resource = createTextureResource(device, image.getWidth(), image.getHeight());
  image.upload(device.queue, resource.texture, 0, 0, 0, 0, 0, image.getWidth(), image.getHeight());
  return resource;
}

function createMissingTextureResource(device: GPUDevice): EntityTextureResource {
  const image = new NativeImage(MISSING_TEXTURE_SIZE, MISSING_TEXTURE_SIZE, false);
  try {
    for (let y = 0; y < MISSING_TEXTURE_SIZE; y++) {
      for (let x = 0; x < MISSING_TEXTURE_SIZE; x++) {
        const checker = ((x >> 4) + (y >> 4)) & 1;
        image.setPixelRGBA(x, y, checker === 0 ? MISSING_MAGENTA_RGBA : MISSING_BLACK_RGBA);
      }
    }
    return uploadImageTexture(device, image);
  } finally {
    image.close();
  }
}

function createOverlayTextureResource(device: GPUDevice): EntityTextureResource {
  const image = new NativeImage(OVERLAY_TEXTURE_SIZE, OVERLAY_TEXTURE_SIZE, false);
  try {
    image.fillRect(0, 0, OVERLAY_TEXTURE_SIZE, OVERLAY_TEXTURE_SIZE, OPAQUE_WHITE_RGBA);
    return uploadImageTexture(device, image);
  } finally {
    image.close();
  }
}
