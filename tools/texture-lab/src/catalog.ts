import fs from "node:fs/promises";
import path from "node:path";
import { loadTexturePack } from "./load";
import { decodePng, type RgbaImage } from "./png";
import { referenceRoots, runtimeCompatTexturePath } from "./reference";
import type { TexturePackAsset, TextureSpec } from "./dsl";

type AlphaMode = "opaque" | "cutout" | "translucent";
type RenderLayer = "solid" | "cutout" | "cutoutMipped" | "translucent" | "tripwire" | "fluid" | "unknown";
type UsageShape = "full-cube" | "cross-plant" | "partial-model" | "particle-only" | "fluid-sprite";
type CatalogTiling = "xy" | "x" | "none" | "unknown";
type CatalogRotation = "free" | "y90-safe" | "y180-safe" | "fixed" | "model-driven" | "unknown";

interface CatalogArgs {
  input: string;
  outDir: string;
  referenceRoot: string | undefined;
}

interface TextureCatalog {
  schemaVersion: 1;
  referenceRoot: string;
  pack: {
    name: string;
    defaultSize: number;
    textureCount: number;
  };
  summary: CatalogSummary;
  textures: TextureCatalogEntry[];
}

interface CatalogSummary {
  vanillaBlockTextures: number;
  vanillaBlockTexturesWithModelUsage: number;
  authoredRuntimeOverrides: number;
  missingAuthoredRuntimeOverrides: number;
  cutoutOrTranslucentTextures: number;
  nonFullVisualBoundsTextures: number;
  tintedTextures: number;
  animatedTextures: number;
}

interface TextureCatalogEntry {
  texture: string;
  pngPath: string;
  size: {
    width: number;
    height: number;
  };
  alpha: AlphaSummary;
  animation: AnimationSummary | null;
  renderLayers: RenderLayer[];
  tintIndexes: number[];
  inferredTintRoles: string[];
  modelFamilies: string[];
  usageShapes: UsageShape[];
  constraints: TextureConstraints;
  authored: AuthoredTextureStatus;
  usageCount: number;
  uses: TextureUse[];
}

interface AlphaSummary {
  mode: AlphaMode;
  transparentPixels: number;
  translucentPixels: number;
  opaquePixels: number;
  visiblePixels: number;
  visibleCoverage: number;
  visibleBounds: VisibleBounds | null;
  frameBounds: FrameBoundsSummary | null;
}

interface VisibleBounds {
  x: number;
  y: number;
  width: number;
  height: number;
  area: number;
  boxFillRatio: number;
  fillsTexture: boolean;
  touches: {
    left: boolean;
    right: boolean;
    top: boolean;
    bottom: boolean;
  };
}

interface FrameBoundsSummary {
  frameWidth: number;
  frameHeight: number;
  frameCount: number;
  visiblePixels: number;
  visibleCoverage: number;
  visibleBounds: VisibleBounds | null;
}

interface AnimationSummary {
  frameTime: number | null;
  explicitFrames: number | null;
  interpolated: boolean;
}

interface TextureConstraints {
  tiling: CatalogTiling;
  rotation: CatalogRotation;
  alpha: AlphaMode;
  renderLayers: RenderLayer[];
  tintRoles: string[];
}

interface AuthoredTextureStatus {
  status: "missing" | "authored" | "draft" | "reviewed" | "accepted";
  textureName: string | null;
  exportPath: string | null;
  source: string | null;
  tintRole: string | null;
  previewTiling: string | null;
  rotation: string | null;
  notes: string[];
  tags: string[];
}

interface TextureUse {
  role: "face" | "particle";
  block: string;
  blockstatePath: string;
  selector: string;
  model: string;
  modelParentChain: string[];
  modelFamily: string | null;
  variant: VariantUse;
  face: FaceUse | null;
}

interface VariantUse {
  x: number;
  y: number;
  uvlock: boolean;
  weight: number;
}

interface FaceUse {
  direction: string;
  cullface: string | null;
  tintindex: number;
  uv: [number, number, number, number];
  uvRotation: number;
  shade: boolean;
  elementFrom: [number, number, number];
  elementTo: [number, number, number];
  elementRotation: ElementRotation | null;
}

interface ElementRotation {
  origin: [number, number, number] | null;
  axis: string | null;
  angle: number | null;
  rescale: boolean;
}

interface VanillaTexture {
  texture: string;
  pngPath: string;
  alpha: AlphaSummary;
  width: number;
  height: number;
  animation: AnimationSummary | null;
}

interface BlockStateModelUse {
  block: string;
  blockstatePath: string;
  selector: string;
  model: string;
  x: number;
  y: number;
  uvlock: boolean;
  weight: number;
}

interface RawBlockModel {
  parent?: string | undefined;
  textures?: Record<string, string> | undefined;
  ambientocclusion?: boolean | undefined;
  elements?: RawBlockElement[] | undefined;
}

interface RawBlockElement {
  from?: number[] | undefined;
  to?: number[] | undefined;
  rotation?: RawElementRotation | undefined;
  shade?: boolean | undefined;
  faces?: Record<string, RawBlockFace> | undefined;
}

interface RawElementRotation {
  origin?: number[] | undefined;
  axis?: string | undefined;
  angle?: number | undefined;
  rescale?: boolean | undefined;
}

interface RawBlockFace {
  texture?: string | undefined;
  cullface?: string | undefined;
  tintindex?: number | undefined;
  uv?: number[] | undefined;
  rotation?: number | undefined;
}

interface EffectiveElement {
  from: [number, number, number];
  to: [number, number, number];
  rotation: ElementRotation | null;
  shade: boolean;
  faces: Record<string, EffectiveFace>;
}

