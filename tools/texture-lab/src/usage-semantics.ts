import fs from "node:fs/promises";
import path from "node:path";
import { referenceCounterpart, referenceRoots } from "./reference";
import type { TextureVanillaUsage, TextureVanillaUseSummary } from "./core/index-model";

type RenderLayer = "solid" | "cutout" | "cutoutMipped" | "translucent" | "tripwire" | "fluid" | "unknown";
type GeometryKind =
  | "cube"
  | "cross-sprite"
  | "crop-cross"
  | "flat-ground"
  | "pane"
  | "torch"
  | "door"
  | "trapdoor"
  | "rail"
  | "wall"
  | "fence"
  | "slab"
  | "stairs"
  | "button"
  | "pressure-plate"
  | "candle"
  | "partial-model"
  | "fluid"
  | "unknown";

interface UsageSemanticsIndex {
  referenceRoot: string | null;
  byTexture: Map<string, TextureVanillaUsage>;
  warnings: string[];
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
  faces: Record<string, EffectiveFace>;
}

interface EffectiveFace {
  texture: string;
  cullface: string | null;
  tintindex: number;
  uv: [number, number, number, number];
  uvRotation: number;
}

interface TextureUse {
  block: string;
  blockstatePath: string;
  selector: string;
  model: string;
  modelFamilies: string[];
  geometryKind: GeometryKind;
  renderLayer: RenderLayer;
  role: "face" | "particle";
  textureSlot: string | null;
  face: string | null;
  tintindex: number | null;
}

const JAVA_RENDER_TYPES_PATH = path.join("net", "minecraft", "client", "renderer", "ItemBlockRenderTypes.java");
const FLUID_TEXTURES = new Set([
  "minecraft:block/water_still",
  "minecraft:block/water_flow",
  "minecraft:block/lava_still",
  "minecraft:block/lava_flow",
]);

export async function buildVanillaUsageSemantics(referenceRoot?: string): Promise<UsageSemanticsIndex> {
  const root = await resolveReferenceRoot(referenceRoot);
  if (!root) {
    return {
      referenceRoot: null,
      byTexture: new Map(),
      warnings: ["Vanilla usage semantics unavailable: no local Minecraft reference assets found."],
    };
  }

  const [modelIndex, blockstateUses, renderLayersByBlock] = await Promise.all([
    ModelIndex.load(root),
    loadBlockStateModelUses(root),
    loadRenderLayersByBlock(root),
  ]);
  const usesByTexture = collectTextureUses(modelIndex, blockstateUses, renderLayersByBlock);
  const byTexture = new Map<string, TextureVanillaUsage>();
  for (const [texture, uses] of usesByTexture) {
    byTexture.set(texture, usageEntry(texture, uses));
  }
  for (const fluid of FLUID_TEXTURES) {
    if (!byTexture.has(fluid)) {
      byTexture.set(fluid, usageEntry(fluid, []));
    }
  }
  return { referenceRoot: root, byTexture, warnings: [] };
}

export function vanillaTextureResourceForExportPath(exportPath: string): string | null {
  const counterpart = referenceCounterpart(exportPath);
  if (!counterpart || counterpart.kind !== "single") {
    return null;
  }
  const prefix = "assets/minecraft/textures/block/";
  if (!counterpart.path.startsWith(prefix) || !counterpart.path.endsWith(".png")) {
    return null;
  }
  return `minecraft:block/${counterpart.path.slice(prefix.length, -".png".length)}`;
}

function usageEntry(texture: string, uses: TextureUse[]): TextureVanillaUsage {
  const geometryKinds = uniqueSorted([
    ...uses.map((use) => use.geometryKind),
    ...(FLUID_TEXTURES.has(texture) ? ["fluid" as const] : []),
  ]);
  const modelFamilies = uniqueSorted(uses.flatMap((use) => use.modelFamilies));
  const renderLayers = uniqueSorted([
    ...uses.map((use) => use.renderLayer),
    ...(FLUID_TEXTURES.has(texture) ? ["fluid" as const] : []),
  ]);
  const textureSlots = uniqueSorted(uses.flatMap((use) => (use.textureSlot ? [use.textureSlot] : [])));
  const tintIndexes = uniqueSorted(uses.flatMap((use) => (use.tintindex !== null && use.tintindex >= 0 ? [use.tintindex] : [])));
  const tintRoles = uniqueSorted(uses.flatMap(inferTintRolesForUse));
  const blocks = uniqueSorted(uses.map((use) => use.block));
  return {
    texture,
    blockCount: blocks.length,
    useCount: uses.length,
    geometryKinds,
    modelFamilies,
    renderLayers,
    textureSlots,
    tintIndexes,
    tintRoles,
    previewHint: previewHintFor(geometryKinds),
    authoringNotes: authoringNotesFor(geometryKinds, renderLayers, tintRoles),
    uses: summarizeUses(uses),
  };
}

