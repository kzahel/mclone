import { describe, expect, test } from "vitest";
import { GuiDrawList } from "../../src/renderer/gui/gui-draw-list";
import { renderTouchControlsHud, renderTouchJoystickHud } from "../../src/renderer/gui/touch-joystick-hud";

describe("touch joystick HUD", () => {
  test("renders base and thumb circles in GUI coordinates", () => {
    const drawList = new GuiDrawList();
    renderTouchJoystickHud(
      drawList,
      {
        baseX: 100,
        baseY: 200,
        thumbX: 130,
        thumbY: 180,
        maxDistance: 50,
      },
      {
        guiWidth: 200,
        guiHeight: 150,
        cssLeft: 0,
        cssTop: 0,
        cssWidth: 400,
        cssHeight: 300,
      },
    );

    const commands = drawList.getCommands();
    expect(commands.length).toBeGreaterThan(0);
    expect(commands.some((command) => command.type === "solid_rect" && command.color === 0x44000000)).toBe(true);
    expect(commands.some((command) => command.type === "solid_rect" && command.color === 0x88ffffff)).toBe(true);
    expect(commands.some((command) => command.type === "solid_rect" && command.x0 <= 25 && command.x1 >= 75)).toBe(true);
  });

  test("does not draw when no joystick is active", () => {
    const drawList = new GuiDrawList();
    renderTouchJoystickHud(drawList, null, {
      guiWidth: 200,
      guiHeight: 150,
      cssLeft: 0,
      cssTop: 0,
      cssWidth: 400,
      cssHeight: 300,
    });

    expect(drawList.getCommands()).toHaveLength(0);
  });

  test("renders right-side move buttons without an active joystick", () => {
    const drawList = new GuiDrawList();
    renderTouchControlsHud(
      drawList,
      {
        joystick: null,
        moveForward: true,
        moveBack: false,
        visible: true,
      },
      {
        guiWidth: 390,
        guiHeight: 844,
        cssLeft: 0,
        cssTop: 0,
        cssWidth: 390,
        cssHeight: 844,
      },
    );

    const commands = drawList.getCommands();
    expect(commands.length).toBeGreaterThan(0);
    expect(commands.some((command) => command.type === "solid_rect" && command.color === 0x88ffffff)).toBe(true);
    expect(commands.some((command) => command.type === "solid_rect" && command.color === 0x55000000)).toBe(true);
    expect(commands.some((command) => command.type === "solid_rect" && command.x0 >= 290)).toBe(true);
  });

  test("does not render movement buttons when touch controls are hidden", () => {
    const drawList = new GuiDrawList();
    renderTouchControlsHud(
      drawList,
      {
        joystick: null,
        moveForward: false,
        moveBack: false,
        visible: false,
      },
      {
        guiWidth: 390,
        guiHeight: 844,
        cssLeft: 0,
        cssTop: 0,
        cssWidth: 390,
        cssHeight: 844,
      },
    );

    expect(drawList.getCommands()).toHaveLength(0);
  });
});