interface EffectiveFace {
  texture: string;
  cullface: string | null;
  tintindex: number;
  uv: [number, number, number, number];
  uvRotation: number;
}

interface AuthoredTextureRecord {
  textureName: string;
  texture: TextureSpec;
}

const DEFAULT_OUT_DIR = path.join("/tmp", "mclone-texture-lab");
const BLOCK_TEXTURE_PREFIX = "assets/minecraft/textures/block/";
const BLOCK_TEXTURE_SUFFIX = ".png";
const JAVA_RENDER_TYPES_PATH = path.join("net", "minecraft", "client", "renderer", "ItemBlockRenderTypes.java");
const FLUID_TEXTURES = new Set([
  "minecraft:block/water_still",
  "minecraft:block/water_flow",
  "minecraft:block/lava_still",
  "minecraft:block/lava_flow",
]);

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2));
  const pack = await loadTexturePack(args.input);
  const referenceRoot = await resolveReferenceRoot(args.referenceRoot);
  const catalog = await buildCatalog(pack, referenceRoot);
  await writeCatalog(catalog, args.outDir);
}

async function buildCatalog(pack: TexturePackAsset, root: string): Promise<TextureCatalog> {
  const [textures, modelIndex, blockstateUses, renderLayersByBlock] = await Promise.all([
    loadVanillaBlockTextures(root),
    ModelIndex.load(root),
    loadBlockStateModelUses(root),
    loadRenderLayersByBlock(root),
  ]);
  const authoredByRuntimePath = authoredRuntimeTextureMap(pack);
  const usesByTexture = collectTextureUses(modelIndex, blockstateUses);

  const entries = textures.map((texture) => {
    const uses = usesByTexture.get(texture.texture) ?? [];
    return textureEntry(texture, uses, renderLayersByBlock, authoredByRuntimePath);
  });
  const summary = summarizeCatalog(entries);
  return {
    schemaVersion: 1,
    referenceRoot: root,
    pack: {
      name: pack.name,
      defaultSize: pack.defaultSize,
      textureCount: Object.keys(pack.textures).length,
    },
    summary,
    textures: entries,
  };
}

function textureEntry(
  texture: VanillaTexture,
  uses: TextureUse[],
  renderLayersByBlock: Map<string, RenderLayer>,
  authoredByRuntimePath: Map<string, AuthoredTextureRecord>,
): TextureCatalogEntry {
  const renderLayers = uniqueSorted(
    uses
      .map((use) => renderLayerForUse(use, renderLayersByBlock))
      .concat(FLUID_TEXTURES.has(texture.texture) ? ["fluid"] : []),
  );
  const tintIndexes = uniqueSorted(
    uses.flatMap((use) => (use.face && use.face.tintindex >= 0 ? [use.face.tintindex] : [])),
  );
  const inferredTintRoles = uniqueSorted(uses.flatMap(inferTintRolesForUse));
  const modelFamilies = uniqueSorted(uses.flatMap((use) => (use.modelFamily ? [use.modelFamily] : [])));
  const usageShapes = inferUsageShapes(texture.texture, uses);
  const authored = authoredStatus(texture.pngPath, authoredByRuntimePath);
  const inferredTiling = inferTiling(texture.texture, texture.alpha.mode, usageShapes, uses);
  const inferredRotation = inferRotation(texture.texture, usageShapes, uses);
  const constraints = {
    tiling: isCatalogTiling(authored.previewTiling) ? authored.previewTiling : inferredTiling,
    rotation: isCatalogRotation(authored.rotation) ? authored.rotation : inferredRotation,
    alpha: texture.alpha.mode,
    renderLayers,
    tintRoles: inferredTintRoles,
  };

  return {
    texture: texture.texture,
    pngPath: texture.pngPath,
    size: {
      width: texture.width,
      height: texture.height,
    },
    alpha: texture.alpha,
    animation: texture.animation,
    renderLayers,
    tintIndexes,
    inferredTintRoles,
    modelFamilies,
    usageShapes,
    constraints,
    authored,
    usageCount: uses.length,
    uses,
  };
}

async function writeCatalog(catalog: TextureCatalog, outDir: string): Promise<void> {
  await fs.mkdir(outDir, { recursive: true });
  const base = `${catalog.pack.name}-texture-catalog`;
  const jsonPath = path.join(outDir, `${base}.json`);
  const markdownPath = path.join(outDir, `${base}.md`);
  await fs.writeFile(jsonPath, `${JSON.stringify(catalog, null, 2)}\n`);
  await fs.writeFile(markdownPath, catalogMarkdown(catalog));
  console.log(`Wrote ${jsonPath}`);
  console.log(`Wrote ${markdownPath}`);
  console.log(
    `texture catalog: ${catalog.summary.authoredRuntimeOverrides}/${catalog.summary.vanillaBlockTextures} authored, ${catalog.summary.cutoutOrTranslucentTextures} cutout/translucent, ${catalog.summary.tintedTextures} tinted`,
  );
}

async function loadVanillaBlockTextures(root: string): Promise<VanillaTexture[]> {
  const textureRoot = path.join(root, BLOCK_TEXTURE_PREFIX);
  const files = (await listFiles(textureRoot))
    .filter((file) => file.endsWith(BLOCK_TEXTURE_SUFFIX))
    .sort((a, b) => a.localeCompare(b));
  const textures: VanillaTexture[] = [];
  for (const file of files) {
    const relative = normalizePath(path.relative(root, file));
    const name = normalizePath(path.relative(textureRoot, file)).replace(/\.png$/u, "");
    const image = decodePng(await fs.readFile(file));
    const animation = await loadAnimationMetadata(file);
    textures.push({
      texture: `minecraft:block/${name}`,
      pngPath: relative,
      alpha: analyzeAlpha(image, animation ? image.width : null),
      width: image.width,
      height: image.height,
      animation,
    });
  }
  return textures;
}

