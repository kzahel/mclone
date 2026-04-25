export interface MovementIntent {
  readonly wishX: number;
  readonly wishZ: number;
  readonly jump: boolean;
  readonly crouch: boolean;
  readonly sprint: boolean;
  readonly yaw: number;
  readonly pitch: number;
}

export const NO_MOVEMENT_INTENT: MovementIntent = {
  wishX: 0.0,
  wishZ: 0.0,
  jump: false,
  crouch: false,
  sprint: false,
  yaw: 0.0,
  pitch: 0.0,
};
