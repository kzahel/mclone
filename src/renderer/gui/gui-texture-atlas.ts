import type { AssetPack } from "../assets/asset-pack";
import { getBrowserAssetPack } from "../assets/asset-pack";
import { loadNativeImageFromBlob } from "../texture/browser-native-image-loader";
import type { GuiTextureId } from "./gui-draw-list";

interface GuiTextureResource {
  readonly texture: GPUTexture;
  readonly view: GPUTextureView;
  readonly width: number;
  readonly height: number;
}

const GUI_TEXTURE_PATHS = {
  font: "assets/minecraft/textures/font/ascii.png",
  widgets: "assets/minecraft/textures/gui/widgets.png",
} as const satisfies Record<GuiTextureId, string>;

export class GuiTextureAtlas {
  public readonly sampler: GPUSampler;
  private readonly textures = new Map<GuiTextureId, GuiTextureResource>();

  private constructor(
    private readonly device: GPUDevice,
    private readonly assetPack: AssetPack,
  ) {
    this.sampler = device.createSampler({
      addressModeU: "clamp-to-edge",
      addressModeV: "clamp-to-edge",
      magFilter: "nearest",
      minFilter: "nearest",
      mipmapFilter: "nearest",
    });
  }

  public static async create(device: GPUDevice, assetPack?: AssetPack): Promise<GuiTextureAtlas> {
    const pack = assetPack ?? await getBrowserAssetPack();
    const atlas = new GuiTextureAtlas(device, pack);
    await Promise.all(
      (Object.entries(GUI_TEXTURE_PATHS) as Array<[GuiTextureId, string]>).map(async ([id, path]) => {
        atlas.textures.set(id, await atlas.loadTexture(path, `gui-${id}`));
      }),
    );
    return atlas;
  }

  public getView(id: GuiTextureId): GPUTextureView {
    return this.getResource(id).view;
  }

  public getSize(id: GuiTextureId): { readonly width: number; readonly height: number } {
    const resource = this.getResource(id);
    return {
      width: resource.width,
      height: resource.height,
    };
  }

  public destroy(): void {
    for (const resource of this.textures.values()) {
      resource.texture.destroy();
    }
    this.textures.clear();
  }

  private getResource(id: GuiTextureId): GuiTextureResource {
    const resource = this.textures.get(id);
    if (resource === undefined) {
      throw new Error(`GUI texture ${id} was not loaded`);
    }
    return resource;
  }

  private async loadTexture(path: string, label: string): Promise<GuiTextureResource> {
    const blob = await this.assetPack.readBlob(path, "image/png");
    if (blob === undefined) {
      throw new Error(`Unable to load GUI texture ${path}`);
    }

    const image = await loadNativeImageFromBlob(blob);
    try {
      const width = image.getWidth();
      const height = image.getHeight();
      const texture = this.device.createTexture({
        label,
        size: { width, height },
        format: "rgba8unorm",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      });
      image.upload(this.device.queue, texture, 0, 0, 0, 0, 0, width, height);
      return {
        texture,
        view: texture.createView(),
        width,
        height,
      };
    } finally {
      image.close();
    }
  }
}
