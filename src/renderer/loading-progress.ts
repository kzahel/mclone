export interface LoadingProgress {
  readonly stage: string;
  readonly detail?: string;
  readonly current?: number;
  readonly total?: number;
  readonly fraction?: number;
}

export type LoadingProgressSink = (progress: LoadingProgress) => void;

export function progressFraction(progress: LoadingProgress): number | undefined {
  if (progress.fraction !== undefined) {
    return Math.max(0, Math.min(1, progress.fraction));
  }

  if (progress.current !== undefined && progress.total !== undefined && progress.total > 0) {
    return Math.max(0, Math.min(1, progress.current / progress.total));
  }

  return undefined;
}

export function scaleProgress(
  sink: LoadingProgressSink | undefined,
  start: number,
  end: number,
  stage: string,
): LoadingProgressSink | undefined {
  if (sink === undefined) {
    return undefined;
  }

  return (progress) => {
    const childFraction = progressFraction(progress);
    sink({
      ...progress,
      stage: progress.stage || stage,
      fraction: childFraction === undefined ? start : start + ((end - start) * childFraction),
    });
  };
}
