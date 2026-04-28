import type { BlockPos } from "../../../core/block-pos";
import type { MobAttribute } from "../attribute";
import type { AABB } from "../../phys/aabb";
import type { Fluid } from "../../level/material/fluid";
import type { BlockPathTypes } from "../../level/pathfinder/block-path-types";
import type { PathNavigationRegion } from "../../level/pathfinder/path-navigation-region";

export interface MobRandom {
  nextInt(bound: number): number;
  nextFloat(): number;
  nextDouble(): number;
}

export interface MobAiLevel extends PathNavigationRegion {
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
  getBoundingBox(): AABB;
  getX(): number;
  getY(): number;
  getZ(): number;
  getBlockY(): number;
  getYRot(): number;
  setYRot(yaw: number): void;
  getBbWidth(): number;
  getBbHeight(): number;
  getMaxUpStep(): number;
  getMaxFallDistance(): number;
  getRandom(): MobRandom;
  getNoActionTime(): number;
  getNavigation(): MobNavigation;
  getMoveControl(): MobMoveControl;
  getAiLevel(): MobAiLevel | undefined;
  getPathfindingMalus(type: BlockPathTypes): number;
  setPathfindingMalus(type: BlockPathTypes, priority: number): void;
  canCutCorner(type: BlockPathTypes): boolean;
  getAttributeValue(attribute: MobAttribute): number;
  setSpeed(speed: number): void;
  setZza(forward: number): void;
  setXxa(strafe: number): void;
  canStandOnFluid(fluid: Fluid): boolean;
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
