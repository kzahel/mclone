import fs from "node:fs/promises";
import type { IncomingMessage, ServerResponse } from "node:http";
import path from "node:path";
import { buildTextureLabIndex, DEFAULT_TEXTURE_LAB_OUTPUT_ROOT } from "../core/texture-index";
import type { TextureLabIndex } from "../core/index-model";
import { contentTypeForImage, ImageFileError, resolveAllowedImageFile } from "./image-files";

export interface TextureLabApiOptions {
  textureLabRoot: string;
  inputPath: string;
  outputRoot?: string;
}

export interface TextureLabApiHandler {
  (request: IncomingMessage, response: ServerResponse): Promise<boolean>;
}

export function createTextureLabApi(options: TextureLabApiOptions): TextureLabApiHandler {
  const textureLabRoot = path.resolve(options.textureLabRoot);
  const inputPath = path.resolve(options.inputPath);
  const outputRoot = path.resolve(options.outputRoot ?? DEFAULT_TEXTURE_LAB_OUTPUT_ROOT);
  const allowedImageRoots = [
    textureLabRoot,
    outputRoot,
    path.resolve(textureLabRoot, "..", "..", "reference", "minecraft-1.17.1"),
  ];
  let cachedIndex: TextureLabIndex | null = null;

  async function loadIndex(force = false): Promise<TextureLabIndex> {
    if (!cachedIndex || force) {
      cachedIndex = await buildTextureLabIndex({ inputPath, outputRoot });
    }
    return cachedIndex;
  }

  return async (request, response) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1");
    if (!url.pathname.startsWith("/api/")) {
      return false;
    }

    try {
      if (request.method === "GET" && url.pathname === "/api/index") {
        sendJson(response, await loadIndex());
        return true;
      }

      if (request.method === "POST" && url.pathname === "/api/reindex") {
        sendJson(response, await loadIndex(true));
        return true;
      }

      if (request.method === "GET" && url.pathname.startsWith("/api/textures/")) {
        const textureName = decodeURIComponent(url.pathname.slice("/api/textures/".length));
        const index = await loadIndex();
        const texture = index.textures.find((entry) => entry.name === textureName);
        if (!texture) {
          sendJson(response, { error: `Texture '${textureName}' not found` }, 404);
          return true;
        }
        sendJson(response, { texture, blocks: index.blocks.filter((block) => blockUsesTexture(block, texture.name)) });
        return true;
      }

      if (request.method === "GET" && url.pathname === "/api/image") {
        await sendImage(response, url.searchParams.get("path"), allowedImageRoots);
        return true;
      }

      sendJson(response, { error: `No API route for ${request.method ?? "GET"} ${url.pathname}` }, 404);
      return true;
    } catch (error) {
      sendError(response, error);
      return true;
    }
  };
}

async function sendImage(response: ServerResponse, rawPath: string | null, allowedRoots: string[]): Promise<void> {
  if (!rawPath) {
    sendJson(response, { error: "Missing image path" }, 400);
    return;
  }
  const imagePath = await resolveAllowedImageFile(rawPath, allowedRoots);
  const image = await fs.readFile(imagePath);
  response.statusCode = 200;
  response.setHeader("Content-Type", contentTypeForImage(imagePath));
  response.setHeader("Cache-Control", "no-store");
  response.end(image);
}

function blockUsesTexture(block: { faces: { textureName: string }[] }, textureName: string): boolean {
  return block.faces.some((face) => face.textureName === textureName);
}

function sendJson(response: ServerResponse, payload: unknown, statusCode = 200): void {
  response.statusCode = statusCode;
  response.setHeader("Content-Type", "application/json; charset=utf-8");
  response.setHeader("Cache-Control", "no-store");
  response.end(JSON.stringify(payload, null, 2));
}

function sendError(response: ServerResponse, error: unknown): void {
  if (error instanceof ImageFileError) {
    sendJson(response, { error: error.message }, error.statusCode);
    return;
  }
  const message = error instanceof Error ? error.message : String(error);
  sendJson(response, { error: message }, 500);
}
