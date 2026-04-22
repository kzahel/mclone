import { ResourceLocation } from "../../core/resource-location";
import type { BlockModelSource } from "./block-model-repository";

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object";
}

function collectModelLocations(value: unknown, output: ResourceLocation[]): void {
  if (Array.isArray(value)) {
    for (const entry of value) {
      collectModelLocations(entry, output);
    }
    return;
  }

  if (!isRecord(value) || typeof value.model !== "string") {
    return;
  }

  output.push(new ResourceLocation(value.model));
}

function getModelLocationsFromBlockStateJson(json: string): ResourceLocation[] {
  const parsed = JSON.parse(json) as unknown;
  if (!isRecord(parsed)) {
    return [];
  }

  const locations: ResourceLocation[] = [];
  if (isRecord(parsed.variants)) {
    for (const value of Object.values(parsed.variants)) {
      collectModelLocations(value, locations);
    }
  }

  if (Array.isArray(parsed.multipart)) {
    for (const entry of parsed.multipart) {
      if (isRecord(entry)) {
        collectModelLocations(entry.apply, locations);
      }
    }
  }

  return locations;
}

function getParentLocationFromModelJson(json: string): ResourceLocation | undefined {
  const parsed = JSON.parse(json) as unknown;
  if (!isRecord(parsed) || typeof parsed.parent !== "string" || parsed.parent.startsWith("builtin/")) {
    return undefined;
  }

  return new ResourceLocation(parsed.parent);
}

async function fetchJsonText(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Unable to load model asset ${url}: ${response.status} ${response.statusText}`);
  }

  return response.text();
}

export class PreloadedBlockModelSource implements BlockModelSource {
  public constructor(
    private readonly modelJsonByLocation: ReadonlyMap<string, string>,
    private readonly blockStateJsonByLocation: ReadonlyMap<string, string>,
  ) {}

  public getModelJson(location: ResourceLocation): string | undefined {
    return this.modelJsonByLocation.get(location.toString());
  }

  public getBlockStateJson(location: ResourceLocation): string | undefined {
    return this.blockStateJsonByLocation.get(location.toString());
  }
}

export async function preloadBlockModelSource(
  blockLocations: readonly ResourceLocation[],
  assetRoot = "/reference/minecraft-1.17.1/extracted/assets",
): Promise<PreloadedBlockModelSource> {
  const modelJsonByLocation = new Map<string, string>();
  const blockStateJsonByLocation = new Map<string, string>();
  const queuedModels = new Set<string>();
  const modelQueue: ResourceLocation[] = [];

  const enqueueModel = (location: ResourceLocation): void => {
    if (location.getPath().startsWith("builtin/")) {
      return;
    }

    const key = location.toString();
    if (queuedModels.has(key) || modelJsonByLocation.has(key)) {
      return;
    }

    queuedModels.add(key);
    modelQueue.push(location);
  };

  for (const blockLocation of blockLocations) {
    const blockStateJson = await fetchJsonText(
      `${assetRoot}/${blockLocation.getNamespace()}/blockstates/${blockLocation.getPath()}.json`,
    );
    blockStateJsonByLocation.set(blockLocation.toString(), blockStateJson);
    for (const modelLocation of getModelLocationsFromBlockStateJson(blockStateJson)) {
      enqueueModel(modelLocation);
    }
  }

  while (modelQueue.length !== 0) {
    const modelLocation = modelQueue.shift()!;
    const modelJson = await fetchJsonText(`${assetRoot}/${modelLocation.getNamespace()}/models/${modelLocation.getPath()}.json`);
    modelJsonByLocation.set(modelLocation.toString(), modelJson);
    const parentLocation = getParentLocationFromModelJson(modelJson);
    if (parentLocation !== undefined) {
      enqueueModel(parentLocation);
    }
  }

  return new PreloadedBlockModelSource(modelJsonByLocation, blockStateJsonByLocation);
}
