import type { LoadingProgressSink } from "./loading-progress";
import type { CameraState } from "./game-renderer";
import type { RendererScene } from "./scene-setup";
import {
  GENERATED_WORLD_SMOKE_SCENARIO,
  type GeneratedWorldSmokeScenario,
} from "./generated-world-smoke-scenario";
import {
  runGeneratedWorldSmokeScenario,
  type GeneratedWorldSmokePresentationHost,
  type GeneratedWorldSmokeRun,
  type GeneratedWorldSmokeRunOptions,
  type GeneratedWorldSmokeWorldTransport,
} from "./generated-world-smoke-runner";

export interface GeneratedWorldBootSceneContext {
  readonly scenario: GeneratedWorldSmokeScenario;
  readonly onProgress?: LoadingProgressSink;
}

export type GeneratedWorldBootSceneResult =
  | {
    readonly ok: true;
    readonly scene: RendererScene;
    close?(): Promise<void> | void;
  }
  | { readonly ok: false; readonly reason: string };

export interface GeneratedWorldBootPresentationContext {
  readonly scenario: GeneratedWorldSmokeScenario;
  readonly scene: RendererScene;
}

export interface GeneratedWorldBootAdapter {
  createScene(context: GeneratedWorldBootSceneContext): Promise<GeneratedWorldBootSceneResult>;
  createPresentationHost(context: GeneratedWorldBootPresentationContext): Promise<GeneratedWorldSmokePresentationHost> | GeneratedWorldSmokePresentationHost;
}

export interface GeneratedWorldBootOptions extends Omit<GeneratedWorldSmokeRunOptions, "scene" | "presentationHost"> {
  readonly adapter: GeneratedWorldBootAdapter;
}

export interface GeneratedWorldBootRun {
  readonly scene: RendererScene;
  readonly run: GeneratedWorldSmokeRun;
  readonly camera: CameraState;
  close(): Promise<void>;
}

export type GeneratedWorldBootResult =
  | ({ readonly ok: true } & GeneratedWorldBootRun)
  | { readonly ok: false; readonly reason: string };

export function resolveGeneratedWorldBootCamera(
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
  cameraOverride?: CameraState,
): CameraState {
  if (cameraOverride !== undefined) {
    return cameraOverride;
  }

  return scenario.steps[scenario.steps.length - 1]?.camera ?? scenario.camera;
}

export async function runGeneratedWorldBoot(
  options: GeneratedWorldBootOptions,
): Promise<GeneratedWorldBootResult> {
  const scenario = options.scenario ?? GENERATED_WORLD_SMOKE_SCENARIO;
  const sceneResult = await options.adapter.createScene({
    scenario,
    onProgress: options.onProgress,
  });
  if (!sceneResult.ok) {
    return sceneResult;
  }

  const closeScene = async (): Promise<void> => {
    await sceneResult.close?.();
  };

  try {
    const presentationHost = await options.adapter.createPresentationHost({
      scenario,
      scene: sceneResult.scene,
    });
    const run = await runGeneratedWorldSmokeScenario({
      ...options,
      scenario,
      scene: sceneResult.scene,
      presentationHost,
      format: options.format,
    });

    return {
      ok: true,
      scene: sceneResult.scene,
      run,
      camera: resolveGeneratedWorldBootCamera(scenario, options.camera),
      close: closeScene,
    };
  } catch (error) {
    await closeScene();
    return {
      ok: false,
      reason: error instanceof Error ? error.message : String(error),
    };
  }
}

export type { GeneratedWorldSmokeWorldTransport };
