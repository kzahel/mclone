import { describe, expect, test } from "vitest";
import { getTouchMoveButtonRects, hitTouchMoveButton } from "../../src/renderer/debug/touch-control-layout";

describe("touch control layout", () => {
  test("places forward and back buttons on the right side", () => {
    const target = {
      cssLeft: 0,
      cssTop: 0,
      cssWidth: 390,
      cssHeight: 844,
    };
    const [forward, back] = getTouchMoveButtonRects(target);

    expect(forward?.kind).toBe("forward");
    expect(back?.kind).toBe("back");
    expect(forward?.x).toBeGreaterThan(280);
    expect(back?.x).toBe(forward?.x);
    expect(forward?.y).toBeLessThan(back?.y ?? 0);
    expect(hitTouchMoveButton((forward?.x ?? 0) + 10, (forward?.y ?? 0) + 10, target)).toBe("forward");
    expect(hitTouchMoveButton((back?.x ?? 0) + 10, (back?.y ?? 0) + 10, target)).toBe("back");
    expect(hitTouchMoveButton(80, 720, target)).toBeNull();
  });
});
