import fs from "node:fs/promises";
import type { IncomingMessage, ServerResponse } from "node:http";
import path from "node:path";
import { buildTextureLabIndex } from "../core/texture-index";
import {
  applyTextureCuration,
  clearTextureCandidateCuration,
  selectTextureCandidateForCuration,
  TextureCurationError,
} from "../core/curation";
import { createTextureFreezeRequest, listTextureFreezeRequests, TextureFreezeRequestError } from "../core/freeze-request";
import { isHexColor } from "../dsl";
import { loadTexturePack } from "../load";
import type { TextureLabIndex } from "../core/index-model";
import { parseHexColor, tintTexture } from "../image";
import { textureLabOutputRoot } from "../output-root";
import { decodePng, encodePng } from "../png";
import { contentTypeForImage, ImageFileError, resolveAllowedImageFile } from "./image-files";
import {
  promoteTextureLifecycle,
  returnTextureLifecycleToCandidate,
  TextureLifecycleError,
} from "../core/texture-lifecycle";
import type { TextureFrozenMetadata, TextureLifecycleState } from "../dsl";

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
  const outputRoot = path.resolve(options.outputRoot ?? textureLabOutputRoot());
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

      if (request.method === "GET" && url.pathname === "/api/candidates") {
        const textureName = url.searchParams.get("texture");
        const index = await loadIndex();
        sendJson(
          response,
          textureName
            ? index.candidates.filter((candidate) => candidate.textureName === textureName)
            : index.candidates,
        );
        return true;
      }

      if (request.method === "POST" && url.pathname === "/api/reindex") {
        sendJson(response, await loadIndex(true));
        return true;
      }

      if (request.method === "POST" && url.pathname === "/api/lifecycle/promote") {
        const body = await readJsonBody<{
          textureName?: unknown;
          candidateId?: unknown;
          state?: unknown;
        }>(request);
        if (
          typeof body.textureName !== "string" ||
          (body.state !== "provisional" && body.state !== "curated") ||
          (body.candidateId !== undefined && typeof body.candidateId !== "string")
        ) {
          sendJson(response, { error: "Expected textureName, optional candidateId, and provisional or curated state" }, 400);
          return true;
        }
        const index = await loadIndex();
        const texture = index.textures.find((entry) => entry.name === body.textureName);
        if (!texture) {
          sendJson(response, { error: `Texture '${body.textureName}' not found` }, 404);
          return true;
        }
        const candidate =
          typeof body.candidateId === "string"
            ? index.candidates.find(
                (entry) => entry.id === body.candidateId && entry.textureName === body.textureName,
              )
            : null;
        if (body.candidateId && !candidate) {
          sendJson(response, { error: `Candidate '${body.candidateId}' is not linked to '${body.textureName}'` }, 404);
          return true;
        }
        const candidateImage = candidate?.images.projected;
        const imagePath =
          candidateImage?.exists && candidateImage.path
            ? candidateImage.path
            : texture.images.currentExport.exists
              ? texture.images.currentExport.path
              : null;
        if (!imagePath) {
          sendJson(response, { error: `No promotable first-party candidate image exists for '${body.textureName}'` }, 400);
          return true;
        }
        const pack = await loadTexturePack(inputPath);
        const promoteOptions: Parameters<typeof promoteTextureLifecycle>[0] = {
          pack,
          packInputPath: inputPath,
          textureName: body.textureName,
          imagePath,
          state: body.state as TextureLifecycleState,
          runtimeMaterials: texture.lifecycle.runtimeMaterials,
        };
        const metadata = candidateMetadata(candidate ?? null);
        if (metadata) {
          promoteOptions.metadata = metadata;
        }
        const result = await promoteTextureLifecycle(promoteOptions);
        sendJson(response, { result, index: await loadIndex(true) });
        return true;
      }

      if (request.method === "POST" && url.pathname === "/api/lifecycle/candidate") {
        const body = await readJsonBody<{ textureName?: unknown }>(request);
        if (typeof body.textureName !== "string") {
          sendJson(response, { error: "Expected textureName" }, 400);
          return true;
        }
        await returnTextureLifecycleToCandidate(inputPath, body.textureName);
        sendJson(response, await loadIndex(true));
        return true;
      }

      if (request.method === "POST" && url.pathname === "/api/curation/select") {
        const body = await readJsonBody<{ textureName?: unknown; candidateId?: unknown }>(request);
        if (typeof body.textureName !== "string" || typeof body.candidateId !== "string") {
          sendJson(response, { error: "Expected textureName and candidateId" }, 400);
          return true;
        }
        const index = await loadIndex();
        await selectTextureCandidateForCuration(outputRoot, index.candidates, body.textureName, body.candidateId);
        sendJson(response, await loadIndex(true));
        return true;
      }

      if (request.method === "POST" && url.pathname === "/api/curation/clear") {
        const body = await readJsonBody<{ textureName?: unknown }>(request);
        if (typeof body.textureName !== "string") {
          sendJson(response, { error: "Expected textureName" }, 400);
          return true;
        }
        const index = await loadIndex();
        await clearTextureCandidateCuration(outputRoot, index.candidates, body.textureName);
        sendJson(response, await loadIndex(true));
        return true;
      }

      if (request.method === "POST" && url.pathname === "/api/curation/apply") {
        const index = await loadIndex();
        const pack = await loadTexturePack(inputPath);
        const result = await applyTextureCuration({ outputRoot, pack, candidates: index.candidates });
        sendJson(response, { result, index: await loadIndex(true) });
        return true;
      }

      if (request.method === "POST" && url.pathname === "/api/freeze-request") {
        const body = await readJsonBody<{ textureName?: unknown }>(request);
        if (typeof body.textureName !== "string") {
          sendJson(response, { error: "Expected textureName" }, 400);
          return true;
        }
        const index = await loadIndex();
        const pack = await loadTexturePack(inputPath);
        const freezeRequest = await createTextureFreezeRequest({
          outputRoot,
          pack,
          packInputPath: inputPath,
          candidates: index.candidates,
          textureName: body.textureName,
        });
        sendJson(response, { request: freezeRequest });
        return true;
      }

      if (request.method === "GET" && url.pathname === "/api/freeze-requests") {
        const textureName = url.searchParams.get("texture") ?? undefined;
        sendJson(response, { requests: await listTextureFreezeRequests(outputRoot, textureName) });
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
        sendJson(response, {
          texture,
          blocks: index.blocks.filter((block) => blockUsesTexture(block, texture.name)),
          candidates: index.candidates.filter((candidate) => candidate.textureName === texture.name),
        });
        return true;
      }

      if (request.method === "GET" && url.pathname === "/api/image") {
        await sendImage(response, url.searchParams.get("path"), allowedImageRoots);
        return true;
      }

      if (request.method === "GET" && url.pathname === "/api/tinted-image") {
        await sendTintedImage(response, url.searchParams.get("path"), url.searchParams.get("tint"), allowedImageRoots);
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

async function sendTintedImage(
  response: ServerResponse,
  rawPath: string | null,
  rawTint: string | null,
  allowedRoots: string[],
): Promise<void> {
  if (!rawTint || !isHexColor(rawTint)) {
    sendJson(response, { error: "Missing or invalid tint color" }, 400);
    return;
  }
  if (!rawPath) {
    sendJson(response, { error: "Missing image path" }, 400);
    return;
  }
  const imagePath = await resolveAllowedImageFile(rawPath, allowedRoots);
  const image = decodePng(await fs.readFile(imagePath));
  const tinted = encodePng(tintTexture(image, parseHexColor(rawTint)));
  response.statusCode = 200;
  response.setHeader("Content-Type", "image/png");
  response.setHeader("Cache-Control", "no-store");
  response.end(tinted);
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

async function readJsonBody<T>(request: IncomingMessage): Promise<T> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  if (chunks.length === 0) {
    return {} as T;
  }
  return JSON.parse(Buffer.concat(chunks).toString("utf8")) as T;
}

function sendError(response: ServerResponse, error: unknown): void {
  if (error instanceof ImageFileError) {
    sendJson(response, { error: error.message }, error.statusCode);
    return;
  }
  if (error instanceof TextureCurationError) {
    sendJson(response, { error: error.message }, error.statusCode);
    return;
  }
  if (error instanceof TextureFreezeRequestError) {
    sendJson(response, { error: error.message }, error.statusCode);
    return;
  }
  if (error instanceof TextureLifecycleError) {
    sendJson(response, { error: error.message }, error.statusCode);
    return;
  }
  const message = error instanceof Error ? error.message : String(error);
  sendJson(response, { error: message }, 500);
}

function candidateMetadata(
  candidate: TextureLabIndex["candidates"][number] | null,
): TextureFrozenMetadata | undefined {
  if (!candidate) {
    return undefined;
  }
  const metadata: TextureFrozenMetadata = {
    codename: candidate.codename,
    candidateId: candidate.candidateId,
  };
  if (candidate.promptPreset) metadata.promptPreset = candidate.promptPreset;
  if (candidate.prompt) metadata.prompt = candidate.prompt;
  if (candidate.negativePrompt) metadata.negativePrompt = candidate.negativePrompt;
  if (candidate.modelId) metadata.modelId = candidate.modelId;
  if (candidate.scheduler) metadata.scheduler = candidate.scheduler;
  if (candidate.steps !== null) metadata.steps = candidate.steps;
  if (candidate.seed !== null) metadata.seed = candidate.seed;
  if (candidate.strength !== null) metadata.strength = candidate.strength;
  if (candidate.resolution !== null) metadata.resolution = candidate.resolution;
  return metadata;
}
