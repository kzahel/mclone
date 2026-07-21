import path from "node:path";
import { fileURLToPath } from "node:url";

export const structureLabRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
export const repositoryRoot = path.resolve(structureLabRoot, "../..");

export function repositoryRelative(filePath: string): string {
  const relative = path.relative(repositoryRoot, path.resolve(filePath));
  if (relative === "" || relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`Path '${filePath}' must be a file beneath ${repositoryRoot}`);
  }
  return relative.replaceAll(path.sep, "/");
}
