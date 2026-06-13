import { Camera } from "./camera";
import { Matrix4f } from "./math/matrix4f";
import { Vector3f } from "./math/vector3f";
import { LevelRenderer, type LevelRenderFrame } from "./level-renderer";
import { LightTexture } from "./light-texture";
import { PoseStack } from "./vertex/pose-stack";
import { Vec3 } from "../world/phys/vec3";
import type { ClientEntityPresentationState } from "../runtime/client/entity-interpolation-service";

export type CameraState = {
  readonly position: Vec3;
  readonly xRot: number;
  readonly yRot: number;
};

export interface RenderLevelOptions {
  readonly waitForChunkTasks?: boolean;
  readonly entityPresentation?: readonly ClientEntityPresentationState[];
}

export class GameRenderer {
  private readonly mainCamera = new Camera();
  private readonly projectionMatrix = new Matrix4f();
  private darkenWorldAmount = 0;
  private darkenWorldAmountO = 0;

  public constructor(
    private width: number,
    private height: number,
    private readonly renderDistance: number,
    private readonly fov = 70.0,
    private fogEnabled = true,
  ) {}

  public resize(width: number, height: number): void {
    this.width = width;
    this.height = height;
  }

  public resetProjectionMatrix(matrix: Matrix4f): void {
    this.projectionMatrix.load(matrix);
  }

  public getProjectionMatrix(fov: number): Matrix4f {
    return Matrix4f.perspective(fov, this.width / this.height, 0.05, this.getDepthFar());
  }

  public getDepthFar(): number {
    return this.renderDistance * 4.0;
  }

  public getDarkenWorldAmount(partialTick: number): number {
    return this.darkenWorldAmountO + ((this.darkenWorldAmount - this.darkenWorldAmountO) * partialTick);
  }

  public getRenderDistance(): number {
    return this.renderDistance;
  }

  public isFogEnabled(): boolean {
    return this.fogEnabled;
  }

  public setFogEnabled(fogEnabled: boolean): void {
    this.fogEnabled = fogEnabled;
  }

  public getMainCamera(): Camera {
    return this.mainCamera;
  }

  // WebGPU: renderLevel returns frame data for bind-group creation instead of mutating GL render state directly.
  public async renderLevel(
    partialTick: number,
    finishTimeNano: number,
    levelRenderer: LevelRenderer,
    lightTexture: LightTexture,
    cameraState: CameraState,
    options: RenderLevelOptions = {},
  ): Promise<LevelRenderFrame> {
    lightTexture.updateLightTexture(partialTick);
    this.mainCamera.setup(cameraState.position, cameraState.xRot, cameraState.yRot);
    const projectionMatrix = this.getProjectionMatrix(this.fov);
    this.resetProjectionMatrix(projectionMatrix);
    const poseStack = new PoseStack();
    poseStack.mulPose(Vector3f.XP.rotationDegrees(this.mainCamera.getXRot()));
    poseStack.mulPose(Vector3f.YP.rotationDegrees(this.mainCamera.getYRot() + 180.0));
    levelRenderer.prepareCullFrustum(poseStack, this.mainCamera.getPosition(), projectionMatrix.copy());
    return levelRenderer.renderLevel(
      poseStack,
      partialTick,
      finishTimeNano,
      false,
      this.mainCamera,
      this,
      lightTexture,
      projectionMatrix,
      options,
    );
  }
}
