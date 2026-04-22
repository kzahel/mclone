import { Transformation } from "./transformation";

export interface ModelState {
  getRotation(): Transformation;

  isUvLocked(): boolean;
}

export class IdentityModelState implements ModelState {
  public getRotation(): Transformation {
    return Transformation.identity();
  }

  public isUvLocked(): boolean {
    return false;
  }
}

export const IDENTITY_MODEL_STATE = new IdentityModelState();
