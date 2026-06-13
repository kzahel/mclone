import { describe, expect, test } from "vitest";
import {
  createGeneratedWorldSmokeSearchParams,
  GENERATED_WORLD_SMOKE_SCENARIO,
  GENERATED_WORLD_TICK_CADENCE_SCENARIO,
  GENERATED_WORLD_TRANSITION_SCENARIO,
  GENERATED_WORLD_VANILLA_LIGHTING_SCENARIO,
  getGeneratedWorldSmokeScenarioById,
  getGeneratedWorldSmokeStepExpectedChunkCenter,
  getGeneratedWorldSmokeStepExpectedLoadedChunkCount,
  type GeneratedWorldSmokeResultLike,
  type GeneratedWorldSmokeScenario,
  type GeneratedWorldSmokeScenarioStep,
  validateGeneratedWorldSmokeResult,
} from "../../src/renderer/generated-world-smoke-scenario";
import {
  createGeneratedWorldRuntimeTopology,
  resolveGeneratedWorldLightingTopology,
  worldTransportToRuntimeWorldHost,
} from "../../src/renderer/generated-world-runtime-topology";

const renderWorldCounters = {
  ingestBatchCount: 1,
  meshBuildRequestCount: 1,
  meshNotReadyResponseCount: 0,
  meshCompletionCount: 1,
  mainThreadGpuUploadCount: 1,
};

const renderQueueStats = {
  renderedChunkCount: 1,
  pendingVisibleChunkCompileCount: 0,
  queuedChunkBuildCount: 0,
  activeChunkBuildCount: 0,
};

describe("generated-world smoke scenarios", () => {
  test("transition scenario declares distinct chunk-interest steps", () => {
    const [initial, shifted] = GENERATED_WORLD_TRANSITION_SCENARIO.steps;
    expect(initial?.name).toBe("initial");
    expect(shifted?.name).toBe("shifted");
    expect(getGeneratedWorldSmokeStepExpectedChunkCenter(initial!)).not.toEqual(
      getGeneratedWorldSmokeStepExpectedChunkCenter(shifted!),
    );
  });

  test("tick cadence scenario declares repeated input frames", () => {
    expect(getGeneratedWorldSmokeScenarioById("tick-cadence")).toBe(GENERATED_WORLD_TICK_CADENCE_SCENARIO);
    expect(GENERATED_WORLD_TICK_CADENCE_SCENARIO.cadence?.intervalMs).toBe(50);
    expect(GENERATED_WORLD_TICK_CADENCE_SCENARIO.steps.map((step) => step.frameIndex)).toEqual([0, 1, 2, 3]);
    expect(GENERATED_WORLD_TICK_CADENCE_SCENARIO.steps.map((step) => step.playerInput?.sequence)).toEqual([1, 2, 3, 4]);
    expect(new Set(GENERATED_WORLD_TICK_CADENCE_SCENARIO.steps.map((step) => {
      const center = getGeneratedWorldSmokeStepExpectedChunkCenter(step);
      return `${center.x.toString()},${center.z.toString()}`;
    }))).toHaveLength(1);
  });

  test("vanilla lighting scenario requires worker-backed lighting topology", () => {
    expect(getGeneratedWorldSmokeScenarioById("vanilla-lighting")).toBe(GENERATED_WORLD_VANILLA_LIGHTING_SCENARIO);
    expect(GENERATED_WORLD_VANILLA_LIGHTING_SCENARIO.preset).toBe("default");
    expect(GENERATED_WORLD_VANILLA_LIGHTING_SCENARIO.engineConfig).toEqual({
      lightingMode: "vanilla17",
      liquidSimulationMode: "none",
    });

    const step = GENERATED_WORLD_VANILLA_LIGHTING_SCENARIO.steps[0]!;
    const result = makeStepResult(GENERATED_WORLD_VANILLA_LIGHTING_SCENARIO, step, 0);
    expect(result.topology?.lighting).toBe("worker");
    expect(validateGeneratedWorldSmokeResult({
      ...result,
      steps: [result],
    }, {
      expectedWorldTransport: "worker",
      requireReadback: true,
      requirePlayerInput: true,
      requireSteps: true,
    }, GENERATED_WORLD_VANILLA_LIGHTING_SCENARIO)).toEqual([]);
  });

  test("search params carry scenario id and first-step camera", () => {
    const params = createGeneratedWorldSmokeSearchParams(
      { worldTransport: "worker" },
      GENERATED_WORLD_TRANSITION_SCENARIO,
    );

    expect(params.get("generatedWorldScenario")).toBe("transition");
    expect(params.get("cameraX")).toBe(GENERATED_WORLD_TRANSITION_SCENARIO.steps[0]!.camera.position.x.toString());
    expect(params.get("worldTransport")).toBe("worker");
  });

  test("validation accepts per-step tick cadence results", () => {
    const stepResults = GENERATED_WORLD_TICK_CADENCE_SCENARIO.steps.map((step, stepIndex) =>
      makeStepResult(GENERATED_WORLD_TICK_CADENCE_SCENARIO, step, stepIndex)
    );
    const result: GeneratedWorldSmokeResultLike = {
      ...stepResults[stepResults.length - 1]!,
      steps: stepResults,
    };

    expect(validateGeneratedWorldSmokeResult(result, {
      expectedWorldTransport: "worker",
      requireReadback: true,
      requirePlayerInput: true,
      requireSteps: true,
    }, GENERATED_WORLD_TICK_CADENCE_SCENARIO)).toEqual([]);
  });

  test("tick cadence validation rejects stalled player progression", () => {
    const stepResults = GENERATED_WORLD_TICK_CADENCE_SCENARIO.steps.map((step, stepIndex) =>
      makeStepResult(GENERATED_WORLD_TICK_CADENCE_SCENARIO, step, stepIndex)
    );
    stepResults[2] = {
      ...stepResults[2]!,
      playerTick: stepResults[1]!.playerTick,
    };
    const result: GeneratedWorldSmokeResultLike = {
      ...stepResults[stepResults.length - 1]!,
      steps: stepResults,
    };

    expect(validateGeneratedWorldSmokeResult(result, {
      expectedWorldTransport: "worker",
      requireReadback: true,
      requirePlayerInput: true,
      requireSteps: true,
    }, GENERATED_WORLD_TICK_CADENCE_SCENARIO)).toContain("steps[2].expected playerTick to advance past 2, got 2");
  });

  test("validation accepts per-step transition results", () => {
    const stepResults = GENERATED_WORLD_TRANSITION_SCENARIO.steps.map((step, stepIndex) =>
      makeStepResult(GENERATED_WORLD_TRANSITION_SCENARIO, step, stepIndex)
    );
    const result: GeneratedWorldSmokeResultLike = {
      ...stepResults[stepResults.length - 1]!,
      steps: stepResults,
    };

    expect(validateGeneratedWorldSmokeResult(result, {
      expectedWorldTransport: "worker",
      requireReadback: true,
      requirePlayerInput: true,
      requireSteps: true,
    }, GENERATED_WORLD_TRANSITION_SCENARIO)).toEqual([]);
  });

  test("default scenario still validates as a one-step smoke", () => {
    const step = GENERATED_WORLD_SMOKE_SCENARIO.steps[0]!;
    const result = makeStepResult(GENERATED_WORLD_SMOKE_SCENARIO, step, 0);

    expect(validateGeneratedWorldSmokeResult({
      ...result,
      steps: [result],
    }, {
      expectedWorldTransport: "worker",
      requireReadback: true,
      requirePlayerInput: true,
      requireSteps: true,
    }, GENERATED_WORLD_SMOKE_SCENARIO)).toEqual([]);
  });

  test("validation rejects results without declared runtime topology", () => {
    const step = GENERATED_WORLD_SMOKE_SCENARIO.steps[0]!;
    const { topology: _topology, ...result } = makeStepResult(GENERATED_WORLD_SMOKE_SCENARIO, step, 0);

    expect(validateGeneratedWorldSmokeResult({
      ...result,
      steps: [result],
    }, {
      expectedWorldTransport: "worker",
      requireReadback: true,
      requirePlayerInput: true,
      requireSteps: true,
    }, GENERATED_WORLD_SMOKE_SCENARIO)).toContain("expected topology");
  });
});

