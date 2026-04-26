import { describe, expect, it } from "vitest";
import { Button } from "../../../src/client/gui/components/button";
import { Font } from "../../../src/client/gui/font";
import { GuiComponent } from "../../../src/client/gui/gui-component";
import { ScreenManager } from "../../../src/client/gui/screen-manager";
import { ProgressScreen } from "../../../src/client/gui/screens/progress-screen";
import { PauseScreen } from "../../../src/client/gui/screens/pause-screen";
import { OptionsScreen, type GuiOptionsState } from "../../../src/client/gui/screens/options-screen";
import { Screen } from "../../../src/client/gui/screens/screen";
import { TitleScreen } from "../../../src/client/gui/screens/title-screen";
import { GuiDrawList } from "../../../src/renderer/gui/gui-draw-list";

describe("Gui0 model foundation", () => {
  it("measures and emits ASCII bitmap font glyph commands", () => {
    const font = new Font();
    const drawList = new GuiDrawList();

    expect(font.width("Gui0")).toBe(24);
    expect(font.drawShadow(drawList, "A", 4, 5, 0xffffff)).toBe(11);
    expect(drawList.getCommands()).toHaveLength(2);
    expect(drawList.getCommands().every((command) => command.type === "textured_quad")).toBe(true);
  });

  it("routes button clicks through vanilla-style rectangle hit testing", () => {
    const font = new Font();
    let pressCount = 0;
    const button = new Button(10, 20, 100, 20, "Run", font, () => {
      pressCount++;
    });

    expect(button.mouseClicked(9, 20, 0)).toBe(false);
    expect(button.mouseClicked(10, 20, 1)).toBe(false);
    expect(button.mouseClicked(12, 24, 0)).toBe(true);
    expect(pressCount).toBe(1);
  });

  it("owns screen lifecycle, renderables, and focused child input", () => {
    const manager = new ScreenManager(320, 240);
    const screen = new TestScreen();
    manager.setScreen(screen);

    const drawList = new GuiDrawList();
    manager.render(drawList, 14, 24, 0);

    expect(screen.initCount).toBe(1);
    expect(drawList.getCommands().length).toBeGreaterThan(0);
    expect(manager.mouseClicked(14, 24, 0)).toBe(true);
    expect(screen.pressCount).toBe(1);

    manager.resize(400, 300);
    expect(screen.initCount).toBe(2);
  });

  it("lays out the opt-in title screen and routes GPU button actions", () => {
    const manager = new ScreenManager(320, 240);
    const actions: string[] = [];
    manager.setScreen(new TitleScreen({
      onContinue: () => actions.push("continue"),
      onStartWorld: () => actions.push("start_world"),
      onOptions: () => actions.push("options"),
    }));

    const drawList = new GuiDrawList();
    manager.render(drawList, 160, 132, 0);

    expect(drawList.getCommands().length).toBeGreaterThan(0);
    expect(manager.mouseClicked(160, 132, 0)).toBe(true);
    expect(actions).toEqual(["start_world"]);
    expect(manager.keyPressed(256, 0, 0)).toBe(false);
  });

  it("renders vanilla-style progress text and a GPU progress bar", () => {
    const manager = new ScreenManager(320, 240);
    const progressScreen = new ProgressScreen(true);
    manager.setScreen(progressScreen);
    progressScreen.progressStart("Loading world");
    progressScreen.updateProgress({ stage: "Loading terrain chunks", current: 3, total: 10 });

    const drawList = new GuiDrawList();
    manager.render(drawList, 0, 0, 0);

    expect(drawList.getCommands().length).toBeGreaterThan(4);
    expect(drawList.getCommands().some((command) => command.type === "solid_rect")).toBe(true);
    expect(drawList.getCommands().some((command) => command.type === "textured_quad")).toBe(true);
  });

  it("lays out the pause menu and returns to gameplay", () => {
    const manager = new ScreenManager(320, 240);
    let returnedToGame = false;
    manager.setScreen(new PauseScreen(true, {
      onReturnToGame: () => {
        returnedToGame = true;
        manager.setScreen(null);
      },
    }));

    const drawList = new GuiDrawList();
    manager.render(drawList, 160, 78, 0);

    expect(manager.currentScreen?.getTitle()).toBe("menu.game");
    expect(drawList.getCommands().length).toBeGreaterThan(8);
    expect(manager.mouseClicked(160, 78, 0)).toBe(true);
    expect(returnedToGame).toBe(true);
    expect(manager.currentScreen).toBeNull();
  });

  it("edits browser render options with sliders, cycle buttons, and a checkbox", () => {
    const manager = new ScreenManager(320, 240);
    const titleScreen = new TitleScreen({
      onContinue: () => {},
      onStartWorld: () => {},
      onOptions: () => {},
    });
    const options: GuiOptionsState = {
      viewDistance: 6,
      renderDistance: 192,
      lightingMode: "vanilla17",
      liquidSimulationMode: "vanilla17",
    };
    let changedState: GuiOptionsState | undefined;
    let done = false;
    manager.setScreen(new OptionsScreen(titleScreen, options, {
      onChanged: (state) => {
        changedState = { ...state };
      },
      onDone: () => {
        done = true;
      },
    }));

    expect(manager.currentScreen?.getTitle()).toBe("options.title");
    expect(manager.mouseClicked(151, 38, 0)).toBe(true);
    expect(options.viewDistance).toBe(16);
    expect(manager.mouseClicked(80, 62, 0)).toBe(true);
    expect(options.lightingMode).toBe("none");
    expect(manager.mouseClicked(175, 62, 0)).toBe(true);
    expect(options.liquidSimulationMode).toBe("none");
    expect(manager.mouseClicked(160, 146, 0)).toBe(true);
    expect(done).toBe(true);
    expect(changedState).toEqual(options);
    expect(manager.currentScreen).toBe(titleScreen);
  });

  it("opens the GPU options screen from pause when the action exists", () => {
    const manager = new ScreenManager(320, 240);
    const options: GuiOptionsState = {
      viewDistance: 6,
      renderDistance: 192,
      lightingMode: "vanilla17",
      liquidSimulationMode: "vanilla17",
    };
    let pauseScreen!: PauseScreen;
    pauseScreen = new PauseScreen(true, {
      onReturnToGame: () => {},
      onOptions: () => {
        manager.setScreen(new OptionsScreen(pauseScreen, options));
      },
    });
    manager.setScreen(pauseScreen);

    expect(manager.mouseClicked(110, 150, 0)).toBe(true);
    expect(manager.currentScreen?.getTitle()).toBe("options.title");
    expect(manager.mouseClicked(160, 146, 0)).toBe(true);
    expect(manager.currentScreen).toBe(pauseScreen);
  });
});

class TestScreen extends Screen {
  public initCount = 0;
  public pressCount = 0;

  public constructor() {
    super("Test Screen");
  }

  protected override init(): void {
    this.initCount++;
    this.addRenderableWidget(new Button(10, 20, 100, 20, "Click", this.getFont(), () => {
      this.pressCount++;
    }));
  }

  public override render(drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number): void {
    GuiComponent.fill(drawList, 0, 0, this.width, this.height, 0x80000000);
    super.render(drawList, mouseX, mouseY, partialTick);
  }
}
