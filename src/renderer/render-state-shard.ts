export interface TextureBindingState {
  readonly location: string;
  readonly blur: boolean;
  readonly mipmap: boolean;
}

export class RenderStateShard {
  public constructor(public readonly name: string) {}

  public setupRenderState(): void {}

  public clearRenderState(): void {}

  public toString(): string {
    return this.name;
  }
}

export class BooleanStateShard extends RenderStateShard {
  public constructor(
    name: string,
    protected readonly enabled: boolean,
  ) {
    super(name);
  }

  public isEnabled(): boolean {
    return this.enabled;
  }

  public override toString(): string {
    return `${this.name}[${this.enabled}]`;
  }
}

export class TransparencyStateShard extends RenderStateShard {
  public constructor(
    name: string,
    private readonly blendState?: GPUBlendState,
  ) {
    super(name);
  }

  public getBlendState(): GPUBlendState | undefined {
    return this.blendState;
  }
}

export class ShaderStateShard extends RenderStateShard {
  public constructor(private readonly shaderName?: string) {
    // WebGPU: store a shader program key instead of a GameRenderer ShaderInstance supplier.
    super("shader");
  }

  public getShaderName(): string | undefined {
    return this.shaderName;
  }

  public override toString(): string {
    return `${this.name}[${this.shaderName ?? "none"}]`;
  }
}

export class EmptyTextureStateShard extends RenderStateShard {
  public constructor(
    private readonly textures: readonly TextureBindingState[] = [],
  ) {
    super("texture");
  }

  public cutoutTexture(): string | undefined {
    return this.textures[0]?.location;
  }

  public getTextures(): readonly TextureBindingState[] {
    return this.textures;
  }
}

export class TextureStateShard extends EmptyTextureStateShard {
  public constructor(location: string, blur: boolean, mipmap: boolean) {
    super([{ location, blur, mipmap }]);
  }
}

export class MultiTextureStateShard extends EmptyTextureStateShard {
  public constructor(textures: readonly TextureBindingState[]) {
    super(textures);
  }
}

export class LightmapStateShard extends BooleanStateShard {
  public constructor(enabled: boolean) {
    super("lightmap", enabled);
  }
}

export class OverlayStateShard extends BooleanStateShard {
  public constructor(enabled: boolean) {
    super("overlay", enabled);
  }
}

export class CullStateShard extends BooleanStateShard {
  public constructor(enabled: boolean) {
    super("cull", enabled);
  }
}

export class DepthTestStateShard extends RenderStateShard {
  public constructor(
    private readonly functionName: string,
    private readonly compareFunction: GPUCompareFunction,
  ) {
    super("depth_test");
  }

  public getCompareFunction(): GPUCompareFunction {
    return this.compareFunction;
  }

  public override toString(): string {
    return `${this.name}[${this.functionName}]`;
  }
}

export class WriteMaskStateShard extends RenderStateShard {
  public constructor(
    private readonly writeColor: boolean,
    private readonly writeDepth: boolean,
  ) {
    super("write_mask_state");
  }

  public writesColor(): boolean {
    return this.writeColor;
  }

  public writesDepth(): boolean {
    return this.writeDepth;
  }

  public override toString(): string {
    return `${this.name}[writeColor=${this.writeColor}, writeDepth=${this.writeDepth}]`;
  }
}

export class LayeringStateShard extends RenderStateShard {}

export class OutputStateShard extends RenderStateShard {}

export class TexturingStateShard extends RenderStateShard {}

export class LineStateShard extends RenderStateShard {
  public constructor(private readonly width?: number) {
    super("line_width");
  }

  public getWidth(): number | undefined {
    return this.width;
  }

  public override toString(): string {
    return `${this.name}[${this.width ?? "window_scale"}]`;
  }
}

function blendState(
  color: GPUBlendComponent,
  alpha?: GPUBlendComponent,
): GPUBlendState {
  return {
    color,
    alpha: alpha ?? color,
  };
}

