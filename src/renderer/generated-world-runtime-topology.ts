import type {
  NormalizedWorldEngineConfig,
  WorldEngineLightingMode,
  WorldEngineLiquidSimulationMode,
  WorldStorageMode,
} from "../runtime/protocol/world-messages";

export type GeneratedWorldRuntimeHost = "browser" | "deno" | "native" | "node" | "test";
export type GeneratedWorldRuntimeWorldHost = "worker" | "remote" | "in-process";
export type GeneratedWorldRuntimeRenderWorld = "worker";
export type GeneratedWorldRuntimeMeshTransport = "worker";
export type GeneratedWorldRuntimeLighting = "none" | "worker" | "remote";
export type GeneratedWorldRuntimeStorage = "indexeddb" | "file" | "memory" | "none" | "remote" | "default";
export type GeneratedWorldRuntimeAssetSource = "browser-asset-pack" | "file-asset-pack" | "test";
export type GeneratedWorldRuntimeRenderTarget = "canvas" | "offscreen-texture";

export interface GeneratedWorldRuntimeTopology {
  readonly host: GeneratedWorldRuntimeHost;
  readonly worldHost: GeneratedWorldRuntimeWorldHost;
  readonly renderWorld: GeneratedWorldRuntimeRenderWorld;
  readonly meshTransport: GeneratedWorldRuntimeMeshTransport;
  readonly lighting: GeneratedWorldRuntimeLighting;
  readonly liquidSimulation: WorldEngineLiquidSimulationMode;
  readonly storage: GeneratedWorldRuntimeStorage;
  readonly assetSource: GeneratedWorldRuntimeAssetSource;
  readonly renderTarget: GeneratedWorldRuntimeRenderTarget;
}

export interface GeneratedWorldRuntimeTopologyRequirement {
  readonly worldHost?: GeneratedWorldRuntimeWorldHost;
  readonly renderWorld?: GeneratedWorldRuntimeRenderWorld;
  readonly meshTransport?: GeneratedWorldRuntimeMeshTransport;
  readonly lighting?: GeneratedWorldRuntimeLighting;
  readonly liquidSimulation?: WorldEngineLiquidSimulationMode;
}

export interface CreateGeneratedWorldRuntimeTopologyOptions {
  readonly host: GeneratedWorldRuntimeHost;
  readonly worldHost: GeneratedWorldRuntimeWorldHost;
  readonly lighting: GeneratedWorldRuntimeLighting;
  readonly liquidSimulation: WorldEngineLiquidSimulationMode;
  readonly storage: GeneratedWorldRuntimeStorage;
  readonly assetSource: GeneratedWorldRuntimeAssetSource;
  readonly renderTarget: GeneratedWorldRuntimeRenderTarget;
}

export function createGeneratedWorldRuntimeTopology(
  options: CreateGeneratedWorldRuntimeTopologyOptions,
): GeneratedWorldRuntimeTopology {
  return {
    host: options.host,
    worldHost: options.worldHost,
    renderWorld: "worker",
    meshTransport: "worker",
    lighting: options.lighting,
    liquidSimulation: options.liquidSimulation,
    storage: options.storage,
    assetSource: options.assetSource,
    renderTarget: options.renderTarget,
  };
}

export function getGeneratedWorldRuntimeTopologyRequirement(
  engineConfig: NormalizedWorldEngineConfig,
  worldHost?: GeneratedWorldRuntimeWorldHost,
): GeneratedWorldRuntimeTopologyRequirement {
  return {
    ...(worldHost === undefined ? {} : { worldHost }),
    renderWorld: "worker",
    meshTransport: "worker",
    lighting: getRequiredGeneratedWorldLightingTopology(engineConfig.lightingMode, worldHost),
    liquidSimulation: engineConfig.liquidSimulationMode,
  };
}

export function worldTransportToRuntimeWorldHost(
  worldTransport: "worker" | "remote",
): GeneratedWorldRuntimeWorldHost {
  return worldTransport;
}

export function resolveBrowserGeneratedWorldStorageTopology(
  worldHost: GeneratedWorldRuntimeWorldHost,
  storageMode: WorldStorageMode,
): GeneratedWorldRuntimeStorage {
  if (worldHost === "remote") {
    return "remote";
  }

  return storageMode === "none" ? "none" : "indexeddb";
}

export function resolveGeneratedWorldLightingTopology(
  lightingMode: WorldEngineLightingMode,
  worldHost: GeneratedWorldRuntimeWorldHost,
): GeneratedWorldRuntimeLighting {
  if (lightingMode === "none") {
    return "none";
  }

  return worldHost === "remote" ? "remote" : "worker";
}

export function validateGeneratedWorldRuntimeTopology(
  topology: GeneratedWorldRuntimeTopology | undefined,
  requirement: GeneratedWorldRuntimeTopologyRequirement,
  label = "topology",
): string[] {
  if (topology === undefined) {
    return [`expected ${label}`];
  }

  const errors: string[] = [];
  pushMismatch(errors, label, "worldHost", topology.worldHost, requirement.worldHost);
  pushMismatch(errors, label, "renderWorld", topology.renderWorld, requirement.renderWorld);
  pushMismatch(errors, label, "meshTransport", topology.meshTransport, requirement.meshTransport);
  pushMismatch(errors, label, "lighting", topology.lighting, requirement.lighting);
  pushMismatch(errors, label, "liquidSimulation", topology.liquidSimulation, requirement.liquidSimulation);
  return errors;
}

function getRequiredGeneratedWorldLightingTopology(
  lightingMode: WorldEngineLightingMode,
  worldHost: GeneratedWorldRuntimeWorldHost | undefined,
): GeneratedWorldRuntimeLighting {
  if (lightingMode === "none") {
    return "none";
  }

  return worldHost === "remote" ? "remote" : "worker";
}

function pushMismatch<T extends string>(
  errors: string[],
  label: string,
  field: string,
  actual: T,
  expected: T | undefined,
): void {
  if (expected !== undefined && actual !== expected) {
    errors.push(`expected ${label}.${field}=${expected}, got ${actual}`);
  }
}
