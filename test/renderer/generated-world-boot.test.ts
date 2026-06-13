import { describe, expect, test } from "vitest";
import {
  GENERATED_WORLD_SMOKE_SCENARIO,
  GENERATED_WORLD_TRANSITION_SCENARIO,
  GENERATED_WORLD_VANILLA_LIGHTING_SCENARIO,
} from "../../src/renderer/generated-world-smoke-scenario";
import {
  type GeneratedWorldBootAdapter,
  runGeneratedWorldBoot,
  resolveGeneratedWorldBootCamera,
} from "../../src/renderer/generated-world-boot";
import {
  createGeneratedWorldBrowserBootAdapter,
  resolveGeneratedWorldBrowserCamera,
} from "../../src/renderer/generated-world-browser-boot";
import { createGeneratedWorldHeadlessBootAdapter } from "../../src/renderer/generated-world-headless-boot";
import { createGeneratedWorldRuntimeTopology } from "../../src/renderer/generated-world-runtime-topology";
import type { GeneratedWorldSmokePresentationHost } from "../../src/renderer/generated-world-smoke-runner";
import type { BrowserRenderConfig } from "../../src/renderer/browser-render-config";
import type { AssetPack } from "../../src/renderer/assets/asset-pack";
import type { HeadlessRendererHost } from "../../src/renderer/renderer-host";
import type { WorldTransport } from "../../src/runtime/transport/local-world-transport";

