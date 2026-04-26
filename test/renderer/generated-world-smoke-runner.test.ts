import { describe, expect, test } from "vitest";
import {
  GENERATED_WORLD_SMOKE_SCENARIO,
  GENERATED_WORLD_TICK_CADENCE_SCENARIO,
} from "../../src/renderer/generated-world-smoke-scenario";
import {
  createGeneratedWorldSmokePresentationHost,
  getGeneratedWorldSmokePresentationDelayMs,
  type GeneratedWorldSmokeRenderTarget,
} from "../../src/renderer/generated-world-smoke-runner";

describe("generated-world smoke runner presentation host", () => {
  test("resolves default and cadence presentation delays", () => {
    expect(getGeneratedWorldSmokePresentationDelayMs(
      GENERATED_WORLD_SMOKE_SCENARIO,
      GENERATED_WORLD_SMOKE_SCENARIO.steps[0]!,
    )).toBe(0);
    expect(getGeneratedWorldSmokePresentationDelayMs(
      GENERATED_WORLD_TICK_CADENCE_SCENARIO,
      GENERATED_WORLD_TICK_CADENCE_SCENARIO.steps[0]!,
    )).toBe(50);
    expect(getGeneratedWorldSmokePresentationDelayMs(
      GENERATED_WORLD_TICK_CADENCE_SCENARIO,
      {
        ...GENERATED_WORLD_TICK_CADENCE_SCENARIO.steps[0]!,
        presentationDelayMs: 75,
      },
    )).toBe(75);
  });

  test("presentation host factory preserves target and lifecycle hooks", () => {
    const target: GeneratedWorldSmokeRenderTarget = {
      width: 1,
      height: 1,
      format: "rgba8unorm",
      renderFrame: async () => ({
        width: 1,
        height: 1,
        format: "rgba8unorm",
      }),
    };
    const waitForPresentationDelay = async (): Promise<void> => {};
    const writeStepArtifact = async (): Promise<void> => {};
    const close = (): void => {};

    const host = createGeneratedWorldSmokePresentationHost({
      target,
      waitForPresentationDelay,
      writeStepArtifact,
      close,
    });

    expect(host.target).toBe(target);
    expect(host.waitForPresentationDelay).toBe(waitForPresentationDelay);
    expect(host.writeStepArtifact).toBe(writeStepArtifact);
    expect(host.close).toBe(close);
  });
});
