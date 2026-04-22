import { DefaultVertexFormat } from "./vertex/default-vertex-format";
import { BufferBuilder } from "./vertex/buffer-builder";
import { VertexFormat, VertexFormatMode } from "./vertex/vertex-format";
import { LineStateShard, RenderStateShard, RenderStateShards, type CullStateShard, type DepthTestStateShard, type EmptyTextureStateShard, type LayeringStateShard, type LightmapStateShard, type OutputStateShard, type OverlayStateShard, type ShaderStateShard, type TexturingStateShard, type TransparencyStateShard, type WriteMaskStateShard } from "./render-state-shard";

export class RenderType extends RenderStateShard {
  private readonly asOptionalValue: RenderType | undefined;

  public constructor(
    name: string,
    private readonly formatValue: VertexFormat,
    private readonly modeValue: VertexFormatMode,
    private readonly bufferSizeValue: number,
    private readonly affectsCrumblingValue: boolean,
    private readonly sortOnUploadValue: boolean,
    setupState: () => void,
    clearState: () => void,
  ) {
    super(name);
    this.setupRenderState = setupState;
    this.clearRenderState = clearState;
    this.asOptionalValue = this;
  }

  public override setupRenderState(): void {}

  public override clearRenderState(): void {}

  public end(bufferBuilder: BufferBuilder, sortX: number, sortY: number, sortZ: number): void {
    if (!bufferBuilder.building()) {
      return;
    }

    if (this.sortOnUploadValue) {
      bufferBuilder.setQuadSortOrigin(sortX, sortY, sortZ);
    }

    bufferBuilder.end();
    this.setupRenderState();
    this.clearRenderState();
  }

  public bufferSize(): number {
    return this.bufferSizeValue;
  }

  public format(): VertexFormat {
    return this.formatValue;
  }

  public mode(): VertexFormatMode {
    return this.modeValue;
  }

  public outline(): RenderType | undefined {
    return undefined;
  }

  public isOutline(): boolean {
    return false;
  }

  public affectsCrumbling(): boolean {
    return this.affectsCrumblingValue;
  }

  public asOptional(): RenderType | undefined {
    return this.asOptionalValue;
  }

  public static solid(): CompositeRenderType {
    return SOLID;
  }

  public static cutoutMipped(): CompositeRenderType {
    return CUTOUT_MIPPED;
  }

  public static cutout(): CompositeRenderType {
    return CUTOUT;
  }

  public static translucent(): CompositeRenderType {
    return TRANSLUCENT;
  }

  public static translucentMovingBlock(): CompositeRenderType {
    return TRANSLUCENT_MOVING_BLOCK;
  }

  public static translucentNoCrumbling(): CompositeRenderType {
    return TRANSLUCENT_NO_CRUMBLING;
  }

  public static tripwire(): CompositeRenderType {
    return TRIPWIRE;
  }

  public static lightning(): CompositeRenderType {
    return LIGHTNING;
  }

  public static lines(): CompositeRenderType {
    return LINES;
  }

  public static lineStrip(): CompositeRenderType {
    return LINE_STRIP;
  }

  public static chunkBufferLayers(): readonly RenderType[] {
    return [RenderType.solid(), RenderType.cutoutMipped(), RenderType.cutout(), RenderType.translucent(), RenderType.tripwire()];
  }

  public static create(
    name: string,
    format: VertexFormat,
    mode: VertexFormatMode,
    bufferSize: number,
    state: RenderTypeCompositeState,
  ): CompositeRenderType;
  public static create(
    name: string,
    format: VertexFormat,
    mode: VertexFormatMode,
    bufferSize: number,
    affectsCrumbling: boolean,
    sortOnUpload: boolean,
    state: RenderTypeCompositeState,
  ): CompositeRenderType;
  public static create(
    name: string,
    format: VertexFormat,
    mode: VertexFormatMode,
    bufferSize: number,
    affectsCrumblingOrState: boolean | RenderTypeCompositeState,
    sortOnUpload?: boolean,
    state?: RenderTypeCompositeState,
  ): CompositeRenderType {
    if (affectsCrumblingOrState instanceof RenderTypeCompositeState) {
      return new CompositeRenderType(name, format, mode, bufferSize, false, false, affectsCrumblingOrState);
    }

    return new CompositeRenderType(
      name,
      format,
      mode,
      bufferSize,
      affectsCrumblingOrState,
      sortOnUpload ?? false,
      state!,
    );
  }
}

