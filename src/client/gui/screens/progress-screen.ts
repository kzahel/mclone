import type { GuiDrawList } from "../../../renderer/gui/gui-draw-list";
import type { GeneratedChunkLifecycleSnapshot } from "../../../runtime/protocol/chunk-lifecycle";
import {
  measureChunkLifecycleHudPanel,
  measureChunkLifecycleHudPlaceholderPanel,
  renderChunkLifecycleHud,
  renderChunkLifecycleHudPlaceholder,
} from "../chunk-lifecycle-hud";
import { GuiComponent } from "../gui-component";
import { Screen } from "./screen";

export interface ProgressScreenStatus {
  readonly stage: string;
  readonly detail?: string;
  readonly current?: number;
  readonly total?: number;
  readonly fraction?: number;
}

const CHUNK_LIFECYCLE_PENDING_LINES = [
  "Chunk Lifecycle",
  "waiting for chunk view",
];

export class ProgressScreen extends Screen {
  private header: string | null = null;
  private stage: string | null = null;
  private progress = 0;
  private stop = false;
  private chunkLifecycle: GeneratedChunkLifecycleSnapshot | undefined;
  private chunkLifecycleDiagnosticsVisible = false;

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

  public updateChunkLifecycle(snapshot: GeneratedChunkLifecycleSnapshot | undefined): void {
    this.chunkLifecycle = snapshot;
  }

  public setChunkLifecycleDiagnosticsVisible(visible: boolean): void {
    this.chunkLifecycleDiagnosticsVisible = visible;
    if (!visible) {
      this.chunkLifecycle = undefined;
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
    const lifecyclePanel = this.measureChunkLifecyclePanel();
    const contentWidth = lifecyclePanel !== undefined && lifecyclePanel.x >= 120
      ? lifecyclePanel.x - 12
      : this.width;
    const contentCenterX = Math.floor(contentWidth / 2);
    if (this.header !== null) {
      GuiComponent.drawCenteredString(drawList, this.getFont(), this.header, contentCenterX, 70, 0xffffffff);
    }

    if (this.stage !== null && this.progress !== 0) {
      GuiComponent.drawCenteredString(
        drawList,
        this.getFont(),
        `${this.stage} ${this.progress.toString()}%`,
        contentCenterX,
        90,
        0xffffffff,
      );
    }

    // WebGPU: visible progress bar instead of vanilla's text-only ProgressScreen.
    this.renderProgressBar(drawList, contentWidth);
    this.renderChunkLifecyclePanel(drawList);
    super.render(drawList, mouseX, mouseY, partialTick);
  }

  private measureChunkLifecyclePanel() {
    if (this.chunkLifecycle !== undefined) {
      return measureChunkLifecycleHudPanel(this.getFont(), this.width, this.height, this.chunkLifecycle);
    }

    if (!this.chunkLifecycleDiagnosticsVisible) {
      return undefined;
    }

    return measureChunkLifecycleHudPlaceholderPanel(
      this.getFont(),
      this.width,
      this.height,
      CHUNK_LIFECYCLE_PENDING_LINES,
    );
  }

  private renderChunkLifecyclePanel(drawList: GuiDrawList): void {
    if (this.chunkLifecycle !== undefined) {
      renderChunkLifecycleHud(drawList, this.getFont(), this.width, this.height, this.chunkLifecycle);
      return;
    }

    if (!this.chunkLifecycleDiagnosticsVisible) {
      return;
    }

    renderChunkLifecycleHudPlaceholder(
      drawList,
      this.getFont(),
      this.width,
      this.height,
      CHUNK_LIFECYCLE_PENDING_LINES,
    );
  }

  private renderProgressBar(drawList: GuiDrawList, contentWidth: number): void {
    const progressWidth = Math.min(240, Math.max(120, contentWidth - 80));
    const progressHeight = 10;
    const x = Math.floor((contentWidth - progressWidth) / 2);
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
