import { Camera } from "./camera";
import { clamp } from "../util/mth";
import { FogType } from "../world/level/material/fog-type";
import { StaticRenderLevel } from "../world/level/static-render-level";

export enum FogMode {
  FOG_SKY = "fog_sky",
  FOG_TERRAIN = "fog_terrain",
}

export class FogRenderer {
  private static fogRed = 0;
  private static fogGreen = 0;
  private static fogBlue = 0;
  private static fogStart = Number.MAX_VALUE;
  private static fogEnd = Number.MAX_VALUE;

  // WebGPU: static render levels provide direct sky/fog inputs instead of sampling full ClientLevel biome and weather state.
  public static setupColor(
    camera: Camera,
    partialTick: number,
    level: StaticRenderLevel,
    _renderDistance: number,
    darkenWorldAmount: number,
  ): void {
    const skyColor = level.getSkyColor(camera.getPosition(), partialTick);
    FogRenderer.fogRed = skyColor.x;
    FogRenderer.fogGreen = skyColor.y;
    FogRenderer.fogBlue = skyColor.z;

    let clearFactor = (camera.getPosition().y - level.getMinBuildHeight()) * level.getClearColorScale();
    if (clearFactor < 1.0 && camera.getFluidInCamera() !== FogType.LAVA) {
      if (clearFactor < 0.0) {
        clearFactor = 0.0;
      }

      clearFactor *= clearFactor;
      FogRenderer.fogRed *= clearFactor;
      FogRenderer.fogGreen *= clearFactor;
      FogRenderer.fogBlue *= clearFactor;
    }

    if (darkenWorldAmount > 0.0) {
      FogRenderer.fogRed = FogRenderer.fogRed * (1.0 - darkenWorldAmount) + (FogRenderer.fogRed * 0.7 * darkenWorldAmount);
      FogRenderer.fogGreen = FogRenderer.fogGreen * (1.0 - darkenWorldAmount) + (FogRenderer.fogGreen * 0.6 * darkenWorldAmount);
      FogRenderer.fogBlue = FogRenderer.fogBlue * (1.0 - darkenWorldAmount) + (FogRenderer.fogBlue * 0.6 * darkenWorldAmount);
    }
  }

  public static setupNoFog(): void {
    FogRenderer.fogStart = Number.MAX_VALUE;
    FogRenderer.fogEnd = Number.MAX_VALUE;
  }

  // WebGPU: player/effect-dependent fog branches stay deferred until entity and status-effect state exists in the browser client.
  public static setupFog(camera: Camera, mode: FogMode, renderDistance: number, thickFog: boolean): void {
    const fogType = camera.getFluidInCamera();
    let fogStart: number;
    let fogEnd: number;
    if (fogType === FogType.WATER) {
      fogStart = -8.0;
      fogEnd = 96.0;
    } else if (fogType === FogType.LAVA) {
      fogStart = 0.25;
      fogEnd = 1.0;
    } else if (fogType === FogType.POWDER_SNOW) {
      fogStart = 0.0;
      fogEnd = 2.0;
    } else if (thickFog) {
      fogStart = renderDistance * 0.05;
      fogEnd = Math.min(renderDistance, 192.0) * 0.5;
    } else if (mode === FogMode.FOG_SKY) {
      fogStart = 0.0;
      fogEnd = renderDistance;
    } else {
      fogStart = renderDistance * 0.75;
      fogEnd = renderDistance;
    }

    FogRenderer.fogStart = fogStart;
    FogRenderer.fogEnd = fogEnd;
  }

  public static levelFogColor(): readonly [number, number, number, number] {
    return [FogRenderer.fogRed, FogRenderer.fogGreen, FogRenderer.fogBlue, 1.0];
  }

  public static getShaderFogStart(): number {
    return FogRenderer.fogStart;
  }

  public static getShaderFogEnd(): number {
    return FogRenderer.fogEnd;
  }

  public static getShaderFogColor(): readonly [number, number, number, number] {
    return [
      clamp(FogRenderer.fogRed, 0.0, 1.0),
      clamp(FogRenderer.fogGreen, 0.0, 1.0),
      clamp(FogRenderer.fogBlue, 0.0, 1.0),
      1.0,
    ];
  }
}
