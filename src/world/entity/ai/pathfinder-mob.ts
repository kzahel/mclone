import type { BlockPos } from "../../../core/block-pos";
import type { MobAttribute } from "../attribute";

export interface MobRandom {
  nextInt(bound: number): number;
  nextFloat(): number;
  nextDouble(): number;
}

export interface MobAiLevel {
  getMinBuildHeight(): number;
  getMaxBuildHeight(): number;
  findStableStandingY(x: number, z: number, nearY: number): number | undefined;
  isStableDestination(pos: BlockPos): boolean;
  isWater(pos: BlockPos): boolean;
  isSolid(pos: BlockPos): boolean;
}

export interface MobNavigation {
  isDone(): boolean;
  isStableDestination(pos: BlockPos): boolean;
  moveTo(x: number, y: number, z: number, speed: number): boolean;
  stop(): void;
  tick(): void;
}

export interface MobMoveControl {
  setWantedPosition(x: number, y: number, z: number, speed: number): void;
}

export interface PathfinderMob {
  readonly position: {
    readonly x: number;
    readonly y: number;
    readonly z: number;
  };

  blockPosition(): BlockPos;
  getX(): number;
  getY(): number;
  getZ(): number;
  getYRot(): number;
  setYRot(yaw: number): void;
  getBbWidth(): number;
  getRandom(): MobRandom;
  getNoActionTime(): number;
  getNavigation(): MobNavigation;
  getMoveControl(): MobMoveControl;
  getAiLevel(): MobAiLevel | undefined;
  getAttributeValue(attribute: MobAttribute): number;
  setSpeed(speed: number): void;
  setZza(forward: number): void;
  setXxa(strafe: number): void;
  isVehicle(): boolean;
  isOnGround(): boolean;
  isInWaterOrBubble(): boolean;
  hasRestriction(): boolean;
  getRestrictCenter(): BlockPos;
  getRestrictRadius(): number;
  isWithinRestriction(pos: BlockPos): boolean;
  getWalkTargetValue(pos: BlockPos): number;
  hasPathfindingMalus(pos: BlockPos): boolean;
}
