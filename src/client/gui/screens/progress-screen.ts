import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import { GuiComponent } from "../gui-component";
import { Screen } from "./screen";

export interface ProgressScreenStatus {
  readonly stage: string;
  readonly detail?: string;
  readonly current?: number;
  readonly total?: number;
  readonly fraction?: number;
}

export class ProgressScreen extends Screen {
  private header: string | null = null;
  private stage: string | null = null;
  private progress = 0;
  private stop = false;

  public constructor(private readonly clearScreenAfterStop: boolean) {
    super("narrator.screen.progress");
  }

  public override shouldCloseOnEsc(): boolean {
    return false;
  }

  public progressStartNoAbort(header: string): void {
    this.progressStart(header);
  }

  public progressStart(header: string): void {
    this.header = header;
    this.progressStage("Working");
  }

  public progressStage(stage: string): void {
    this.stage = stage;
    this.progressStagePercentage(0);
  }

  public progressStagePercentage(progress: number): void {
    this.progress = Math.max(0, Math.min(100, Math.round(progress)));
  }

  public updateProgress(progress: ProgressScreenStatus): void {
    if (this.header === null) {
      this.header = "Loading world";
    }

    this.stage = progress.detail === undefined ? progress.stage : `${progress.stage}: ${progress.detail}`;
    const fraction = progressFraction(progress);
    if (fraction !== undefined) {
      this.progressStagePercentage(fraction * 100);
    }
  }

  public stopProgress(): void {
    this.stop = true;
  }

  public override render(drawList: GuiDrawList, mouseX: number, mouseY: number, partialTick: number): void {
    if (this.stop) {
      if (this.clearScreenAfterStop) {
        this.manager?.setScreen(null);
      }
      return;
    }

    this.renderBackground(drawList);
    if (this.header !== null) {
      GuiComponent.drawCenteredString(drawList, this.getFont(), this.header, Math.floor(this.width / 2), 70, 0xffffffff);
    }

    if (this.stage !== null && this.progress !== 0) {
      GuiComponent.drawCenteredString(
        drawList,
        this.getFont(),
        `${this.stage} ${this.progress.toString()}%`,
        Math.floor(this.width / 2),
        90,
        0xffffffff,
      );
    }

    // WebGPU: visible progress bar instead of vanilla's text-only ProgressScreen.
    this.renderProgressBar(drawList);
    super.render(drawList, mouseX, mouseY, partialTick);
  }

  private renderProgressBar(drawList: GuiDrawList): void {
    const progressWidth = Math.min(240, Math.max(120, this.width - 80));
    const progressHeight = 10;
    const x = Math.floor((this.width - progressWidth) / 2);
    const y = 108;
    const filled = Math.floor((progressWidth - 2) * (this.progress / 100));
    GuiComponent.fill(drawList, x - 1, y - 1, x + progressWidth + 1, y + progressHeight + 1, 0xff000000);
    GuiComponent.fill(drawList, x, y, x + progressWidth, y + progressHeight, 0xff3f3f3f);
    GuiComponent.fill(drawList, x + 1, y + 1, x + 1 + filled, y + progressHeight - 1, 0xffffffff);
    GuiComponent.fill(drawList, x, y, x + progressWidth, y + 1, 0xffa0a0a0);
  }
}

function progressFraction(progress: ProgressScreenStatus): number | undefined {
  if (progress.fraction !== undefined) {
    return Math.max(0, Math.min(1, progress.fraction));
  }

  if (progress.current !== undefined && progress.total !== undefined && progress.total > 0) {
    return Math.max(0, Math.min(1, progress.current / progress.total));
  }

  return undefined;
}