export class CompositeRenderType extends RenderType {
  private readonly outlineValue?: RenderType;
  private readonly isOutlineValue: boolean;

  public constructor(
    name: string,
    format: VertexFormat,
    mode: VertexFormatMode,
    bufferSize: number,
    affectsCrumbling: boolean,
    sortOnUpload: boolean,
    private readonly compositeState: RenderTypeCompositeState,
  ) {
    // WebGPU: pipeline state is lowered from the composite state instead of toggling RenderSystem globals.
    super(
      name,
      format,
      mode,
      bufferSize,
      affectsCrumbling,
      sortOnUpload,
      () => {
        for (const state of compositeState.states) {
          state.setupRenderState();
        }
      },
      () => {
        for (const state of compositeState.states) {
          state.clearRenderState();
        }
      },
    );

    // WebGPU: outline RenderType recursion is deferred until the dedicated outline path is ported.
    this.outlineValue = undefined;
    this.isOutlineValue = compositeState.outlineProperty === RenderTypeOutlineProperty.IS_OUTLINE;
  }

  public override outline(): RenderType | undefined {
    return this.outlineValue;
  }

  public override isOutline(): boolean {
    return this.isOutlineValue;
  }

  public state(): RenderTypeCompositeState {
    return this.compositeState;
  }

  public override toString(): string {
    return `RenderType[${this.name}:${this.compositeState}]`;
  }
}

export enum RenderTypeOutlineProperty {
  NONE = "none",
  IS_OUTLINE = "is_outline",
  AFFECTS_OUTLINE = "affects_outline",
}

export class RenderTypeCompositeState {
  public readonly states: readonly RenderStateShard[];

  public constructor(
    public readonly textureState: EmptyTextureStateShard,
    public readonly shaderState: ShaderStateShard,
    public readonly transparencyState: TransparencyStateShard,
    public readonly depthTestState: DepthTestStateShard,
    public readonly cullState: CullStateShard,
    public readonly lightmapState: LightmapStateShard,
    public readonly overlayState: OverlayStateShard,
    public readonly layeringState: LayeringStateShard,
    public readonly outputState: OutputStateShard,
    public readonly texturingState: TexturingStateShard,
    public readonly writeMaskState: WriteMaskStateShard,
    public readonly lineState: LineStateShard,
    public readonly outlineProperty: RenderTypeOutlineProperty,
  ) {
    this.states = [
      this.textureState,
      this.shaderState,
      this.transparencyState,
      this.depthTestState,
      this.cullState,
      this.lightmapState,
      this.overlayState,
      this.layeringState,
      this.outputState,
      this.texturingState,
      this.writeMaskState,
      this.lineState,
    ];
  }

  public toString(): string {
    return `CompositeState[${this.states.join(", ")}, outlineProperty=${this.outlineProperty}]`;
  }

  public static builder(): RenderTypeCompositeStateBuilder {
    return new RenderTypeCompositeStateBuilder();
  }
}

export class RenderTypeCompositeStateBuilder {
  private textureState: EmptyTextureStateShard = RenderStateShards.NO_TEXTURE;
  private shaderState: ShaderStateShard = RenderStateShards.NO_SHADER;
  private transparencyState: TransparencyStateShard = RenderStateShards.NO_TRANSPARENCY;
  private depthTestState: DepthTestStateShard = RenderStateShards.LEQUAL_DEPTH_TEST;
  private cullState: CullStateShard = RenderStateShards.CULL;
  private lightmapState: LightmapStateShard = RenderStateShards.NO_LIGHTMAP;
  private overlayState: OverlayStateShard = RenderStateShards.NO_OVERLAY;
  private layeringState: LayeringStateShard = RenderStateShards.NO_LAYERING;
  private outputState: OutputStateShard = RenderStateShards.MAIN_TARGET;
  private texturingState: TexturingStateShard = RenderStateShards.DEFAULT_TEXTURING;
  private writeMaskState: WriteMaskStateShard = RenderStateShards.COLOR_DEPTH_WRITE;
  private lineState: LineStateShard = RenderStateShards.DEFAULT_LINE;

  public setTextureState(value: EmptyTextureStateShard): this {
    this.textureState = value;
    return this;
  }