async function loadBlockStateModelUses(root: string): Promise<BlockStateModelUse[]> {
  const blockstateRoot = path.join(root, "assets", "minecraft", "blockstates");
  const files = (await listFiles(blockstateRoot)).filter((file) => file.endsWith(".json")).sort();
  const uses: BlockStateModelUse[] = [];
  for (const file of files) {
    const blockPath = normalizePath(path.relative(blockstateRoot, file)).replace(/\.json$/u, "");
    const block = `minecraft:${blockPath}`;
    const blockstatePath = normalizePath(path.relative(root, file));
    const json = asRecord(JSON.parse(await fs.readFile(file, "utf8")));
    uses.push(...extractVariantUses(block, blockstatePath, json));
    uses.push(...extractMultipartUses(block, blockstatePath, json));
  }
  return uses;
}

function extractVariantUses(block: string, blockstatePath: string, json: Record<string, unknown>): BlockStateModelUse[] {
  const variants = asOptionalRecord(json.variants);
  if (!variants) {
    return [];
  }
  const uses: BlockStateModelUse[] = [];
  for (const [key, value] of Object.entries(variants)) {
    uses.push(...variantObjects(value).map((variant) => blockStateModelUse(block, blockstatePath, `variant:${key || "<default>"}`, variant)));
  }
  return uses;
}

function extractMultipartUses(block: string, blockstatePath: string, json: Record<string, unknown>): BlockStateModelUse[] {
  if (!Array.isArray(json.multipart)) {
    return [];
  }
  const uses: BlockStateModelUse[] = [];
  for (const [index, entry] of json.multipart.entries()) {
    const object = asOptionalRecord(entry);
    if (!object) {
      continue;
    }
    for (const variant of variantObjects(object.apply)) {
      uses.push(blockStateModelUse(block, blockstatePath, `multipart:${index}`, variant));
    }
  }
  return uses;
}

function variantObjects(value: unknown): Record<string, unknown>[] {
  if (Array.isArray(value)) {
    return value.map(asOptionalRecord).filter(isPresent);
  }
  const object = asOptionalRecord(value);
  return object ? [object] : [];
}

function blockStateModelUse(
  block: string,
  blockstatePath: string,
  selector: string,
  variant: Record<string, unknown>,
): BlockStateModelUse {
  const model = stringField(variant, "model");
  return {
    block,
    blockstatePath,
    selector,
    model: normalizeResourceLocation(model ?? "minecraft:block/missing"),
    x: normalizedRotation(numberField(variant, "x", 0)),
    y: normalizedRotation(numberField(variant, "y", 0)),
    uvlock: booleanField(variant, "uvlock", false),
    weight: Math.max(1, numberField(variant, "weight", 1)),
  };
}

function collectTextureUses(modelIndex: ModelIndex, blockstateUses: BlockStateModelUse[]): Map<string, TextureUse[]> {
  const usesByTexture = new Map<string, TextureUse[]>();
  for (const blockstateUse of blockstateUses) {
    const parentChain = modelIndex.parentChain(blockstateUse.model);
    const modelFamily = inferModelFamily(parentChain);
    const variant = {
      x: blockstateUse.x,
      y: blockstateUse.y,
      uvlock: blockstateUse.uvlock,
      weight: blockstateUse.weight,
    };
    const particle = modelIndex.resolveTexture(blockstateUse.model, "particle");
    if (particle?.startsWith("minecraft:block/")) {
      pushUse(usesByTexture, particle, {
        role: "particle",
        block: blockstateUse.block,
        blockstatePath: blockstateUse.blockstatePath,
        selector: blockstateUse.selector,
        model: blockstateUse.model,
        modelParentChain: parentChain,
        modelFamily,
        variant,
        face: null,
      });
    }
    for (const element of modelIndex.effectiveElements(blockstateUse.model)) {
      for (const [direction, face] of Object.entries(element.faces)) {
        const texture = modelIndex.resolveTexture(blockstateUse.model, face.texture);
        if (!texture?.startsWith("minecraft:block/")) {
          continue;
        }
        pushUse(usesByTexture, texture, {
          role: "face",
          block: blockstateUse.block,
          blockstatePath: blockstateUse.blockstatePath,
          selector: blockstateUse.selector,
          model: blockstateUse.model,
          modelParentChain: parentChain,
          modelFamily,
          variant,
          face: {
            direction,
            cullface: face.cullface,
            tintindex: face.tintindex,
            uv: face.uv,
            uvRotation: face.uvRotation,
            shade: element.shade,
            elementFrom: element.from,
            elementTo: element.to,
            elementRotation: element.rotation,
          },
        });
      }
    }
  }
  return usesByTexture;
}

function pushUse(map: Map<string, TextureUse[]>, texture: string, use: TextureUse): void {
  const uses = map.get(texture);
  if (uses) {
    uses.push(use);
  } else {
    map.set(texture, [use]);
  }
}

class ModelIndex {
  private constructor(private readonly models: Map<string, RawBlockModel>) {}

  static async load(root: string): Promise<ModelIndex> {
    const modelRoot = path.join(root, "assets", "minecraft", "models", "block");
    const files = (await listFiles(modelRoot)).filter((file) => file.endsWith(".json")).sort();
    const models = new Map<string, RawBlockModel>();
    for (const file of files) {
      const modelPath = normalizePath(path.relative(modelRoot, file)).replace(/\.json$/u, "");
      const location = `minecraft:block/${modelPath}`;
      models.set(location, JSON.parse(await fs.readFile(file, "utf8")) as RawBlockModel);
    }
    return new ModelIndex(models);
  }

