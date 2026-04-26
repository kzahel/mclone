import { describe, expect, test } from "vitest";
import {
  createGeneratedWorldSmokeSearchParams,
  GENERATED_WORLD_SMOKE_SCENARIO,
  GENERATED_WORLD_TRANSITION_SCENARIO,
  getGeneratedWorldSmokeStepExpectedChunkCenter,
  getGeneratedWorldSmokeStepExpectedLoadedChunkCount,
  type GeneratedWorldSmokeResultLike,
  type GeneratedWorldSmokeScenario,
  type GeneratedWorldSmokeScenarioStep,
  validateGeneratedWorldSmokeResult,
} from "../../src/renderer/generated-world-smoke-scenario";

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

  test("search params carry scenario id and first-step camera", () => {
    const params = createGeneratedWorldSmokeSearchParams(
      { worldTransport: "worker" },
      GENERATED_WORLD_TRANSITION_SCENARIO,
    );

    expect(params.get("generatedWorldScenario")).toBe("transition");
    expect(params.get("cameraX")).toBe(GENERATED_WORLD_TRANSITION_SCENARIO.steps[0]!.camera.position.x.toString());
    expect(params.get("worldTransport")).toBe("worker");
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
});

function makeStepResult(
  scenario: GeneratedWorldSmokeScenario,
  step: GeneratedWorldSmokeScenarioStep,
  stepIndex: number,
): GeneratedWorldSmokeResultLike {
  const chunkCenter = getGeneratedWorldSmokeStepExpectedChunkCenter(step);
  const inputSequence = step.playerInput?.sequence ?? scenario.playerInput.sequence;
  return {
    worldTransport: "worker",
    stepName: step.name,
    stepIndex,
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
    playerPosition: [0, 1, 2],
  };
}
