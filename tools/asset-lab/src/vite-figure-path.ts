import path from "node:path";
import { fileURLToPath } from "node:url";

export const assetLabRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
export const repositoryRoot = path.resolve(assetLabRoot, "../..");

export function toViteFigurePath(input: string): string {
  const absolute = path.resolve(input);
  const repositoryRelative = path.relative(repositoryRoot, absolute);
  if (repositoryRelative.startsWith("..") || path.isAbsolute(repositoryRelative)) {
    throw new Error(`Figure '${input}' must live under ${repositoryRoot}`);
  }

  const assetLabRelative = path.relative(assetLabRoot, absolute);
  if (!assetLabRelative.startsWith("..") && !path.isAbsolute(assetLabRelative)) {
    return `/${assetLabRelative.replaceAll(path.sep, "/")}`;
  }

  return `/@fs/${absolute.replaceAll(path.sep, "/")}`;
}