  parentChain(location: string): string[] {
    const chain: string[] = [];
    let current: string | undefined = location;
    const seen = new Set<string>();
    while (current && !seen.has(current)) {
      seen.add(current);
      const model = this.models.get(current);
      const parent = model?.parent ? normalizeResourceLocation(model.parent) : undefined;
      if (!parent) {
        break;
      }
      chain.push(parent);
      current = parent;
    }
    return chain;
  }

  effectiveElements(location: string): EffectiveElement[] {
    const model = this.models.get(location);
    if (!model) {
      return [];
    }
    if (model.elements && model.elements.length > 0) {
      return model.elements.map((element) => effectiveElement(element));
    }
    if (model.parent) {
      return this.effectiveElements(normalizeResourceLocation(model.parent));
    }
    return [];
  }

  resolveTexture(location: string, slotOrReference: string): string | null {
    if (!slotOrReference.startsWith("#")) {
      return normalizeResourceLocation(slotOrReference);
    }
    return this.resolveTextureSlot(location, slotOrReference.slice(1), []);
  }

  private resolveTextureSlot(location: string, slot: string, seen: string[]): string | null {
    if (seen.includes(slot)) {
      return null;
    }
    const reference = this.findTextureEntry(location, slot);
    if (!reference) {
      return null;
    }
    if (reference.startsWith("#")) {
      return this.resolveTextureSlot(location, reference.slice(1), [...seen, slot]);
    }
    return normalizeResourceLocation(reference);
  }

  private findTextureEntry(location: string, slot: string): string | null {
    const model = this.models.get(location);
    if (!model) {
      return null;
    }
    const own = model.textures?.[slot];
    if (own) {
      return own;
    }
    if (!model.parent) {
      return null;
    }
    return this.findTextureEntry(normalizeResourceLocation(model.parent), slot);
  }
}

function effectiveElement(element: RawBlockElement): EffectiveElement {
  const from = tuple3(element.from, [0, 0, 0]);
  const to = tuple3(element.to, [16, 16, 16]);
  const faces: Record<string, EffectiveFace> = {};
  for (const [direction, face] of Object.entries(element.faces ?? {})) {
    const texture = face.texture;
    if (!texture) {
      continue;
    }
    faces[direction] = {
      texture,
      cullface: face.cullface ?? null,
      tintindex: numberField(face as Record<string, unknown>, "tintindex", -1),
      uv: tuple4(face.uv, defaultFaceUv(direction, from, to)),
      uvRotation: normalizedRotation(numberField(face as Record<string, unknown>, "rotation", 0)),
    };
  }
  return {
    from,
    to,
    rotation: element.rotation ? elementRotation(element.rotation) : null,
    shade: element.shade ?? true,
    faces,
  };
}

function elementRotation(rotation: RawElementRotation): ElementRotation {
  return {
    origin: rotation.origin ? tuple3(rotation.origin, [8, 8, 8]) : null,
    axis: rotation.axis ?? null,
    angle: typeof rotation.angle === "number" ? rotation.angle : null,
    rescale: rotation.rescale ?? false,
  };
}

function defaultFaceUv(direction: string, from: [number, number, number], to: [number, number, number]): [number, number, number, number] {
  const [minX, minY, minZ] = from;
  const [maxX, maxY, maxZ] = to;
  switch (direction) {
    case "down":
    case "up":
      return [minX, minZ, maxX, maxZ];
    case "north":
    case "south":
      return [16 - maxX, 16 - maxY, 16 - minX, 16 - minY];
    case "west":
    case "east":
      return [minZ, 16 - maxY, maxZ, 16 - minY];
    default:
      return [0, 0, 16, 16];
  }
}

async function loadRenderLayersByBlock(root: string): Promise<Map<string, RenderLayer>> {
  const candidates = [
    path.resolve(root, "..", "src", JAVA_RENDER_TYPES_PATH),
    path.resolve("..", "..", "reference", "minecraft-1.17.1", "src", JAVA_RENDER_TYPES_PATH),
  ];
  const javaPath = await firstExistingFile(candidates);
  const layers = new Map<string, RenderLayer>();
  if (!javaPath) {
    return layers;
  }
  const source = await fs.readFile(javaPath, "utf8");
  const vars = new Map<string, RenderLayer>();
  for (const match of source.matchAll(/RenderType\s+(\w+)\s*=\s*RenderType\.([A-Za-z0-9_]+)\(\)/gu)) {
    const variable = match[1];
    const method = match[2];
    if (variable && method) {
      vars.set(variable, renderLayerFromMethod(method));
    }
  }
  for (const match of source.matchAll(/var0\.put\(Blocks\.([A-Z0-9_]+),\s*(\w+)\)/gu)) {
    const blockName = match[1];
    const variable = match[2];
    if (!blockName || !variable) {
      continue;
    }
    const layer = vars.get(variable) ?? "unknown";
    layers.set(`minecraft:${blockName.toLowerCase()}`, layer);
  }
  for (const block of [...layers.keys()].filter((block) => block.endsWith("_leaves"))) {
    layers.set(block, "cutoutMipped");
  }
  return layers;
}

function renderLayerFromMethod(method: string): RenderLayer {
  switch (method) {
    case "solid":
      return "solid";
    case "cutout":
      return "cutout";
    case "cutoutMipped":
      return "cutoutMipped";
    case "translucent":
      return "translucent";
    case "tripwire":
      return "tripwire";
    default:
      return "unknown";
  }
}