describe("generated-world boot adapter", () => {
  test("uses the final scenario camera unless a one-step override is provided", () => {
    expect(resolveGeneratedWorldBootCamera(GENERATED_WORLD_TRANSITION_SCENARIO)).toBe(
      GENERATED_WORLD_TRANSITION_SCENARIO.steps[1]!.camera,
    );
    expect(resolveGeneratedWorldBootCamera(
      GENERATED_WORLD_SMOKE_SCENARIO,
      GENERATED_WORLD_TRANSITION_SCENARIO.steps[1]!.camera,
    )).toBe(GENERATED_WORLD_TRANSITION_SCENARIO.steps[1]!.camera);
  });

  test("browser camera override remains limited to one-step smoke scenarios", () => {
    const override = GENERATED_WORLD_TRANSITION_SCENARIO.steps[1]!.camera;
    expect(resolveGeneratedWorldBrowserCamera(1, GENERATED_WORLD_SMOKE_SCENARIO.camera, override)).toBe(override);
    expect(resolveGeneratedWorldBrowserCamera(2, GENERATED_WORLD_TRANSITION_SCENARIO.camera, override)).toBeUndefined();
  });

  test("browser boot adapter declares its local worker topology", () => {
    const adapter = createGeneratedWorldBrowserBootAdapter({
      canvas: {} as HTMLCanvasElement,
      renderConfig: makeBrowserRenderConfig(),
      runtimeConfig: { worldTransport: "worker" },
      readbackFormat: "rgba8unorm",
    });

    expect(adapter.describeTopology({ scenario: GENERATED_WORLD_SMOKE_SCENARIO })).toEqual({
      host: "browser",
      worldHost: "worker",
      renderWorld: "worker",
      meshTransport: "worker",
      lighting: "none",
      liquidSimulation: "none",
      storage: "none",
      assetSource: "browser-asset-pack",
      renderTarget: "canvas",
    });
  });

  test("browser boot adapter declares remote vanilla lighting topology", () => {
    const adapter = createGeneratedWorldBrowserBootAdapter({
      canvas: {} as HTMLCanvasElement,
      renderConfig: makeBrowserRenderConfig({
        lightingMode: "vanilla17",
        liquidSimulationMode: "none",
      }),
      runtimeConfig: {
        worldTransport: "remote",
        remoteWorldHostUrl: "http://127.0.0.1:4173",
      },
      readbackFormat: "rgba8unorm",
    });

    expect(adapter.describeTopology({ scenario: GENERATED_WORLD_VANILLA_LIGHTING_SCENARIO })).toEqual({
      host: "browser",
      worldHost: "remote",
      renderWorld: "worker",
      meshTransport: "worker",
      lighting: "remote",
      liquidSimulation: "none",
      storage: "remote",
      assetSource: "browser-asset-pack",
      renderTarget: "canvas",
    });
  });

  test("headless boot adapter can declare worker-backed vanilla lighting", () => {
    const vanillaLightingScenario = {
      ...GENERATED_WORLD_SMOKE_SCENARIO,
      engineConfig: {
        lightingMode: "vanilla17",
        liquidSimulationMode: "none",
      },
    } as const;
    const adapter = createGeneratedWorldHeadlessBootAdapter({
      gpuAdapter: {} as GPUAdapter,
      device: {} as GPUDevice,
      format: "rgba8unorm",
      rendererHost: {} as HeadlessRendererHost,
      worldTransport: {} as WorldTransport,
      host: "deno",
      worldHost: "worker",
      lighting: "worker",
      storage: "none",
      assetPack: {} as AssetPack,
    });

    expect(adapter.describeTopology({ scenario: vanillaLightingScenario })).toMatchObject({
      host: "deno",
      worldHost: "worker",
      renderWorld: "worker",
      meshTransport: "worker",
      lighting: "worker",
      liquidSimulation: "none",
      storage: "none",
      assetSource: "file-asset-pack",
      renderTarget: "offscreen-texture",
    });
  });

  test("boot rejects a topology that cannot satisfy a vanilla lighting scenario", async () => {
    let createSceneCalled = false;
    const vanillaLightingScenario = {
      ...GENERATED_WORLD_SMOKE_SCENARIO,
      engineConfig: {
        lightingMode: "vanilla17",
        liquidSimulationMode: "none",
      },
    } as const;
    const adapter: GeneratedWorldBootAdapter = {
      describeTopology: () => createGeneratedWorldRuntimeTopology({
        host: "deno",
        worldHost: "worker",
        renderTarget: "offscreen-texture",
        assetSource: "file-asset-pack",
        storage: "none",
        lighting: "none",
        liquidSimulation: "none",
      }),
      createScene: async () => {
        createSceneCalled = true;
        return { ok: false, reason: "scene should not be created" };
      },
      createPresentationHost: () => ({
        target: {
          width: 1,
          height: 1,
          format: "rgba8unorm",
          renderFrame: async () => ({ width: 1, height: 1, format: "rgba8unorm" }),
        },
      }) satisfies GeneratedWorldSmokePresentationHost,
    };

    const result = await runGeneratedWorldBoot({
      scenario: vanillaLightingScenario,
      adapter,
      worldTransport: "worker",
    });

    expect(result.ok).toBe(false);
    if (result.ok) {
      throw new Error("expected topology mismatch");
    }
    expect(createSceneCalled).toBe(false);
    expect(result.reason).toContain("generated-world boot topology mismatch");
    expect(result.reason).toContain("expected topology.lighting=worker, got none");
  });
});

function makeBrowserRenderConfig(overrides: Partial<BrowserRenderConfig> = {}): BrowserRenderConfig {
  return {
    viewDistance: GENERATED_WORLD_SMOKE_SCENARIO.viewDistance,
    renderDistance: GENERATED_WORLD_SMOKE_SCENARIO.renderDistance,
    fogEnabled: true,
    skyColor: GENERATED_WORLD_SMOKE_SCENARIO.skyColor,
    clearColorScale: GENERATED_WORLD_SMOKE_SCENARIO.clearColorScale,
    lightingMode: GENERATED_WORLD_SMOKE_SCENARIO.engineConfig.lightingMode,
    liquidSimulationMode: GENERATED_WORLD_SMOKE_SCENARIO.engineConfig.liquidSimulationMode,
    worldStorageMode: "none",
    autoJump: true,
    ...overrides,
  };
}
