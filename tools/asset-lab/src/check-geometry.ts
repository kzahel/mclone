import path from "node:path";
import { discoverCanonicalFigureSources, discoverCanonicalPropSources } from "./discover-figures";
import {
  evaluateFigureGeometry,
  formatIssue,
} from "./geometry-analysis";
import { loadFigureJsonDocument } from "./load";
import { assetLabRoot } from "./vite-figure-path";

const sourcePaths = [...await discoverCanonicalFigureSources(), ...await discoverCanonicalPropSources()];
let issueCount = 0;
let acknowledgedCount = 0;
const failures: string[] = [];
for (const sourcePath of sourcePaths) {
  const { asset } = await loadFigureJsonDocument(sourcePath);
  const evaluation = evaluateFigureGeometry(asset);
  for (const { issue, reason } of evaluation.acknowledged) {
    acknowledgedCount += 1;
    console.log(`${relative(sourcePath)}: acknowledged ${formatIssue(issue)} — ${reason}`);
  }
  for (const issue of evaluation.unacknowledged) {
    issueCount += 1;
    failures.push(`${relative(sourcePath)}: ${formatIssue(issue)}`);
  }
  for (const exception of evaluation.staleExceptions) {
    failures.push(
      `${relative(sourcePath)}: stale ${exception.rule} exception for [${[...exception.parts].sort().join(", ")}] — ${exception.reason}`,
    );
  }
}

console.log(
  `Geometry analysis found ${issueCount} unacknowledged and ${acknowledgedCount} acknowledged disconnected component${issueCount + acknowledgedCount === 1 ? "" : "s"} across ${sourcePaths.length} canonical figures.`,
);
if (failures.length > 0) {
  throw new Error(`Geometry analysis failed:\n${failures.map((failure) => `- ${failure}`).join("\n")}`);
}

function relative(sourcePath: string): string {
  return path.relative(assetLabRoot, sourcePath).replaceAll(path.sep, "/");
}