  public setShaderState(value: ShaderStateShard): this {
    this.shaderState = value;
    return this;
  }

  public setTransparencyState(value: TransparencyStateShard): this {
    this.transparencyState = value;
    return this;
  }

  public setDepthTestState(value: DepthTestStateShard): this {
    this.depthTestState = value;
    return this;
  }

  public setCullState(value: CullStateShard): this {
    this.cullState = value;
    return this;
  }

  public setLightmapState(value: LightmapStateShard): this {
    this.lightmapState = value;
    return this;
  }

  public setOverlayState(value: OverlayStateShard): this {
    this.overlayState = value;
    return this;
  }

  public setLayeringState(value: LayeringStateShard): this {
    this.layeringState = value;
    return this;
  }

  public setOutputState(value: OutputStateShard): this {
    this.outputState = value;
    return this;
  }

  public setTexturingState(value: TexturingStateShard): this {
    this.texturingState = value;
    return this;
  }

  public setWriteMaskState(value: WriteMaskStateShard): this {
    this.writeMaskState = value;
    return this;
  }

  public setLineState(value: LineStateShard): this {
    this.lineState = value;
    return this;
  }

  public createCompositeState(outline: boolean): RenderTypeCompositeState;
  public createCompositeState(outline: RenderTypeOutlineProperty): RenderTypeCompositeState;
  public createCompositeState(outline: boolean | RenderTypeOutlineProperty): RenderTypeCompositeState {
    const outlineProperty =
      typeof outline === "boolean"
        ? outline
          ? RenderTypeOutlineProperty.AFFECTS_OUTLINE
          : RenderTypeOutlineProperty.NONE
        : outline;

    return new RenderTypeCompositeState(
      this.textureState,
      this.shaderState,
      this.transparencyState,
      this.depthTestState,
      this.cullState,
      this.lightmapState,
      this.overlayState,
      this.layeringState,
      this.outputState,
      this.texturingState,
      this.writeMaskState,
      this.lineState,
      outlineProperty,
    );
  }
}

function translucentState(shaderState: ShaderStateShard): RenderTypeCompositeState {
  return RenderTypeCompositeState.builder()
    .setLightmapState(RenderStateShards.LIGHTMAP)
    .setShaderState(shaderState)
    .setTextureState(RenderStateShards.BLOCK_SHEET_MIPPED)
    .setTransparencyState(RenderStateShards.TRANSLUCENT_TRANSPARENCY)
    .setOutputState(RenderStateShards.TRANSLUCENT_TARGET)
    .createCompositeState(true);
}

function translucentMovingBlockState(): RenderTypeCompositeState {
  return RenderTypeCompositeState.builder()
    .setLightmapState(RenderStateShards.LIGHTMAP)
    .setShaderState(RenderStateShards.RENDERTYPE_TRANSLUCENT_MOVING_BLOCK_SHADER)
    .setTextureState(RenderStateShards.BLOCK_SHEET_MIPPED)
    .setTransparencyState(RenderStateShards.TRANSLUCENT_TRANSPARENCY)
    .setOutputState(RenderStateShards.ITEM_ENTITY_TARGET)
    .createCompositeState(true);
}

function tripwireState(): RenderTypeCompositeState {
  return RenderTypeCompositeState.builder()
    .setLightmapState(RenderStateShards.LIGHTMAP)
    .setShaderState(RenderStateShards.RENDERTYPE_TRIPWIRE_SHADER)
    .setTextureState(RenderStateShards.BLOCK_SHEET_MIPPED)
    .setTransparencyState(RenderStateShards.TRANSLUCENT_TRANSPARENCY)
    .setOutputState(RenderStateShards.WEATHER_TARGET)
    .createCompositeState(true);
}

const SOLID = RenderType.create(
  "solid",
  DefaultVertexFormat.BLOCK,
  VertexFormat.Mode.QUADS,
  2_097_152,
  true,
  false,
  RenderTypeCompositeState.builder()
    .setLightmapState(RenderStateShards.LIGHTMAP)
    .setShaderState(RenderStateShards.RENDERTYPE_SOLID_SHADER)
    .setTextureState(RenderStateShards.BLOCK_SHEET_MIPPED)
    .createCompositeState(true),
);

