import { expectJsonObject, hasJsonValue, type JsonObject } from "./model-json-utils";
import { ItemTransform } from "./item-transform";

export enum TransformType {
  NONE = "none",
  THIRD_PERSON_LEFT_HAND = "third_person_left_hand",
  THIRD_PERSON_RIGHT_HAND = "third_person_right_hand",
  FIRST_PERSON_LEFT_HAND = "first_person_left_hand",
  FIRST_PERSON_RIGHT_HAND = "first_person_right_hand",
  HEAD = "head",
  GUI = "gui",
  GROUND = "ground",
  FIXED = "fixed",
}

export class ItemTransforms {
  public static readonly NO_TRANSFORMS = new ItemTransforms();

  public readonly thirdPersonLeftHand: ItemTransform;
  public readonly thirdPersonRightHand: ItemTransform;
  public readonly firstPersonLeftHand: ItemTransform;
  public readonly firstPersonRightHand: ItemTransform;
  public readonly head: ItemTransform;
  public readonly gui: ItemTransform;
  public readonly ground: ItemTransform;
  public readonly fixed: ItemTransform;

  public constructor();
  public constructor(other: ItemTransforms);
  public constructor(
    thirdPersonLeftHand: ItemTransform,
    thirdPersonRightHand: ItemTransform,
    firstPersonLeftHand: ItemTransform,
    firstPersonRightHand: ItemTransform,
    head: ItemTransform,
    gui: ItemTransform,
    ground: ItemTransform,
    fixed: ItemTransform,
  );
  public constructor(
    thirdPersonLeftHand?: ItemTransform | ItemTransforms,
    thirdPersonRightHand: ItemTransform = ItemTransform.NO_TRANSFORM,
    firstPersonLeftHand: ItemTransform = ItemTransform.NO_TRANSFORM,
    firstPersonRightHand: ItemTransform = ItemTransform.NO_TRANSFORM,
    head: ItemTransform = ItemTransform.NO_TRANSFORM,
    gui: ItemTransform = ItemTransform.NO_TRANSFORM,
    ground: ItemTransform = ItemTransform.NO_TRANSFORM,
    fixed: ItemTransform = ItemTransform.NO_TRANSFORM,
  ) {
    if (thirdPersonLeftHand instanceof ItemTransforms) {
      this.thirdPersonLeftHand = thirdPersonLeftHand.thirdPersonLeftHand;
      this.thirdPersonRightHand = thirdPersonLeftHand.thirdPersonRightHand;
      this.firstPersonLeftHand = thirdPersonLeftHand.firstPersonLeftHand;
      this.firstPersonRightHand = thirdPersonLeftHand.firstPersonRightHand;
      this.head = thirdPersonLeftHand.head;
      this.gui = thirdPersonLeftHand.gui;
      this.ground = thirdPersonLeftHand.ground;
      this.fixed = thirdPersonLeftHand.fixed;
      return;
    }

    this.thirdPersonLeftHand = thirdPersonLeftHand ?? ItemTransform.NO_TRANSFORM;
    this.thirdPersonRightHand = thirdPersonRightHand;
    this.firstPersonLeftHand = firstPersonLeftHand;
    this.firstPersonRightHand = firstPersonRightHand;
    this.head = head;
    this.gui = gui;
    this.ground = ground;
    this.fixed = fixed;
  }

  public getTransform(type: TransformType): ItemTransform {
    switch (type) {
      case TransformType.THIRD_PERSON_LEFT_HAND:
        return this.thirdPersonLeftHand;
      case TransformType.THIRD_PERSON_RIGHT_HAND:
        return this.thirdPersonRightHand;
      case TransformType.FIRST_PERSON_LEFT_HAND:
        return this.firstPersonLeftHand;
      case TransformType.FIRST_PERSON_RIGHT_HAND:
        return this.firstPersonRightHand;
      case TransformType.HEAD:
        return this.head;
      case TransformType.GUI:
        return this.gui;
      case TransformType.GROUND:
        return this.ground;
      case TransformType.FIXED:
        return this.fixed;
      case TransformType.NONE:
      default:
        return ItemTransform.NO_TRANSFORM;
    }
  }

  public hasTransform(type: TransformType): boolean {
    return this.getTransform(type) !== ItemTransform.NO_TRANSFORM;
  }

  public static fromJson(value: unknown): ItemTransforms {
    const json = expectJsonObject(value, "item transforms");
    const thirdPersonRightHand = ItemTransforms.getTransform(json, "thirdperson_righthand");
    let thirdPersonLeftHand = ItemTransforms.getTransform(json, "thirdperson_lefthand");
    if (thirdPersonLeftHand === ItemTransform.NO_TRANSFORM) {
      thirdPersonLeftHand = thirdPersonRightHand;
    }

    const firstPersonRightHand = ItemTransforms.getTransform(json, "firstperson_righthand");
    let firstPersonLeftHand = ItemTransforms.getTransform(json, "firstperson_lefthand");
    if (firstPersonLeftHand === ItemTransform.NO_TRANSFORM) {
      firstPersonLeftHand = firstPersonRightHand;
    }

    const head = ItemTransforms.getTransform(json, "head");
    const gui = ItemTransforms.getTransform(json, "gui");
    const ground = ItemTransforms.getTransform(json, "ground");
    const fixed = ItemTransforms.getTransform(json, "fixed");
    return new ItemTransforms(thirdPersonLeftHand, thirdPersonRightHand, firstPersonLeftHand, firstPersonRightHand, head, gui, ground, fixed);
  }

  private static getTransform(json: JsonObject, key: string): ItemTransform {
    return hasJsonValue(json, key) ? ItemTransform.fromJson(json[key]) : ItemTransform.NO_TRANSFORM;
  }
}
