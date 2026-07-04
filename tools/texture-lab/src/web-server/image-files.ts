import fs from "node:fs/promises";
import path from "node:path";

const IMAGE_EXTENSIONS = new Set([".png", ".jpg", ".jpeg", ".webp", ".gif"]);

export class ImageFileError extends Error {
  constructor(
    message: string,
    readonly statusCode: number,
  ) {
    super(message);
  }
}

export async function resolveAllowedImageFile(rawPath: string, allowedRoots: string[]): Promise<string> {
  const requestedPath = path.resolve(rawPath);
  if (!IMAGE_EXTENSIONS.has(path.extname(requestedPath).toLowerCase())) {
    throw new ImageFileError("Only image files can be served", 400);
  }

  const realPath = await realpathOrNotFound(requestedPath);
  const realRoots = await existingRealRoots(allowedRoots);
  if (!realRoots.some((root) => isPathInside(root, realPath))) {
    throw new ImageFileError("Image path is outside the texture-lab allowlist", 403);
  }

  return realPath;
}

export function contentTypeForImage(filePath: string): string {
  const extension = path.extname(filePath).toLowerCase();
  if (extension === ".jpg" || extension === ".jpeg") {
    return "image/jpeg";
  }
  if (extension === ".webp") {
    return "image/webp";
  }
  if (extension === ".gif") {
    return "image/gif";
  }
  return "image/png";
}

async function realpathOrNotFound(filePath: string): Promise<string> {
  try {
    const stat = await fs.stat(filePath);
    if (!stat.isFile()) {
      throw new ImageFileError("Image path is not a file", 404);
    }
    return await fs.realpath(filePath);
  } catch (error) {
    if (error instanceof ImageFileError) {
      throw error;
    }
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      throw new ImageFileError("Image file not found", 404);
    }
    throw error;
  }
}

async function existingRealRoots(roots: string[]): Promise<string[]> {
  const realRoots: string[] = [];
  for (const root of roots) {
    try {
      const stat = await fs.stat(path.resolve(root));
      if (stat.isDirectory()) {
        realRoots.push(await fs.realpath(path.resolve(root)));
      }
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
        throw error;
      }
    }
  }
  return realRoots;
}

function isPathInside(root: string, target: string): boolean {
  const relative = path.relative(root, target);
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}