function renderLayerForUse(use: TextureUse, renderLayersByBlock: Map<string, RenderLayer>): RenderLayer {
  return renderLayersByBlock.get(use.block) ?? "solid";
}

function authoredRuntimeTextureMap(pack: TexturePackAsset): Map<string, AuthoredTextureRecord> {
  const map = new Map<string, AuthoredTextureRecord>();
  for (const [textureName, texture] of Object.entries(pack.textures)) {
    const runtimePath = runtimeCompatTexturePath(texture.exportPath);
    if (runtimePath) {
      map.set(runtimePath, { textureName, texture });
    }
  }
  return map;
}

function authoredStatus(pathName: string, authoredByRuntimePath: Map<string, AuthoredTextureRecord>): AuthoredTextureStatus {
  const authored = authoredByRuntimePath.get(pathName);
  if (!authored) {
    return {
      status: "missing",
      textureName: null,
      exportPath: null,
      source: null,
      tintRole: null,
      previewTiling: null,
      rotation: null,
      notes: [],
      tags: [],
    };
  }
  return {
    status: authored.texture.catalog?.status ?? "authored",
    textureName: authored.textureName,
    exportPath: authored.texture.exportPath,
    source: authored.texture.source ?? "final-color",
    tintRole: authored.texture.tintRole ?? null,
    previewTiling: authored.texture.catalog?.tiling ?? authored.texture.preview?.tiling ?? null,
    rotation: authored.texture.catalog?.rotation ?? null,
    notes: authored.texture.catalog?.notes ?? [],
    tags: authored.texture.catalog?.tags ?? [],
  };
}

function inferUsageShapes(texture: string, uses: TextureUse[]): UsageShape[] {
  const shapes = new Set<UsageShape>();
  if (FLUID_TEXTURES.has(texture)) {
    shapes.add("fluid-sprite");
  }
  for (const use of uses) {
    if (use.role === "particle") {
      shapes.add("particle-only");
      continue;
    }
    if (use.modelFamily === "cross" || use.modelFamily === "tinted_cross") {
      shapes.add("cross-plant");
    } else if (use.face && isFullCubeElement(use.face.elementFrom, use.face.elementTo)) {
      shapes.add("full-cube");
    } else {
      shapes.add("partial-model");
    }
  }
  if (shapes.size > 1 && shapes.has("particle-only")) {
    shapes.delete("particle-only");
  }
  return [...shapes].sort();
}

function inferTiling(texture: string, alpha: AlphaMode, shapes: UsageShape[], uses: TextureUse[]): CatalogTiling {
  if (texture.endsWith("_flow") || texture.includes("_flow_")) {
    return "x";
  }
  if (shapes.includes("cross-plant") || shapes.includes("fluid-sprite")) {
    return "none";
  }
  if (texture.endsWith("_side") || texture.includes("_side_")) {
    return "x";
  }
  if (alpha !== "opaque" && !shapes.includes("full-cube")) {
    return "none";
  }
  const faceUses = uses.filter((use) => use.face);
  if (faceUses.length > 0 && faceUses.every((use) => use.face && isFullUv(use.face.uv))) {
    return "xy";
  }
  if (shapes.includes("full-cube")) {
    return "xy";
  }
  return "unknown";
}

function inferRotation(texture: string, shapes: UsageShape[], uses: TextureUse[]): CatalogRotation {
  if (shapes.includes("cross-plant") || shapes.includes("fluid-sprite")) {
    return "fixed";
  }
  const yRotations = uniqueSorted(uses.map((use) => use.variant.y));
  const xRotations = uniqueSorted(uses.map((use) => use.variant.x));
  if (
    yRotations.length > 1 &&
    xRotations.every((value) => value === 0) &&
    yRotations.every((value) => value === 0 || value === 180)
  ) {
    return "y180-safe";
  }
  if (
    yRotations.length > 1 &&
    xRotations.every((value) => value === 0) &&
    yRotations.every((value) => value === 0 || value === 90 || value === 180 || value === 270)
  ) {
    return "y90-safe";
  }
  if (
    uses.some(
      (use) =>
        use.variant.x !== 0 ||
        use.variant.y !== 0 ||
        use.variant.uvlock ||
        (use.face?.uvRotation ?? 0) !== 0 ||
        use.face?.elementRotation !== null,
    )
  ) {
    return "model-driven";
  }
  if (uses.length === 0 && !FLUID_TEXTURES.has(texture)) {
    return "unknown";
  }
  return "free";
}

function inferTintRolesForUse(use: TextureUse): string[] {
  if (!use.face || use.face.tintindex < 0) {
    return [];
  }
  const block = use.block.split(":")[1] ?? use.block;
  if (block.includes("leaves") || block.includes("azalea")) {
    return ["foliage"];
  }
  if (block.includes("water") || block.includes("seagrass") || block.includes("kelp")) {
    return ["water"];
  }
  if (
    block.includes("grass") ||
    block.includes("fern") ||
    block.includes("vine") ||
    block.includes("lily_pad") ||
    block.includes("sugar_cane")
  ) {
    return ["grass"];
  }
  return [`tintindex:${use.face.tintindex}`];
}

function inferModelFamily(parentChain: string[]): string | null {
  for (const parent of parentChain) {
    const name = parent.split(":")[1] ?? parent;
    if (name === "block/tinted_cross") {
      return "tinted_cross";
    }
    if (name === "block/cross") {
      return "cross";
    }
    if (name.startsWith("block/cube")) {
      return name.slice("block/".length);
    }
    if (name.startsWith("block/template_")) {
      return name.slice("block/".length);
    }
  }
  return parentChain[0] ?? null;
}