function summarizeUses(uses: TextureUse[]): TextureVanillaUseSummary[] {
  const byKey = new Map<string, TextureVanillaUseSummary>();
  for (const use of uses) {
    const key = [
      use.block,
      use.model,
      use.role,
      use.textureSlot ?? "",
      use.face ?? "",
      use.geometryKind,
      use.renderLayer,
      use.selector,
    ].join("|");
    if (byKey.has(key)) {
      continue;
    }
    byKey.set(key, {
      block: use.block,
      selector: use.selector,
      model: use.model,
      role: use.role,
      textureSlot: use.textureSlot,
      face: use.face,
      geometryKind: use.geometryKind,
      renderLayer: use.renderLayer,
      tintindex: use.tintindex,
    });
  }
  return [...byKey.values()].sort(compareUseSummary);
}

function compareUseSummary(left: TextureVanillaUseSummary, right: TextureVanillaUseSummary): number {
  return (
    left.block.localeCompare(right.block) ||
    left.geometryKind.localeCompare(right.geometryKind) ||
    left.model.localeCompare(right.model) ||
    (left.face ?? "").localeCompare(right.face ?? "") ||
    left.selector.localeCompare(right.selector)
  );
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

function collectTextureUses(
  modelIndex: ModelIndex,
  blockstateUses: BlockStateModelUse[],
  renderLayersByBlock: Map<string, RenderLayer>,
): Map<string, TextureUse[]> {
  const usesByTexture = new Map<string, TextureUse[]>();
  for (const blockstateUse of blockstateUses) {
    const modelFamilies = modelIndex.modelFamilies(blockstateUse.model);
    const geometryKind = geometryKindFromFamilies(modelFamilies);
    const renderLayer = renderLayersByBlock.get(blockstateUse.block) ?? "solid";
    const particle = modelIndex.resolveTexture(blockstateUse.model, "particle");
    if (particle?.startsWith("minecraft:block/")) {
      pushUse(usesByTexture, particle, {
        block: blockstateUse.block,
        blockstatePath: blockstateUse.blockstatePath,
        selector: blockstateUse.selector,
        model: blockstateUse.model,
        modelFamilies,
        geometryKind,
        renderLayer,
        role: "particle",
        textureSlot: "particle",
        face: null,
        tintindex: null,
      });
    }
    for (const element of modelIndex.effectiveElements(blockstateUse.model)) {
      for (const [direction, face] of Object.entries(element.faces)) {
        const texture = modelIndex.resolveTexture(blockstateUse.model, face.texture);
        if (!texture?.startsWith("minecraft:block/")) {
          continue;
        }
        pushUse(usesByTexture, texture, {
          block: blockstateUse.block,
          blockstatePath: blockstateUse.blockstatePath,
          selector: blockstateUse.selector,
          model: blockstateUse.model,
          modelFamilies,
          geometryKind: geometryKindForFace(geometryKind, element, direction),
          renderLayer,
          role: "face",
          textureSlot: textureSlotName(face.texture),
          face: direction,
          tintindex: face.tintindex >= 0 ? face.tintindex : null,
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
      models.set(`minecraft:block/${modelPath}`, JSON.parse(await fs.readFile(file, "utf8")) as RawBlockModel);
    }
    return new ModelIndex(models);
  }

  modelFamilies(location: string): string[] {
    const families: string[] = [];
    const seen = new Set<string>();
    let current: string | undefined = location;
    while (current && !seen.has(current)) {
      seen.add(current);
      const name = resourcePath(current).replace(/^block\//u, "");
      families.push(name);
      const model = this.models.get(current);
      current = model?.parent ? normalizeResourceLocation(model.parent) : undefined;
    }
    return families;
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
    if (!face.texture) {
      continue;
    }
    faces[direction] = {
      texture: face.texture,
      cullface: face.cullface ?? null,
      tintindex: numberField(face as Record<string, unknown>, "tintindex", -1),
      uv: tuple4(face.uv, defaultFaceUv(direction, from, to)),
      uvRotation: normalizedRotation(numberField(face as Record<string, unknown>, "rotation", 0)),
    };
  }
  return { from, to, faces };
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
    layers.set(`minecraft:${blockName.toLowerCase()}`, vars.get(variable) ?? "unknown");
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

function geometryKindFromFamilies(families: string[]): GeometryKind {
  if (families.some((family) => family === "cross" || family === "tinted_cross" || family === "flower_pot_cross" || family === "tinted_flower_pot_cross")) {
    return "cross-sprite";
  }
  if (families.includes("crop")) {
    return "crop-cross";
  }
  if (families.some((family) => family === "rail_flat" || family.startsWith("template_rail_"))) {
    return "rail";
  }
  if (families.some((family) => family.includes("glass_pane") || family.includes("pane"))) {
    return "pane";
  }
  if (families.some((family) => family.startsWith("template_torch"))) {
    return "torch";
  }
  if (families.some((family) => family.startsWith("door_"))) {
    return "door";
  }
  if (families.some((family) => family.includes("trapdoor"))) {
    return "trapdoor";
  }
  if (families.some((family) => family.startsWith("template_wall") || family === "wall_inventory")) {
    return "wall";
  }
  if (families.some((family) => family.startsWith("fence_") || family.startsWith("template_fence"))) {
    return "fence";
  }
  if (families.some((family) => family.includes("slab"))) {
    return "slab";
  }
  if (families.some((family) => family.includes("stairs"))) {
    return "stairs";
  }
  if (families.some((family) => family.startsWith("button"))) {
    return "button";
  }
  if (families.some((family) => family.startsWith("pressure_plate"))) {
    return "pressure-plate";
  }
  if (families.some((family) => family.includes("candle"))) {
    return "candle";
  }
  if (families.some((family) => family.startsWith("cube") || family === "orientable" || family === "orientable_with_bottom" || family === "leaves")) {
    return "cube";
  }
  if (families.length === 0 || families.every((family) => family === "missing")) {
    return "unknown";
  }
  return "partial-model";
}

function geometryKindForFace(base: GeometryKind, element: EffectiveElement, direction: string): GeometryKind {
  if (base === "rail" || isHorizontalPlane(element)) {
    return "flat-ground";
  }
  if (base === "cube" && !isFullCubeElement(element.from, element.to)) {
    return "partial-model";
  }
  if ((direction === "up" || direction === "down") && isHorizontalPlane(element)) {
    return "flat-ground";
  }
  return base;
}

function isHorizontalPlane(element: EffectiveElement): boolean {
  return element.from[1] === element.to[1] && element.from[0] === 0 && element.from[2] === 0 && element.to[0] === 16 && element.to[2] === 16;
}

function isFullCubeElement(from: [number, number, number], to: [number, number, number]): boolean {
  return from[0] === 0 && from[1] === 0 && from[2] === 0 && to[0] === 16 && to[1] === 16 && to[2] === 16;
}

function previewHintFor(geometryKinds: GeometryKind[]): TextureVanillaUsage["previewHint"] {
  if (geometryKinds.includes("cross-sprite") || geometryKinds.includes("crop-cross")) {
    return "cross";
  }
  if (geometryKinds.includes("flat-ground") || geometryKinds.includes("rail")) {
    return "flat";
  }
  if (geometryKinds.includes("cube")) {
    return "cube";
  }
  if (geometryKinds.includes("door") || geometryKinds.includes("trapdoor") || geometryKinds.includes("pane")) {
    return "partial";
  }
  if (geometryKinds.includes("fluid")) {
    return "fluid";
  }
  return "unknown";
}

function authoringNotesFor(
  geometryKinds: GeometryKind[],
  renderLayers: RenderLayer[],
  tintRoles: string[],
): string[] {
  const notes: string[] = [];
  if (geometryKinds.includes("cross-sprite") || geometryKinds.includes("crop-cross")) {
    notes.push("center alpha mass on the crossed planes");
    notes.push("judge silhouette in cross-plane preview, not cube preview");
  }
  if (geometryKinds.includes("flat-ground") || geometryKinds.includes("rail")) {
    notes.push("author as a horizontal ground-plane cutout");
  }
  if (geometryKinds.includes("cube")) {
    notes.push("check tile seams and mip readability on cube faces");
  }
  if (renderLayers.some((layer) => layer === "cutout" || layer === "cutoutMipped")) {
    notes.push("alpha should be intentional cutout coverage");
  }
  if (tintRoles.length > 0) {
    notes.push(`tint-driven in vanilla: ${tintRoles.join(", ")}`);
  }
  return notes;
}

function inferTintRolesForUse(use: TextureUse): string[] {
  if (use.tintindex === null || use.tintindex < 0) {
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
  return [`tintindex:${use.tintindex}`];
}

function textureSlotName(texture: string): string | null {
  return texture.startsWith("#") ? texture.slice(1) : null;
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

async function resolveReferenceRoot(referenceRoot: string | undefined): Promise<string | null> {
  for (const candidate of referenceRoots(referenceRoot)) {
    try {
      const assetsRoot = path.join(candidate, "assets", "minecraft");
      const modelRoot = path.join(assetsRoot, "models", "block");
      const blockstateRoot = path.join(assetsRoot, "blockstates");
      const [assetsStat, modelStat, blockstateStat] = await Promise.all([
        fs.stat(assetsRoot),
        fs.stat(modelRoot),
        fs.stat(blockstateRoot),
      ]);
      if (assetsStat.isDirectory() && modelStat.isDirectory() && blockstateStat.isDirectory()) {
        return candidate;
      }
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
        throw error;
      }
    }
  }
  return null;
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
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
        throw error;
      }
    }
  }
  return null;
}

function normalizeResourceLocation(value: string): string {
  const [namespace, resourcePath] = value.includes(":") ? value.split(":", 2) : ["minecraft", value];
  return `${namespace || "minecraft"}:${resourcePath ?? ""}`;
}

function resourcePath(location: string): string {
  return location.split(":")[1] ?? location;
}

function normalizePath(value: string): string {
  return value.replace(/\\/gu, "/");
}

function normalizedRotation(value: number): number {
  return ((Math.trunc(value) % 360) + 360) % 360;
}

function tuple3(value: unknown, fallback: [number, number, number]): [number, number, number] {
  return Array.isArray(value) && value.length >= 3
    ? [numberAt(value, 0, fallback[0]), numberAt(value, 1, fallback[1]), numberAt(value, 2, fallback[2])]
    : fallback;
}

function tuple4(value: unknown, fallback: [number, number, number, number]): [number, number, number, number] {
  return Array.isArray(value) && value.length >= 4
    ? [numberAt(value, 0, fallback[0]), numberAt(value, 1, fallback[1]), numberAt(value, 2, fallback[2]), numberAt(value, 3, fallback[3])]
    : fallback;
}

function numberAt(values: unknown[], index: number, fallback: number): number {
  const value = values[index];
  return typeof value === "number" ? value : fallback;
}

function numberField(object: Record<string, unknown>, key: string, fallback: number): number {
  return typeof object[key] === "number" ? object[key] : fallback;
}

function stringField(object: Record<string, unknown>, key: string): string | null {
  return typeof object[key] === "string" ? object[key] : null;
}

function booleanField(object: Record<string, unknown>, key: string, fallback: boolean): boolean {
  return typeof object[key] === "boolean" ? object[key] : fallback;
}

function asRecord(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return {};
  }
  return value as Record<string, unknown>;
}

function asOptionalRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function isPresent<T>(value: T | null | undefined): value is T {
  return value !== null && value !== undefined;
}

function uniqueSorted<T extends string | number>(values: T[]): T[] {
  return [...new Set(values)].sort((left, right) => String(left).localeCompare(String(right)));
}
