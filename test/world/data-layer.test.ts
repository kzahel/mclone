import { describe, expect, test } from "vitest";

import { DataLayer } from "../../src/world/level/chunk/data-layer";

describe("DataLayer", () => {
  test("starts empty and reads zero without allocating", () => {
    const layer = new DataLayer();

    expect(layer.isEmpty()).toBe(true);
    expect(layer.get(1, 2, 3)).toBe(0);
    expect(layer.isEmpty()).toBe(true);
  });

  test("stores two 4-bit values per byte in y-major,z-major,x-minor order", () => {
    const layer = new DataLayer();

    layer.set(0, 0, 0, 0xA);
    layer.set(1, 0, 0, 0xB);
    layer.set(2, 0, 0, 0xC);
    layer.set(0, 1, 0, 0xD);
    layer.set(15, 0, 1, 0xE);

    const data = layer.getData();
    expect(data[0]).toBe(0xBA);
    expect(data[1]).toBe(0x0C);
    expect(data[15]).toBe(0xE0);
    expect(data[128]).toBe(0x0D);
    expect(layer.get(0, 0, 0)).toBe(0xA);
    expect(layer.get(1, 0, 0)).toBe(0xB);
    expect(layer.get(2, 0, 0)).toBe(0xC);
    expect(layer.get(0, 1, 0)).toBe(0xD);
    expect(layer.get(15, 0, 1)).toBe(0xE);
  });

  test("masks stored values to one nibble", () => {
    const layer = new DataLayer();

    layer.set(0, 0, 0, 0x21);
    layer.set(1, 0, 0, -1);

    expect(layer.get(0, 0, 0)).toBe(1);
    expect(layer.get(1, 0, 0)).toBe(15);
    expect(layer.getData()[0]).toBe(0xF1);
  });

  test("constructs from exact 2048-byte data and rejects other lengths", () => {
    const data = new Uint8Array(DataLayer.SIZE);
    data[0] = 0x7F;

    const layer = new DataLayer(data);
    expect(layer.get(0, 0, 0)).toBe(15);
    expect(layer.get(1, 0, 0)).toBe(7);
    expect(() => new DataLayer(new Uint8Array(DataLayer.SIZE - 1))).toThrow(/2048 bytes/);
  });

  test("copy clones data and preserves empty layers", () => {
    const empty = new DataLayer();
    expect(empty.copy().isEmpty()).toBe(true);

    const layer = new DataLayer();
    layer.set(0, 0, 0, 3);
    const copy = layer.copy();
    copy.set(0, 0, 0, 9);

    expect(layer.get(0, 0, 0)).toBe(3);
    expect(copy.get(0, 0, 0)).toBe(9);
  });

  test("debug string helpers match vanilla line breaks", () => {
    const layer = new DataLayer();
    layer.set(0, 0, 0, 1);
    layer.set(15, 0, 0, 2);
    layer.set(0, 1, 0, 3);

    const firstLayerRows = layer.layerToString(0).split("\n");
    expect(firstLayerRows[0]).toBe("1000000000000002");
    expect(firstLayerRows).toHaveLength(17);
    expect(layer.layerToString(7)).toBe(layer.layerToString(0));
    expect(layer.toString().startsWith("1000000000000002\n")).toBe(true);
    expect(layer.toString()).toContain("\n\n3");
  });
});