function summarizeCatalog(entries: TextureCatalogEntry[]): CatalogSummary {
  const authoredRuntimeOverrides = entries.filter((entry) => entry.authored.status !== "missing").length;
  return {
    vanillaBlockTextures: entries.length,
    vanillaBlockTexturesWithModelUsage: entries.filter((entry) => entry.usageCount > 0).length,
    authoredRuntimeOverrides,
    missingAuthoredRuntimeOverrides: entries.length - authoredRuntimeOverrides,
    cutoutOrTranslucentTextures: entries.filter((entry) => entry.alpha.mode !== "opaque" || entry.renderLayers.some((layer) => layer === "cutout" || layer === "cutoutMipped" || layer === "translucent")).length,
    nonFullVisualBoundsTextures: entries.filter((entry) => {
      const bounds = reportVisibleBounds(entry);
      return bounds && !bounds.fillsTexture;
    }).length,
    tintedTextures: entries.filter((entry) => entry.tintIndexes.length > 0).length,
    animatedTextures: entries.filter((entry) => entry.animation !== null).length,
  };
}

function catalogMarkdown(catalog: TextureCatalog): string {
  const lines: string[] = [];
  lines.push(`# Texture Usage Catalog: ${catalog.pack.name}`);
  lines.push("");
  lines.push(`Reference root: \`${catalog.referenceRoot}\``);
  lines.push("");
  lines.push("## Summary");
  lines.push("");
  lines.push(`- Vanilla block texture PNGs: ${catalog.summary.vanillaBlockTextures}`);
  lines.push(`- With model/blockstate usage: ${catalog.summary.vanillaBlockTexturesWithModelUsage}`);
  lines.push(`- Authored runtime overrides: ${catalog.summary.authoredRuntimeOverrides}`);
  lines.push(`- Missing runtime overrides: ${catalog.summary.missingAuthoredRuntimeOverrides}`);
  lines.push(`- Cutout/translucent or alpha-bearing textures: ${catalog.summary.cutoutOrTranslucentTextures}`);
  lines.push(`- Non-full visual bounds: ${catalog.summary.nonFullVisualBoundsTextures}`);
  lines.push(`- Tinted textures: ${catalog.summary.tintedTextures}`);
  lines.push(`- Animated textures: ${catalog.summary.animatedTextures}`);
  lines.push("");
  pushPrioritySection(
    lines,
    "Decoration And Cutout Priorities",
    catalog.textures.filter(
      (entry) =>
        entry.authored.status === "missing" &&
        (entry.usageShapes.includes("cross-plant") ||
          entry.renderLayers.includes("cutout") ||
          entry.renderLayers.includes("cutoutMipped") ||
          entry.alpha.mode !== "opaque"),
    ).sort(priorityEntrySort),
    80,
  );
  pushPrioritySection(
    lines,
    "Tinted Priorities",
    catalog.textures
      .filter((entry) => entry.authored.status === "missing" && entry.tintIndexes.length > 0)
      .sort(priorityEntrySort),
    80,
  );
  lines.push("## Matrix");
  lines.push("");
  lines.push(
    "| Texture | Authored | Layer | Alpha | Bounds | Tint | Tiling | Rotation | Shape | Uses | Size | Animation | Families |",
  );
  lines.push("|---|---|---|---|---|---|---|---|---|---:|---|---|---|");
  for (const entry of catalog.textures) {
    lines.push(matrixRow(entry));
  }
  return `${lines.join("\n").trimEnd()}\n`;
}

function pushPrioritySection(lines: string[], title: string, entries: TextureCatalogEntry[], limit: number): void {
  lines.push(`## ${title}`);
  lines.push("");
  if (entries.length === 0) {
    lines.push("- none");
    lines.push("");
    return;
  }
  lines.push("| Texture | Layer | Alpha | Bounds | Tint | Tiling | Rotation | Shape | Uses | Families |");
  lines.push("|---|---|---|---|---|---|---|---|---:|---|");
  for (const entry of entries.slice(0, limit)) {
    lines.push(
      [
        md(entry.texture),
        md(joinOrDash(entry.renderLayers)),
        md(entry.alpha.mode),
        md(formatVisibleBounds(entry)),
        md(joinOrDash(entry.inferredTintRoles.length > 0 ? entry.inferredTintRoles : entry.tintIndexes.map(String))),
        md(entry.constraints.tiling),
        md(entry.constraints.rotation),
        md(joinOrDash(entry.usageShapes)),
        String(entry.usageCount),
        md(joinOrDash(entry.modelFamilies)),
      ].join(" | ").replace(/^/u, "| ").replace(/$/u, " |"),
    );
  }
  if (entries.length > limit) {
    lines.push(`| ... | ${entries.length - limit} more omitted from this priority section; see Matrix below. | | | | | | | | |`);
  }
  lines.push("");
}

function matrixRow(entry: TextureCatalogEntry): string {
  return [
    md(entry.texture),
    md(entry.authored.status === "missing" ? "missing" : entry.authored.textureName ?? entry.authored.status),
    md(joinOrDash(entry.renderLayers)),
    md(entry.alpha.mode),
    md(formatVisibleBounds(entry)),
    md(joinOrDash(entry.inferredTintRoles.length > 0 ? entry.inferredTintRoles : entry.tintIndexes.map(String))),
    md(entry.constraints.tiling),
    md(entry.constraints.rotation),
    md(joinOrDash(entry.usageShapes)),
    String(entry.usageCount),
    `${entry.size.width}x${entry.size.height}`,
    entry.animation ? "yes" : "-",
    md(joinOrDash(entry.modelFamilies)),
  ].join(" | ").replace(/^/u, "| ").replace(/$/u, " |");
}

