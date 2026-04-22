import { Direction } from "../../core/direction";
import { Vector3f } from "../math/vector3f";
import { BlockElementFace } from "./block-element-face";
import { BlockElementRotation } from "./block-element-rotation";
import {
  convertToNumber,
  expectJsonObject,
  getAsBoolean,
  getAsJsonArray,
  getAsJsonObject,
  getAsString,
  hasJsonValue,
  type JsonObject,
} from "./model-json-utils";

export class BlockElement {
  public constructor(
    public readonly from: Vector3f,
    public readonly to: Vector3f,
    public readonly faces: ReadonlyMap<Direction, BlockElementFace>,
    public readonly rotation: BlockElementRotation | undefined,
    public readonly shade: boolean,
  ) {
    this.fillUvs();
  }

  private fillUvs(): void {
    for (const [direction, face] of this.faces.entries()) {
      face.uv.setMissingUv(this.uvsByFace(direction));
    }
  }

  private uvsByFace(direction: Direction): number[] {
    switch (direction) {
      case Direction.DOWN:
        return [this.from.x(), 16.0 - this.to.z(), this.to.x(), 16.0 - this.from.z()];
      case Direction.UP:
        return [this.from.x(), this.from.z(), this.to.x(), this.to.z()];
      case Direction.SOUTH:
        return [this.from.x(), 16.0 - this.to.y(), this.to.x(), 16.0 - this.from.y()];
      case Direction.WEST:
        return [this.from.z(), 16.0 - this.to.y(), this.to.z(), 16.0 - this.from.y()];
      case Direction.EAST:
        return [16.0 - this.to.z(), 16.0 - this.to.y(), 16.0 - this.from.z(), 16.0 - this.from.y()];
      case Direction.NORTH:
      default:
        return [16.0 - this.to.x(), 16.0 - this.to.y(), 16.0 - this.from.x(), 16.0 - this.from.y()];
    }
  }

  public static fromJson(value: unknown): BlockElement {
    const json = expectJsonObject(value, "element");
    const from = BlockElement.getFrom(json);
    const to = BlockElement.getTo(json);
    const rotation = BlockElement.getRotation(json);
    const faces = BlockElement.getFaces(json);
    if (hasJsonValue(json, "shade") && typeof json.shade !== "boolean") {
      throw new Error("Expected shade to be a Boolean");
    }

    const shade = getAsBoolean(json, "shade", true);
    return new BlockElement(from, to, faces, rotation, shade);
  }

  private static getRotation(json: JsonObject): BlockElementRotation | undefined {
    if (!hasJsonValue(json, "rotation")) {
      return undefined;
    }

    const rotationJson = getAsJsonObject(json, "rotation");
    const origin = BlockElement.getVector3f(rotationJson, "origin");
    origin.mul(0.0625);
    const axis = BlockElement.getAxis(rotationJson);
    const angle = BlockElement.getAngle(rotationJson);
    const rescale = getAsBoolean(rotationJson, "rescale", false);
    return new BlockElementRotation(origin, axis, angle, rescale);
  }

  private static getAngle(json: JsonObject): number {
    const angle = convertToNumber(json.angle, "angle");
    if (angle !== 0.0 && Math.abs(angle) !== 22.5 && Math.abs(angle) !== 45.0) {
      throw new Error(`Invalid rotation ${angle} found, only -45/-22.5/0/22.5/45 allowed`);
    }

    return angle;
  }

  private static getAxis(json: JsonObject): Direction.Axis {
    const axisName = getAsString(json, "axis");
    const axis = Direction.Axis.byName(axisName.toLowerCase());
    if (axis === undefined) {
      throw new Error(`Invalid rotation axis: ${axisName}`);
    }

    return axis;
  }

  private static getFaces(json: JsonObject): ReadonlyMap<Direction, BlockElementFace> {
    const faces = new Map<Direction, BlockElementFace>();
    const facesJson = getAsJsonObject(json, "faces");
    for (const [directionName, faceJson] of Object.entries(facesJson)) {
      faces.set(BlockElement.getFacing(directionName), BlockElementFace.fromJson(faceJson));
    }

    if (faces.size === 0) {
      throw new Error("Expected between 1 and 6 unique faces, got 0");
    }

    return faces;
  }

  private static getFacing(value: string): Direction {
    const direction = Direction.byName(value);
    if (direction === undefined) {
      throw new Error(`Unknown facing: ${value}`);
    }

    return direction;
  }

  private static getTo(json: JsonObject): Vector3f {
    const vector = BlockElement.getVector3f(json, "to");
    if (
      vector.x() < -16.0 ||
      vector.y() < -16.0 ||
      vector.z() < -16.0 ||
      vector.x() > 32.0 ||
      vector.y() > 32.0 ||
      vector.z() > 32.0
    ) {
      throw new Error(`'to' specifier exceeds the allowed boundaries: ${vector}`);
    }

    return vector;
  }

  private static getFrom(json: JsonObject): Vector3f {
    const vector = BlockElement.getVector3f(json, "from");
    if (
      vector.x() < -16.0 ||
      vector.y() < -16.0 ||
      vector.z() < -16.0 ||
      vector.x() > 32.0 ||
      vector.y() > 32.0 ||
      vector.z() > 32.0
    ) {
      throw new Error(`'from' specifier exceeds the allowed boundaries: ${vector}`);
    }

    return vector;
  }

  private static getVector3f(json: JsonObject, key: string): Vector3f {
    const array = getAsJsonArray(json, key);
    if (array.length !== 3) {
      throw new Error(`Expected 3 ${key} values, found: ${array.length}`);
    }

    return new Vector3f(
      convertToNumber(array[0], `${key}[0]`),
      convertToNumber(array[1], `${key}[1]`),
      convertToNumber(array[2], `${key}[2]`),
    );
  }
}
