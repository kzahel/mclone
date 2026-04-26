import { ResourceLocation } from "../../core/resource-location";
import { AnimationFrame } from "./animation-frame";
import { AnimationMetadataSection } from "./animation-metadata-section";
import { NativeImage } from "./native-image";
import { TextureAtlasSpriteInfo } from "./texture-atlas-sprite";

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function looksLikeHtmlDocument(value: string): boolean {
  const trimmed = value.trimStart().toLowerCase();
  return trimmed.startsWith("<!doctype html") || trimmed.startsWith("<html");
}

function getInteger(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? Math.trunc(value) : undefined;
}

function parseAnimationFrame(value: unknown): AnimationFrame | undefined {
  if (typeof value === "number" && Number.isFinite(value)) {
    return new AnimationFrame(Math.trunc(value));
  }

  if (!isObject(value)) {
    return undefined;
  }

  const index = getInteger(value.index);
  if (index === undefined) {
    return undefined;
  }

  const time = getInteger(value.time);
  return time === undefined ? new AnimationFrame(index) : new AnimationFrame(index, time);
}

export function parseAnimationMetadataSection(json: unknown): AnimationMetadataSection {
  if (!isObject(json) || !isObject(json.animation)) {
    return AnimationMetadataSection.EMPTY;
  }

  const animation = json.animation;
  const frames: AnimationFrame[] = [];
  if (Array.isArray(animation.frames)) {
    for (const frame of animation.frames) {
      const parsed = parseAnimationFrame(frame);
      if (parsed !== undefined) {
        frames.push(parsed);
      }
    }
  }

  return new AnimationMetadataSection(
    frames,
    getInteger(animation.width) ?? AnimationMetadataSection.UNKNOWN_SIZE,
    getInteger(animation.height) ?? AnimationMetadataSection.UNKNOWN_SIZE,
    getInteger(animation.frametime) ?? AnimationMetadataSection.DEFAULT_FRAME_TIME,
    animation.interpolate === true,
  );
}

export function parseAnimationMetadataResponseText(
  value: string,
  contentType: string | null,
): AnimationMetadataSection {
  if (looksLikeHtmlDocument(value) || contentType?.toLowerCase().includes("text/html") === true) {
    return AnimationMetadataSection.EMPTY;
  }

  return parseAnimationMetadataSection(JSON.parse(value) as unknown);
}

export function createSpriteInfo(
  location: ResourceLocation,
  image: NativeImage,
  metadata: AnimationMetadataSection,
): TextureAtlasSpriteInfo {
  const [frameWidth, frameHeight] = metadata.getFrameSize(image.getWidth(), image.getHeight());
  return new TextureAtlasSpriteInfo(location, frameWidth, frameHeight, metadata);
}