function formatVisibleBounds(entry: TextureCatalogEntry): string {
  const frameBounds = entry.alpha.frameBounds;
  const bounds = frameBounds?.visibleBounds ?? entry.alpha.visibleBounds;
  if (!bounds) {
    return "-";
  }
  const coverage = frameBounds?.visibleCoverage ?? entry.alpha.visibleCoverage;
  const suffix = frameBounds ? "/frame" : "";
  if (bounds.fillsTexture) {
    return `full ${formatPercent(coverage)}${suffix}`;
  }
  return `${bounds.x},${bounds.y} ${bounds.width}x${bounds.height} ${formatPercent(coverage)}${suffix}`;
}

function reportVisibleBounds(entry: TextureCatalogEntry): VisibleBounds | null {
  return entry.alpha.frameBounds?.visibleBounds ?? entry.alpha.visibleBounds;
}

function formatPercent(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}

function analyzeAlpha(image: RgbaImage, animationFrameHeight: number | null): AlphaSummary {
  const data = image.data;
  let transparentPixels = 0;
  let translucentPixels = 0;
  let opaquePixels = 0;
  let minX = image.width;
  let minY = image.height;
  let maxX = -1;
  let maxY = -1;
  for (let index = 3; index < data.length; index += 4) {
    const alpha = data[index]!;
    const pixel = (index - 3) / 4;
    const x = pixel % image.width;
    const y = Math.floor(pixel / image.width);
    if (alpha === 0) {
      transparentPixels += 1;
    } else {
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
      if (alpha === 255) {
        opaquePixels += 1;
      } else {
        translucentPixels += 1;
      }
    }
  }
  const mode: AlphaMode = translucentPixels > 0 ? "translucent" : transparentPixels > 0 ? "cutout" : "opaque";
  const visiblePixels = opaquePixels + translucentPixels;
  const visibleBounds = visiblePixels > 0 ? makeVisibleBounds(image.width, image.height, minX, minY, maxX, maxY, visiblePixels) : null;
  return {
    mode,
    transparentPixels,
    translucentPixels,
    opaquePixels,
    visiblePixels,
    visibleCoverage: visiblePixels / (image.width * image.height),
    visibleBounds,
    frameBounds: animationFrameHeight ? analyzeFrameBounds(image, animationFrameHeight) : null,
  };
}

function analyzeFrameBounds(image: RgbaImage, frameHeight: number): FrameBoundsSummary | null {
  if (frameHeight <= 0 || image.height % frameHeight !== 0) {
    return null;
  }
  const frameCount = image.height / frameHeight;
  if (frameCount <= 1) {
    return null;
  }
  let minX = image.width;
  let minY = frameHeight;
  let maxX = -1;
  let maxY = -1;
  let visiblePixels = 0;
  const visibleFrameCells = new Set<number>();
  for (let y = 0; y < image.height; y += 1) {
    const frameY = y % frameHeight;
    for (let x = 0; x < image.width; x += 1) {
      const alpha = image.data[(y * image.width + x) * 4 + 3]!;
      if (alpha === 0) {
        continue;
      }
      visiblePixels += 1;
      visibleFrameCells.add(frameY * image.width + x);
      minX = Math.min(minX, x);
      minY = Math.min(minY, frameY);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, frameY);
    }
  }
  return {
    frameWidth: image.width,
    frameHeight,
    frameCount,
    visiblePixels,
    visibleCoverage: visiblePixels / (image.width * frameHeight * frameCount),
    visibleBounds: visiblePixels > 0
      ? makeVisibleBounds(image.width, frameHeight, minX, minY, maxX, maxY, visibleFrameCells.size)
      : null,
  };
}

function makeVisibleBounds(
  textureWidth: number,
  textureHeight: number,
  minX: number,
  minY: number,
  maxX: number,
  maxY: number,
  visiblePixels: number,
): VisibleBounds {
  const width = maxX - minX + 1;
  const height = maxY - minY + 1;
  const area = width * height;
  return {
    x: minX,
    y: minY,
    width,
    height,
    area,
    boxFillRatio: visiblePixels / area,
    fillsTexture: minX === 0 && minY === 0 && width === textureWidth && height === textureHeight,
    touches: {
      left: minX === 0,
      right: maxX === textureWidth - 1,
      top: minY === 0,
      bottom: maxY === textureHeight - 1,
    },
  };
}

async function loadAnimationMetadata(pngPath: string): Promise<AnimationSummary | null> {
  const metadataPath = `${pngPath}.mcmeta`;
  try {
    const metadata = asOptionalRecord(JSON.parse(await fs.readFile(metadataPath, "utf8")));
    const animation = asOptionalRecord(metadata?.animation);
    if (!animation) {
      return null;
    }
    return {
      frameTime: typeof animation.frametime === "number" ? animation.frametime : null,
      explicitFrames: Array.isArray(animation.frames) ? animation.frames.length : null,
      interpolated: animation.interpolate === true,
    };
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code;
    if (code === "ENOENT") {
      return null;
    }
    throw new Error(`Failed to read animation metadata '${metadataPath}': ${(error as Error).message}`);
  }
}

async function resolveReferenceRoot(referenceRoot: string | undefined): Promise<string> {
  const candidates = referenceRoot ? [path.resolve(referenceRoot)] : referenceRoots();
  for (const candidate of candidates) {
    const assetsRoot = path.join(candidate, "assets", "minecraft");
    try {
      const stat = await fs.stat(assetsRoot);
      if (stat.isDirectory()) {
        return candidate;
      }
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code;
      if (code !== "ENOENT") {
        throw error;
      }
    }
  }
  throw new Error(
    `No extracted Minecraft asset root found. Tried: ${candidates.map((candidate) => `'${candidate}'`).join(", ")}`,
  );
}