export class RenderStateShards {
  public static readonly NO_TRANSPARENCY = new TransparencyStateShard("no_transparency");
  public static readonly ADDITIVE_TRANSPARENCY = new TransparencyStateShard(
    "additive_transparency",
    blendState({ operation: "add", srcFactor: "one", dstFactor: "one" }),
  );
  public static readonly LIGHTNING_TRANSPARENCY = new TransparencyStateShard(
    "lightning_transparency",
    blendState({ operation: "add", srcFactor: "src-alpha", dstFactor: "one" }),
  );
  public static readonly GLINT_TRANSPARENCY = new TransparencyStateShard(
    "glint_transparency",
    blendState(
      { operation: "add", srcFactor: "src", dstFactor: "one" },
      { operation: "add", srcFactor: "zero", dstFactor: "one" },
    ),
  );
  public static readonly CRUMBLING_TRANSPARENCY = new TransparencyStateShard(
    "crumbling_transparency",
    blendState(
      { operation: "add", srcFactor: "dst", dstFactor: "src" },
      { operation: "add", srcFactor: "one", dstFactor: "zero" },
    ),
  );
  public static readonly TRANSLUCENT_TRANSPARENCY = new TransparencyStateShard(
    "translucent_transparency",
    blendState(
      { operation: "add", srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" },
      { operation: "add", srcFactor: "one", dstFactor: "one-minus-src-alpha" },
    ),
  );

  public static readonly NO_SHADER = new ShaderStateShard();
  public static readonly POSITION_COLOR_SHADER = new ShaderStateShard("position_color");
  public static readonly POSITION_TEX_SHADER = new ShaderStateShard("position_tex");
  public static readonly RENDERTYPE_SOLID_SHADER = new ShaderStateShard("rendertype_solid");
  public static readonly RENDERTYPE_CUTOUT_MIPPED_SHADER = new ShaderStateShard("rendertype_cutout_mipped");
  public static readonly RENDERTYPE_CUTOUT_SHADER = new ShaderStateShard("rendertype_cutout");
  public static readonly RENDERTYPE_TRANSLUCENT_SHADER = new ShaderStateShard("rendertype_translucent");
  public static readonly RENDERTYPE_TRANSLUCENT_MOVING_BLOCK_SHADER = new ShaderStateShard("rendertype_translucent_moving_block");
  public static readonly RENDERTYPE_TRANSLUCENT_NO_CRUMBLING_SHADER = new ShaderStateShard("rendertype_translucent_no_crumbling");
  public static readonly RENDERTYPE_TRIPWIRE_SHADER = new ShaderStateShard("rendertype_tripwire");
  public static readonly RENDERTYPE_LINES_SHADER = new ShaderStateShard("rendertype_lines");
  public static readonly RENDERTYPE_LIGHTNING_SHADER = new ShaderStateShard("position_color");
  public static readonly RENDERTYPE_ENTITY_SOLID_SHADER = new ShaderStateShard("rendertype_entity_solid");
  public static readonly RENDERTYPE_ENTITY_CUTOUT_SHADER = new ShaderStateShard("rendertype_entity_cutout");
  public static readonly RENDERTYPE_ENTITY_CUTOUT_NO_CULL_SHADER = new ShaderStateShard("rendertype_entity_cutout_no_cull");
  public static readonly RENDERTYPE_ENTITY_TRANSLUCENT_SHADER = new ShaderStateShard("rendertype_entity_translucent");

  public static readonly BLOCK_SHEET_MIPPED = new TextureStateShard("minecraft:textures/atlas/blocks.png", false, true);
  public static readonly BLOCK_SHEET = new TextureStateShard("minecraft:textures/atlas/blocks.png", false, false);
  public static readonly NO_TEXTURE = new EmptyTextureStateShard();

  public static readonly DEFAULT_TEXTURING = new TexturingStateShard("default_texturing");
  public static readonly LIGHTMAP = new LightmapStateShard(true);
  public static readonly NO_LIGHTMAP = new LightmapStateShard(false);
  public static readonly OVERLAY = new OverlayStateShard(true);
  public static readonly NO_OVERLAY = new OverlayStateShard(false);
  public static readonly CULL = new CullStateShard(true);
  public static readonly NO_CULL = new CullStateShard(false);
  public static readonly NO_DEPTH_TEST = new DepthTestStateShard("always", "always");
  public static readonly EQUAL_DEPTH_TEST = new DepthTestStateShard("==", "equal");
  public static readonly LEQUAL_DEPTH_TEST = new DepthTestStateShard("<=", "less-equal");
  public static readonly COLOR_DEPTH_WRITE = new WriteMaskStateShard(true, true);
  public static readonly COLOR_WRITE = new WriteMaskStateShard(true, false);
  public static readonly DEPTH_WRITE = new WriteMaskStateShard(false, true);
  public static readonly NO_LAYERING = new LayeringStateShard("no_layering");
  public static readonly VIEW_OFFSET_Z_LAYERING = new LayeringStateShard("view_offset_z_layering");
  public static readonly MAIN_TARGET = new OutputStateShard("main_target");
  public static readonly TRANSLUCENT_TARGET = new OutputStateShard("translucent_target");
  public static readonly ITEM_ENTITY_TARGET = new OutputStateShard("item_entity_target");
  public static readonly WEATHER_TARGET = new OutputStateShard("weather_target");
  public static readonly DEFAULT_LINE = new LineStateShard(1.0);
}