function makeStepResult(
  scenario: GeneratedWorldSmokeScenario,
  step: GeneratedWorldSmokeScenarioStep,
  stepIndex: number,
): GeneratedWorldSmokeResultLike {
  const chunkCenter = getGeneratedWorldSmokeStepExpectedChunkCenter(step);
  const inputSequence = step.playerInput?.sequence ?? scenario.playerInput.sequence;
  const worldHost = worldTransportToRuntimeWorldHost("worker");
  return {
    worldTransport: "worker",
    topology: createGeneratedWorldRuntimeTopology({
      host: "test",
      worldHost,
      lighting: resolveGeneratedWorldLightingTopology(scenario.engineConfig.lightingMode, worldHost),
      liquidSimulation: scenario.engineConfig.liquidSimulationMode,
      storage: "none",
      assetSource: "test",
      renderTarget: "offscreen-texture",
    }),
    stepName: step.name,
    stepIndex,
    frameIndex: step.frameIndex,
    presentationDelayMs: step.presentationDelayMs ?? scenario.cadence?.intervalMs,
    chunkCenter,
    expectedChunkCenter: chunkCenter,
    sessionChunkCenter: chunkCenter,
    sessionChunkRadius: scenario.viewDistance,
    expectedPlayerInputSequence: inputSequence,
    loadedChunkCount: getGeneratedWorldSmokeStepExpectedLoadedChunkCount(scenario, step),
    expectedLoadedChunkCount: getGeneratedWorldSmokeStepExpectedLoadedChunkCount(scenario, step),
    viewDistance: scenario.viewDistance,
    renderDistance: scenario.renderDistance,
    lightingMode: scenario.engineConfig.lightingMode,
    liquidSimulationMode: scenario.engineConfig.liquidSimulationMode,
    solidDrawCount: 1,
    nonClearPixels: scenario.minimumNonClearPixels,
    centerPixel: [1, 2, 3, 255],
    terrainPixel: [4, 5, 6, 255],
    renderWorldCounters,
    renderQueueStats,
    sessionId: "session",
    playerId: "player",
    playerName: "Player",
    sessionRevision: stepIndex + 1,
    playerInputSequence: inputSequence,
    playerStateRevision: stepIndex + 1,
    playerTick: stepIndex + 1,
    playerPosition: [stepIndex, 1, 2],
  };
}