async function listFiles(root: string): Promise<string[]> {
  const entries = await fs.readdir(root, { withFileTypes: true });
  const files: string[] = [];
  for (const entry of entries) {
    const file = path.join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listFiles(file)));
    } else if (entry.isFile()) {
      files.push(file);
    }
  }
  return files;
}

async function firstExistingFile(candidates: string[]): Promise<string | null> {
  for (const candidate of candidates) {
    try {
      const stat = await fs.stat(candidate);
      if (stat.isFile()) {
        return candidate;
      }
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code;
      if (code !== "ENOENT") {
        throw error;
      }
    }
  }
  return null;
}

function parseArgs(argv: string[]): CatalogArgs {
  const input = argv[0];
  if (!input || input.startsWith("-")) {
    throw new Error("Usage: tsx src/catalog.ts <texture.ts> [--out <dir>] [--reference-root <dir>]");
  }
  let outDir = DEFAULT_OUT_DIR;
  let referenceRoot: string | undefined;
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--") {
      continue;
    }
    if (arg === "--out") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--out requires a directory");
      }
      outDir = next;
      index += 1;
    } else if (arg === "--reference-root") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--reference-root requires a directory");
      }
      referenceRoot = next;
      index += 1;
    } else {
      throw new Error(`Unknown argument '${arg}'`);
    }
  }
  return {
    input,
    outDir,
    referenceRoot,
  };
}

function normalizeResourceLocation(value: string): string {
  const [namespace, resourcePath] = value.includes(":") ? value.split(":", 2) : ["minecraft", value];
  const effectivePath = resourcePath ?? "";
  return `${namespace || "minecraft"}:${effectivePath}`;
}

function normalizePath(value: string): string {
  return value.replace(/\\/gu, "/");
}

function normalizedRotation(value: number): number {
  return ((Math.trunc(value) % 360) + 360) % 360;
}

function isCatalogTiling(value: string | null): value is CatalogTiling {
  return value === "xy" || value === "x" || value === "none" || value === "unknown";
}

function isCatalogRotation(value: string | null): value is CatalogRotation {
  return (
    value === "free" ||
    value === "y90-safe" ||
    value === "y180-safe" ||
    value === "fixed" ||
    value === "model-driven" ||
    value === "unknown"
  );
}

function numberField(object: Record<string, unknown>, key: string, fallback: number): number {
  const value = object[key];
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function stringField(object: Record<string, unknown>, key: string): string | null {
  const value = object[key];
  return typeof value === "string" ? value : null;
}

function booleanField(object: Record<string, unknown>, key: string, fallback: boolean): boolean {
  const value = object[key];
  return typeof value === "boolean" ? value : fallback;
}

function asRecord(value: unknown): Record<string, unknown> {
  const object = asOptionalRecord(value);
  if (!object) {
    throw new Error("expected JSON object");
  }
  return object;
}

function asOptionalRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function tuple3(value: number[] | undefined, fallback: [number, number, number]): [number, number, number] {
  if (!value || value.length < 3) {
    return fallback;
  }
  return [finiteNumber(value[0], fallback[0]), finiteNumber(value[1], fallback[1]), finiteNumber(value[2], fallback[2])];
}

function tuple4(value: number[] | undefined, fallback: [number, number, number, number]): [number, number, number, number] {
  if (!value || value.length < 4) {
    return fallback;
  }
  return [
    finiteNumber(value[0], fallback[0]),
    finiteNumber(value[1], fallback[1]),
    finiteNumber(value[2], fallback[2]),
    finiteNumber(value[3], fallback[3]),
  ];
}

function finiteNumber(value: number | undefined, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function isFullCubeElement(from: [number, number, number], to: [number, number, number]): boolean {
  return from[0] === 0 && from[1] === 0 && from[2] === 0 && to[0] === 16 && to[1] === 16 && to[2] === 16;
}

function isFullUv(uv: [number, number, number, number]): boolean {
  return Math.abs(uv[0] - 0) < 0.001 && Math.abs(uv[1] - 0) < 0.001 && Math.abs(uv[2] - 16) < 0.001 && Math.abs(uv[3] - 16) < 0.001;
}

function uniqueSorted<T extends string | number>(values: T[]): T[] {
  return [...new Set(values)].sort((a, b) => String(a).localeCompare(String(b)));
}

function joinOrDash(values: (string | number)[]): string {
  return values.length > 0 ? values.join(", ") : "-";
}

function md(value: string): string {
  return value.replace(/\|/gu, "\\|");
}

function isPresent<T>(value: T | null | undefined): value is T {
  return value !== null && value !== undefined;
}

function priorityEntrySort(a: TextureCatalogEntry, b: TextureCatalogEntry): number {
  return priorityRank(a) - priorityRank(b) || a.texture.localeCompare(b.texture);
}

function priorityRank(entry: TextureCatalogEntry): number {
  if (entry.usageShapes.includes("cross-plant") && entry.tintIndexes.length > 0) {
    return 0;
  }
  if (entry.usageShapes.includes("cross-plant")) {
    return 1;
  }
  if (entry.tintIndexes.length > 0) {
    return 2;
  }
  if (entry.alpha.mode !== "opaque") {
    return 3;
  }
  if (entry.renderLayers.includes("cutout") || entry.renderLayers.includes("cutoutMipped")) {
    return 4;
  }
  return 5;
}

await main();
