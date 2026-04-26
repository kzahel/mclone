import type { HeadlessRendererHost } from "./renderer-host";
import {
  createGeneratedWorldSmokePresentationHost,
  type GeneratedWorldSmokePresentationHost,
  type GeneratedWorldSmokeStepRun,
} from "./generated-world-smoke-runner";
import { renderFrameToHeadlessTarget } from "./static-frame-harness";

export interface GeneratedWorldSmokePngArtifact {
  readonly outputPath: string;
  readonly width: number;
  readonly height: number;
  readonly pixels: Uint8Array;
  readonly stepRun: GeneratedWorldSmokeStepRun;
}

export interface GeneratedWorldHeadlessPresentationHostOptions {
  readonly rendererHost: HeadlessRendererHost;
  readonly width: number;
  readonly height: number;
  readonly format: GPUTextureFormat;
  readonly writePngArtifact?: (artifact: GeneratedWorldSmokePngArtifact) => Promise<void>;
}

export function createGeneratedWorldHeadlessPresentationHost(
  options: GeneratedWorldHeadlessPresentationHostOptions,
): GeneratedWorldSmokePresentationHost {
  return createGeneratedWorldSmokePresentationHost({
    target: {
      width: options.width,
      height: options.height,
      format: options.format,
      renderFrame: ({ scene, frame }) => renderFrameToHeadlessTarget({
        rendererHost: options.rendererHost,
        scene,
        frame,
        width: options.width,
        height: options.height,
        format: options.format,
      }),
    },
    writeStepArtifact: options.writePngArtifact === undefined
      ? undefined
      : async ({ stepRun, outputPath }) => {
        if (stepRun.readback.pixels === undefined) {
          throw new Error(`Generated-world step ${stepRun.result.stepName} did not produce readback pixels`);
        }
        await options.writePngArtifact!({
          outputPath,
          width: stepRun.readback.width,
          height: stepRun.readback.height,
          pixels: stepRun.readback.pixels,
          stepRun,
        });
      },
  });
}
