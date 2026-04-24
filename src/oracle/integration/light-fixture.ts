export type LightLayerName = "sky" | "block";

export interface LightSectionBytes {
  readonly y: number;
  readonly data: Uint8Array;
}

export interface ChunkLightBytes {
  readonly sky: readonly LightSectionBytes[];
  readonly block: readonly LightSectionBytes[];
}

export interface LightSectionFixture {
  readonly y: number;
  readonly dataBase64: string;
}

export interface ChunkLightFixture {
  readonly sky: readonly LightSectionFixture[];
  readonly block: readonly LightSectionFixture[];
}

export interface LightDiff {
  readonly layer: LightLayerName;
  readonly y: number;
  readonly kind: "missing" | "extra" | "byte_mismatch";
  readonly firstByteOffset?: number;
  readonly expected?: number;
  readonly actual?: number;
}

export const LIGHT_DATALAYER_BYTES = 2048;

const BASE64_CHUNK_SIZE = 0x8000;

export function lightBytesToBase64(data: Uint8Array, path = "light data"): string {
  assertLightDataLength(data, path);
  let binary = "";
  for (let offset = 0; offset < data.length; offset += BASE64_CHUNK_SIZE) {
    const end = Math.min(offset + BASE64_CHUNK_SIZE, data.length);
    for (let index = offset; index < end; index++) {
      binary += String.fromCharCode(data[index]!);
    }
  }
  return btoa(binary);
}

export function base64ToLightBytes(value: string, path = "light data"): Uint8Array {
  const binary = atob(value);
  const out = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index++) {
    out[index] = binary.charCodeAt(index);
  }
  assertLightDataLength(out, path);
  return out;
}

export function encodeChunkLightFixture(light: ChunkLightBytes): ChunkLightFixture {
  return {
    sky: encodeLightSections(light.sky, "sky"),
    block: encodeLightSections(light.block, "block"),
  };
}

export function decodeChunkLightFixture(light: ChunkLightFixture): ChunkLightBytes {
  return {
    sky: decodeLightSections(light.sky, "sky"),
    block: decodeLightSections(light.block, "block"),
  };
}

export function compareChunkLight(expected: ChunkLightBytes, actual: ChunkLightBytes): LightDiff[] {
  return [
    ...compareLayer("sky", expected.sky, actual.sky),
    ...compareLayer("block", expected.block, actual.block),
  ];
}

export function assertLightDataLength(data: Uint8Array, path: string): void {
  if (data.length !== LIGHT_DATALAYER_BYTES) {
    throw new Error(`${path} must contain ${LIGHT_DATALAYER_BYTES} bytes, got ${data.length}`);
  }
}

function encodeLightSections(sections: readonly LightSectionBytes[], layer: LightLayerName): LightSectionFixture[] {
  return sortLightSections(sections).map((section) => ({
    y: section.y,
    dataBase64: lightBytesToBase64(section.data, `${layer} light section Y=${section.y}`),
  }));
}

function decodeLightSections(sections: readonly LightSectionFixture[], layer: LightLayerName): LightSectionBytes[] {
  return sortLightSections(
    sections.map((section) => ({
      y: section.y,
      data: base64ToLightBytes(section.dataBase64, `${layer} light section Y=${section.y}`),
    })),
  );
}

function sortLightSections<T extends { readonly y: number }>(sections: readonly T[]): T[] {
  const sorted = [...sections].sort((a, b) => a.y - b.y);
  for (let index = 1; index < sorted.length; index++) {
    if (sorted[index - 1]!.y === sorted[index]!.y) {
      throw new Error(`duplicate light section Y=${sorted[index]!.y}`);
    }
  }
  return sorted;
}

function compareLayer(
  layer: LightLayerName,
  expectedSections: readonly LightSectionBytes[],
  actualSections: readonly LightSectionBytes[],
): LightDiff[] {
  const expected = sectionMap(expectedSections, `${layer} expected`);
  const actual = sectionMap(actualSections, `${layer} actual`);
  const ys = [...new Set([...expected.keys(), ...actual.keys()])].sort((a, b) => a - b);
  const diffs: LightDiff[] = [];

  for (const y of ys) {
    const expectedData = expected.get(y);
    const actualData = actual.get(y);
    if (expectedData === undefined) {
      diffs.push({ layer, y, kind: "extra" });
      continue;
    }
    if (actualData === undefined) {
      diffs.push({ layer, y, kind: "missing" });
      continue;
    }
    const mismatch = firstByteMismatch(expectedData, actualData);
    if (mismatch !== undefined) {
      diffs.push({ layer, y, kind: "byte_mismatch", ...mismatch });
    }
  }

  return diffs;
}

function sectionMap(sections: readonly LightSectionBytes[], path: string): Map<number, Uint8Array> {
  const out = new Map<number, Uint8Array>();
  for (const section of sections) {
    if (out.has(section.y)) {
      throw new Error(`${path} contains duplicate light section Y=${section.y}`);
    }
    out.set(section.y, section.data);
  }
  return out;
}

function firstByteMismatch(
  expected: Uint8Array,
  actual: Uint8Array,
): Pick<LightDiff, "firstByteOffset" | "expected" | "actual"> | undefined {
  const sharedLength = Math.min(expected.length, actual.length);
  for (let offset = 0; offset < sharedLength; offset++) {
    if (expected[offset] !== actual[offset]) {
      return {
        firstByteOffset: offset,
        expected: expected[offset],
        actual: actual[offset],
      };
    }
  }
  if (expected.length !== actual.length) {
    return {
      firstByteOffset: sharedLength,
      expected: expected[sharedLength],
      actual: actual[sharedLength],
    };
  }
  return undefined;
}
