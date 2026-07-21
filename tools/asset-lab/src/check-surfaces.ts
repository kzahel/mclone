import path from "node:path";
import { discoverCanonicalFigureSources } from "./discover-figures";
import { loadFigureJsonDocument } from "./load";
import {
  evaluateFigureSurfaces,
  formatSurfaceIssue,
} from "./surface-analysis";
import { assetLabRoot } from "./vite-figure-path";

const sourcePaths = await discoverCanonicalFigureSources();
const verbose = process.argv.includes("--verbose");
let baselineCount = 0;
let acknowledgedCount = 0;
let failureCount = 0;
const failures: string[] = [];
for (const sourcePath of sourcePaths) {
  const { asset } = await loadFigureJsonDocument(sourcePath);
  const evaluation = evaluateFigureSurfaces(asset);
  for (const issue of evaluation.baseline) {
    baselineCount += 1;
    if (verbose) {
      console.log(`${relative(sourcePath)}: baseline warning ${formatSurfaceIssue(issue)}`);
    }
  }
  for (const { issue, reason } of evaluation.acknowledged) {
    acknowledgedCount += 1;
    console.log(`${relative(sourcePath)}: acknowledged ${formatSurfaceIssue(issue)} — ${reason}`);
  }
  for (const issue of evaluation.unacknowledged) {
    failureCount += 1;
    failures.push(`${relative(sourcePath)}: ${formatSurfaceIssue(issue)}`);
  }
  for (const exception of evaluation.staleExceptions) {
    failureCount += 1;
    failures.push(`${relative(sourcePath)}: stale source exception ${JSON.stringify(exception.faces)}`);
  }
  for (const key of evaluation.staleBaseline) {
    failureCount += 1;
    failures.push(`${relative(sourcePath)}: stale baseline ${key}`);
  }
}
console.log(
  `Surface analysis found ${failureCount} failing, ${acknowledgedCount} acknowledged, and ${baselineCount} baseline surface face pair${failureCount + acknowledgedCount + baselineCount === 1 ? "" : "s"} across ${sourcePaths.length} canonical figures.`,
);
if (baselineCount > 0 && !verbose) {
  console.log("Run pnpm asset-lab:surface:check -- --verbose to list baseline warnings.");
}
if (failures.length > 0) {
  throw new Error(`Surface analysis failed:\n${failures.map((failure) => `- ${failure}`).join("\n")}`);
}

function relative(sourcePath: string): string {
  return path.relative(assetLabRoot, sourcePath).replaceAll(path.sep, "/");
}
