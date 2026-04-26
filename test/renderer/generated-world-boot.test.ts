import { describe, expect, test } from "vitest";
import {
  GENERATED_WORLD_SMOKE_SCENARIO,
  GENERATED_WORLD_TRANSITION_SCENARIO,
} from "../../src/renderer/generated-world-smoke-scenario";
import {
  resolveGeneratedWorldBootCamera,
} from "../../src/renderer/generated-world-boot";
import {
  resolveGeneratedWorldBrowserCamera,
} from "../../src/renderer/generated-world-browser-boot";

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
});