const CUTOUT_MIPPED = RenderType.create(
  "cutout_mipped",
  DefaultVertexFormat.BLOCK,
  VertexFormat.Mode.QUADS,
  131_072,
  true,
  false,
  RenderTypeCompositeState.builder()
    .setLightmapState(RenderStateShards.LIGHTMAP)
    .setShaderState(RenderStateShards.RENDERTYPE_CUTOUT_MIPPED_SHADER)
    .setTextureState(RenderStateShards.BLOCK_SHEET_MIPPED)
    .createCompositeState(true),
);

const CUTOUT = RenderType.create(
  "cutout",
  DefaultVertexFormat.BLOCK,
  VertexFormat.Mode.QUADS,
  131_072,
  true,
  false,
  RenderTypeCompositeState.builder()
    .setLightmapState(RenderStateShards.LIGHTMAP)
    .setShaderState(RenderStateShards.RENDERTYPE_CUTOUT_SHADER)
    .setTextureState(RenderStateShards.BLOCK_SHEET)
    .createCompositeState(true),
);

const TRANSLUCENT = RenderType.create(
  "translucent",
  DefaultVertexFormat.BLOCK,
  VertexFormat.Mode.QUADS,
  2_097_152,
  true,
  true,
  translucentState(RenderStateShards.RENDERTYPE_TRANSLUCENT_SHADER),
);

const TRANSLUCENT_MOVING_BLOCK = RenderType.create(
  "translucent_moving_block",
  DefaultVertexFormat.BLOCK,
  VertexFormat.Mode.QUADS,
  262_144,
  false,
  true,
  translucentMovingBlockState(),
);

const TRANSLUCENT_NO_CRUMBLING = RenderType.create(
  "translucent_no_crumbling",
  DefaultVertexFormat.BLOCK,
  VertexFormat.Mode.QUADS,
  262_144,
  false,
  true,
  translucentState(RenderStateShards.RENDERTYPE_TRANSLUCENT_NO_CRUMBLING_SHADER),
);

const TRIPWIRE = RenderType.create(
  "tripwire",
  DefaultVertexFormat.BLOCK,
  VertexFormat.Mode.QUADS,
  262_144,
  true,
  true,
  tripwireState(),
);

const LIGHTNING = RenderType.create(
  "lightning",
  DefaultVertexFormat.POSITION_COLOR,
  VertexFormat.Mode.QUADS,
  256,
  false,
  true,
  RenderTypeCompositeState.builder()
    .setShaderState(RenderStateShards.RENDERTYPE_LIGHTNING_SHADER)
    .setWriteMaskState(RenderStateShards.COLOR_DEPTH_WRITE)
    .setTransparencyState(RenderStateShards.LIGHTNING_TRANSPARENCY)
    .createCompositeState(false),
);

const LINES = RenderType.create(
  "lines",
  DefaultVertexFormat.POSITION_COLOR_NORMAL,
  VertexFormat.Mode.LINES,
  256,
  RenderTypeCompositeState.builder()
    .setShaderState(RenderStateShards.RENDERTYPE_LINES_SHADER)
    .setLineState(new LineStateShard())
    .setLayeringState(RenderStateShards.VIEW_OFFSET_Z_LAYERING)
    .setTransparencyState(RenderStateShards.TRANSLUCENT_TRANSPARENCY)
    .setOutputState(RenderStateShards.ITEM_ENTITY_TARGET)
    .setWriteMaskState(RenderStateShards.COLOR_DEPTH_WRITE)
    .setCullState(RenderStateShards.NO_CULL)
    .createCompositeState(false),
);

const LINE_STRIP = RenderType.create(
  "line_strip",
  DefaultVertexFormat.POSITION_COLOR_NORMAL,
  VertexFormat.Mode.LINE_STRIP,
  256,
  RenderTypeCompositeState.builder()
    .setShaderState(RenderStateShards.RENDERTYPE_LINES_SHADER)
    .setLineState(new LineStateShard())
    .setLayeringState(RenderStateShards.VIEW_OFFSET_Z_LAYERING)
    .setTransparencyState(RenderStateShards.TRANSLUCENT_TRANSPARENCY)
    .setOutputState(RenderStateShards.ITEM_ENTITY_TARGET)
    .setWriteMaskState(RenderStateShards.COLOR_DEPTH_WRITE)
    .setCullState(RenderStateShards.NO_CULL)
    .createCompositeState(false),
);
