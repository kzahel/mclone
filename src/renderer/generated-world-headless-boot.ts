import type { AssetPack } from "./assets/asset-pack";
import {
  createGeneratedWorldHeadlessHarness,
  type GeneratedWorldHeadlessHarness,
} from "./generated-world-headless-harness";
import { createGeneratedWorldHeadlessPresentationHost } from "./generated-world-headless-presentation-host";
import {
  type GeneratedWorldBootAdapter,
  type GeneratedWorldBootSceneContext,
  type GeneratedWorldBootSceneResult,
} from "./generated-world-boot";
import type { GeneratedWorldSmokePngArtifact } from "./generated-world-headless-presentation-host";
import type { HeadlessRendererHost } from "./renderer-host";
import type { WorldTransport } from "../runtime/transport/local-world-transport";
import {
  createGeneratedWorldRuntimeTopology,
  type GeneratedWorldRuntimeHost,
  type GeneratedWorldRuntimeLighting,
  type GeneratedWorldRuntimeStorage,
  type GeneratedWorldRuntimeWorldHost,
} from "./generated-world-runtime-topology";

export interface GeneratedWorldHeadlessBootAdapterOptions {
  readonly gpuAdapter: GPUAdapter;
  readonly device: GPUDevice;
  readonly format: GPUTextureFormat;
  readonly rendererHost: HeadlessRendererHost;
  readonly worldTransport: WorldTransport;
  readonly host: GeneratedWorldRuntimeHost;
  readonly worldHost: GeneratedWorldRuntimeWorldHost;
  readonly lighting?: GeneratedWorldRuntimeLighting;
  readonly storage?: GeneratedWorldRuntimeStorage;
  readonly assetPack: AssetPack;
  readonly writePngArtifact?: (artifact: GeneratedWorldSmokePngArtifact) => Promise<void>;
}

export function createGeneratedWorldHeadlessBootAdapter(
  options: GeneratedWorldHeadlessBootAdapterOptions,
): GeneratedWorldBootAdapter {
  return {
    describeTopology: ({ scenario }) => createGeneratedWorldRuntimeTopology({
      host: options.host,
      worldHost: options.worldHost,
      lighting: options.lighting ?? "none",
      liquidSimulation: scenario.engineConfig.liquidSimulationMode,
      storage: options.storage ?? "none",
      assetSource: "file-asset-pack",
      renderTarget: "offscreen-texture",
    }),
    createScene: (context) => createGeneratedWorldHeadlessScene(options, context),
    createPresentationHost: ({ scenario }) => createGeneratedWorldHeadlessPresentationHost({
      rendererHost: options.rendererHost,
      width: scenario.width,
      height: scenario.height,
      format: scenario.renderTargetFormat,
      writePngArtifact: options.writePngArtifact,
    }),
  };
}

async function createGeneratedWorldHeadlessScene(
  options: GeneratedWorldHeadlessBootAdapterOptions,
  context: GeneratedWorldBootSceneContext,
): Promise<GeneratedWorldBootSceneResult> {
  let harness: GeneratedWorldHeadlessHarness | undefined;
  try {
    harness = await createGeneratedWorldHeadlessHarness({
      adapter: options.gpuAdapter,
      device: options.device,
      format: options.format,
      rendererHost: options.rendererHost,
      worldTransport: options.worldTransport,
      assetPack: options.assetPack,
      seed: context.scenario.seed,
      width: context.scenario.width,
      height: context.scenario.height,
      viewDistance: context.scenario.viewDistance,
      renderDistance: context.scenario.renderDistance,
      preset: context.scenario.preset,
      engineConfig: context.scenario.engineConfig,
      skyColor: context.scenario.skyColor,
      clearColorScale: context.scenario.clearColorScale,
      onProgress: context.onProgress,
    });
    return {
      ok: true,
      scene: harness.scene,
      close: () => harness?.close(),
    };
  } catch (error) {
    harness?.close();
    return {
      ok: false,
      reason: error instanceof Error ? error.message : String(error),
    };
  }
}
