import path from "node:path";
import { discoverCanonicalFigureSources, discoverCanonicalPropSources } from "./discover-figures";
import {
  evaluateFigureGrounding,
  formatGroundIssue,
} from "./ground-analysis";
import { loadFigureJsonDocument } from "./load";
import { assetLabRoot } from "./vite-figure-path";

const sourcePaths = [...await discoverCanonicalFigureSources(), ...await discoverCanonicalPropSources()];
const verbose = process.argv.includes("--verbose");
let baselineCount = 0;
let acknowledgedCount = 0;
let failureCount = 0;
const failures: string[] = [];
for (const sourcePath of sourcePaths) {
  const { asset } = await loadFigureJsonDocument(sourcePath);
  const evaluation = evaluateFigureGrounding(asset);
  for (const issue of evaluation.baseline) {
    baselineCount += 1;
    if (verbose) {
      console.log(`${relative(sourcePath)}: baseline warning ${formatGroundIssue(issue)}`);
    }
  }
  for (const { issue, reason } of evaluation.acknowledged) {
    acknowledgedCount += 1;
    console.log(`${relative(sourcePath)}: acknowledged ${formatGroundIssue(issue)} — ${reason}`);
  }
  for (const issue of evaluation.unacknowledged) {
    failureCount += 1;
    failures.push(`${relative(sourcePath)}: ${formatGroundIssue(issue)}`);
  }
  for (const exception of evaluation.staleExceptions) {
    failureCount += 1;
    failures.push(`${relative(sourcePath)}: stale source exception ${JSON.stringify(exception.parts)}`);
  }
  for (const key of evaluation.staleBaseline) {
    failureCount += 1;
    failures.push(`${relative(sourcePath)}: stale baseline ${key}`);
  }
}
console.log(
  `Ground analysis found ${failureCount} failing, ${acknowledgedCount} acknowledged, and ${baselineCount} baseline penetrating part${failureCount + acknowledgedCount + baselineCount === 1 ? "" : "s"} across ${sourcePaths.length} canonical figures.`,
);
if (baselineCount > 0 && !verbose) {
  console.log("Run pnpm asset-lab:ground:check -- --verbose to list baseline warnings.");
}
if (failures.length > 0) {
  throw new Error(`Ground analysis failed:\n${failures.map((failure) => `- ${failure}`).join("\n")}`);
}

function relative(sourcePath: string): string {
  return path.relative(assetLabRoot, sourcePath).replaceAll(path.sep, "/");
}
