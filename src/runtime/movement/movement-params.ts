export interface MovementPhysicsParams {
  readonly gravityBlocksPerSecondSq: number;
  readonly jumpSpeedBlocksPerSecond: number;
  readonly maxGroundSpeedBlocksPerSecond: number;
  readonly maxAirSpeedBlocksPerSecond: number;
  readonly sprintSpeedMultiplier: number;
  readonly groundAccelerationBlocksPerSecondSq: number;
  readonly airAccelerationBlocksPerSecondSq: number;
  readonly groundFrictionPerSecond: number;
  readonly airDragPerSecond: number;
  readonly verticalDragPerSecond: number;
  readonly maxStepHeight: number;
}

export const DEFAULT_MOVEMENT_PHYSICS: MovementPhysicsParams = {
  gravityBlocksPerSecondSq: 32.0,
  jumpSpeedBlocksPerSecond: 8.4,
  maxGroundSpeedBlocksPerSecond: 4.317,
  maxAirSpeedBlocksPerSecond: 4.317,
  sprintSpeedMultiplier: 1.3,
  groundAccelerationBlocksPerSecondSq: 70.0,
  airAccelerationBlocksPerSecondSq: 18.0,
  groundFrictionPerSecond: 18.0,
  airDragPerSecond: 0.0,
  verticalDragPerSecond: 0.4,
  maxStepHeight: 0.6,
};
