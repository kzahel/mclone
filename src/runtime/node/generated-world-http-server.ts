import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { randomUUID } from "node:crypto";
import { mkdir, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";
import { createGeneratedWorldHostForRequest } from "../host/generated-world-host-factory";
import type { WorldHost } from "../protocol/world-host";
import {
  deserializeWorldClientMessage,
  serializeWorldHostMessages,
  type OpenWorldSessionRequest,
  type OpenWorldSessionResponse,
  type SessionChunkViewRequest,
  type SessionChunkViewResponse,
} from "../protocol/world-http-protocol";
import type { OpenWorldRequest, SetChunkViewRequest } from "../protocol/world-messages";
import { FileWorldStorage } from "../storage/file-world-storage";

const DEFAULT_HOST = "127.0.0.1";
const DEFAULT_PORT = 4173;
const DEFAULT_SAVE_ROOT = path.resolve(tmpdir(), "mclone-node-worlds");

export interface GeneratedWorldHttpServerOptions {
  readonly host?: string;
  readonly port?: number;
  readonly saveRoot?: string;
}

export interface GeneratedWorldRemoteServiceOptions {
  readonly saveRoot?: string;
}

interface GeneratedWorldHttpServerConfig {
  host: string;
  port: number;
  saveRoot: string;
}

interface ParsedCliOptions {
  configPath?: string;
  host?: string;
  port?: number;
  saveRoot?: string;
}

interface MutableGeneratedWorldHttpServerConfig {
  host?: string;
  port?: number;
  saveRoot?: string;
}

function formatUnknownError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function parsePort(value: string): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > 65_535) {
    throw new Error(`Expected port to be an integer between 0 and 65535, got ${value}`);
  }

  return parsed;
}

async function readRequestBody(request: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) {
    chunks.push(typeof chunk === "string" ? Buffer.from(chunk) : chunk);
  }

  return Buffer.concat(chunks).toString("utf8");
}

async function readJsonBody<T>(request: IncomingMessage): Promise<T> {
  const body = await readRequestBody(request);
  if (body.length === 0) {
    throw new Error("Expected JSON request body");
  }

  return JSON.parse(body) as T;
}

function sendJson(response: ServerResponse, statusCode: number, body: unknown): void {
  response.statusCode = statusCode;
  response.setHeader("access-control-allow-origin", "*");
  response.setHeader("access-control-allow-methods", "GET,POST,OPTIONS");
  response.setHeader("access-control-allow-headers", "content-type");
  response.setHeader("content-type", "application/json");
  response.end(`${JSON.stringify(body)}\n`);
}

function setCorsHeaders(response: ServerResponse): void {
  response.setHeader("access-control-allow-origin", "*");
  response.setHeader("access-control-allow-methods", "GET,POST,OPTIONS");
  response.setHeader("access-control-allow-headers", "content-type");
}

async function readConfigFile(configPath: string): Promise<MutableGeneratedWorldHttpServerConfig> {
  const fileUrl = path.resolve(configPath);
  const value = JSON.parse(await readFile(fileUrl, "utf8")) as unknown;
  if (typeof value !== "object" || value === null) {
    throw new Error(`Generated world HTTP server config ${fileUrl} must contain an object`);
  }

  const record = value as Record<string, unknown>;
  const parsed: MutableGeneratedWorldHttpServerConfig = {};

  if (record.host !== undefined) {
    if (typeof record.host !== "string") {
      throw new Error(`Generated world HTTP server config ${fileUrl} field host must be a string`);
    }

    parsed.host = record.host;
  }

  if (record.port !== undefined) {
    if (typeof record.port !== "number" || !Number.isSafeInteger(record.port)) {
      throw new Error(`Generated world HTTP server config ${fileUrl} field port must be an integer`);
    }

    parsed.port = record.port;
  }

  if (record.saveRoot !== undefined) {
    if (typeof record.saveRoot !== "string") {
      throw new Error(`Generated world HTTP server config ${fileUrl} field saveRoot must be a string`);
    }

    parsed.saveRoot = path.isAbsolute(record.saveRoot)
      ? record.saveRoot
      : path.resolve(path.dirname(fileUrl), record.saveRoot);
  }

  return parsed;
}

function parseCliOptions(argv: readonly string[]): ParsedCliOptions {
  const parsed: ParsedCliOptions = {};

  for (let index = 0; index < argv.length; index++) {
    const option = argv[index];
    if (option === undefined) {
      continue;
    }

    if (option === "--") {
      continue;
    }

    const value = argv[index + 1];
    if (!option.startsWith("--")) {
      throw new Error(`Unexpected argument ${option}`);
    }

    if (value === undefined) {
      throw new Error(`Missing value for ${option}`);
    }

    switch (option) {
      case "--config":
        parsed.configPath = value;
        break;
      case "--host":
        parsed.host = value;
        break;
      case "--port":
        parsed.port = parsePort(value);
        break;
      case "--save-root":
        parsed.saveRoot = path.resolve(value);
        break;
      default:
        throw new Error(`Unknown option ${option}`);
    }

    index++;
  }

  return parsed;
}

async function loadConfig(argv: readonly string[]): Promise<GeneratedWorldHttpServerConfig> {
  const cli = parseCliOptions(argv);
  const fileConfig = cli.configPath === undefined ? {} : await readConfigFile(cli.configPath);
  return {
    host: cli.host ?? fileConfig.host ?? DEFAULT_HOST,
    port: cli.port ?? fileConfig.port ?? DEFAULT_PORT,
    saveRoot: cli.saveRoot ?? fileConfig.saveRoot ?? DEFAULT_SAVE_ROOT,
  };
}

