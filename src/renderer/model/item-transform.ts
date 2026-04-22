import { Vector3f } from "../math/vector3f";
import { convertToNumber, expectJsonObject, getAsJsonArray, hasJsonValue, type JsonObject } from "./model-json-utils";

export class ItemTransform {
  public static readonly NO_TRANSFORM = new ItemTransform(new Vector3f(), new Vector3f(), new Vector3f(1.0, 1.0, 1.0));

  public readonly rotation: Vector3f;
  public readonly translation: Vector3f;
  public readonly scale: Vector3f;

  public constructor(rotation: Vector3f, translation: Vector3f, scale: Vector3f) {
    this.rotation = rotation.copy();
    this.translation = translation.copy();
    this.scale = scale.copy();
  }

  public static fromJson(value: unknown): ItemTransform {
    const json = expectJsonObject(value, "item transform");
    const rotation = ItemTransform.getVector3f(json, "rotation", new Vector3f(0.0, 0.0, 0.0));
    const translation = ItemTransform.getVector3f(json, "translation", new Vector3f(0.0, 0.0, 0.0));
    translation.mul(0.0625);
    translation.clamp(-5.0, 5.0);
    const scale = ItemTransform.getVector3f(json, "scale", new Vector3f(1.0, 1.0, 1.0));
    scale.clamp(-4.0, 4.0);
    return new ItemTransform(rotation, translation, scale);
  }

  private static getVector3f(json: JsonObject, key: string, defaultValue: Vector3f): Vector3f {
    if (!hasJsonValue(json, key)) {
      return defaultValue.copy();
    }

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