type SessionRecord = {
  readonly host: WorldHost;
};

export class GeneratedWorldRemoteService {
  private readonly worldStorage: FileWorldStorage;
  private readonly sessions = new Map<string, SessionRecord>();

  public constructor(options: GeneratedWorldRemoteServiceOptions = {}) {
    this.worldStorage = new FileWorldStorage(path.resolve(options.saveRoot ?? DEFAULT_SAVE_ROOT));
  }

  public async openWorld(request: OpenWorldRequest): Promise<OpenWorldSessionResponse> {
    const host = this.createSessionHost(request);
    const messages = await host.openWorld(request);
    const sessionId = randomUUID();
    this.sessions.set(sessionId, { host });
    return {
      sessionId,
      messages: serializeWorldHostMessages(messages),
    };
  }

  public async setChunkView(sessionId: string, request: SetChunkViewRequest): Promise<SessionChunkViewResponse> {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      throw new Error(`Unknown world session ${sessionId}`);
    }

    return {
      messages: serializeWorldHostMessages(await session.host.setChunkView(request)),
    };
  }

  public getSessionCount(): number {
    return this.sessions.size;
  }

  public clearSessions(): void {
    this.sessions.clear();
  }

  private createSessionHost(request: OpenWorldRequest): WorldHost {
    return createGeneratedWorldHostForRequest(request, {
      worldStorage: this.worldStorage,
    });
  }
}

export class GeneratedWorldHttpServer {
  private readonly config: GeneratedWorldHttpServerConfig;
  private readonly service: GeneratedWorldRemoteService;
  private readonly server: Server;

  public constructor(options: GeneratedWorldHttpServerOptions = {}) {
    this.config = {
      host: options.host ?? DEFAULT_HOST,
      port: options.port ?? DEFAULT_PORT,
      saveRoot: path.resolve(options.saveRoot ?? DEFAULT_SAVE_ROOT),
    };
    this.service = new GeneratedWorldRemoteService({
      saveRoot: this.config.saveRoot,
    });
    this.server = createServer((request, response) => {
      void this.handleRequest(request, response);
    });
  }

  public async start(): Promise<void> {
    await mkdir(this.config.saveRoot, { recursive: true });
    await new Promise<void>((resolve, reject) => {
      this.server.once("error", reject);
      this.server.listen(this.config.port, this.config.host, () => {
        this.server.off("error", reject);
        resolve();
      });
    });
  }

  public async stop(): Promise<void> {
    this.service.clearSessions();
    if (!this.server.listening) {
      return;
    }

    await new Promise<void>((resolve, reject) => {
      this.server.close((error) => {
        if (error) {
          reject(error);
          return;
        }

        resolve();
      });
    });
  }

  public getBaseUrl(): string {
    const address = this.server.address();
    if (address === null || typeof address === "string") {
      throw new Error("GeneratedWorldHttpServer has no bound TCP address");
    }

    return `http://${address.address}:${address.port.toString()}`;
  }

  public getSessionCount(): number {
    return this.service.getSessionCount();
  }

  private async handleRequest(request: IncomingMessage, response: ServerResponse): Promise<void> {
    try {
      setCorsHeaders(response);
      if (request.method === "OPTIONS") {
        response.statusCode = 204;
        response.end();
        return;
      }

      const url = new URL(request.url ?? "/", "http://mclone.invalid");
      if (request.method === "GET" && url.pathname === "/healthz") {
        sendJson(response, 200, {
          ok: true,
          sessionCount: this.service.getSessionCount(),
        });
        return;
      }

      if (request.method === "POST" && url.pathname === "/api/world/session") {
        const body = await readJsonBody<OpenWorldSessionRequest>(request);
        const openWorldRequest = deserializeWorldClientMessage(body.message);
        if (openWorldRequest.type !== "open_world") {
          throw new Error(`Expected open_world message, got ${openWorldRequest.type}`);
        }

        sendJson(response, 200, await this.service.openWorld(openWorldRequest));
        return;
      }

      const sessionChunkMatch = url.pathname.match(/^\/api\/world\/session\/([^/]+)\/chunk-view$/);
      if (request.method === "POST" && sessionChunkMatch !== null) {
        const sessionId = decodeURIComponent(sessionChunkMatch[1]!);
        const body = await readJsonBody<SessionChunkViewRequest>(request);
        const chunkViewRequest = deserializeWorldClientMessage(body.message);
        if (chunkViewRequest.type !== "set_chunk_view") {
          throw new Error(`Expected set_chunk_view message, got ${chunkViewRequest.type}`);
        }

        sendJson(response, 200, await this.service.setChunkView(sessionId, chunkViewRequest));
        return;
      }

      sendJson(response, 404, { error: `Unhandled route ${request.method ?? "UNKNOWN"} ${url.pathname}` });
    } catch (error) {
      sendJson(response, 500, { error: formatUnknownError(error) });
    }
  }

}

export async function main(argv: readonly string[] = process.argv.slice(2)): Promise<void> {
  const config = await loadConfig(argv);
  const server = new GeneratedWorldHttpServer(config);
  await server.start();
  process.stdout.write(`${JSON.stringify({
    type: "listening",
    url: server.getBaseUrl(),
    saveRoot: path.resolve(config.saveRoot),
  })}\n`);
}

function isDirectExecution(): boolean {
  return process.argv[1] !== undefined && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
}

if (isDirectExecution()) {
  void main().catch((error) => {
    process.stderr.write(`${formatUnknownError(error)}\n`);
    process.exitCode = 1;
  });
}
