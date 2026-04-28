import { ChunkPos } from "../../core/chunk-pos";
import { FullChunkStatus } from "./entity/full-chunk-status";
import { DynamicGraphMinFixedPoint } from "./lighting/dynamic-graph-min-fixed-point";

export type GeneratedChunkTicketSource =
  | "player_view"
  | "generation_dependency"
  | "light"
  | "entity"
  | "forced";

export const GENERATED_CHUNK_ENTITY_TICKING_LEVEL = 31;
export const GENERATED_CHUNK_TICKING_LEVEL = 32;
export const GENERATED_CHUNK_BORDER_LEVEL = 33;
export const GENERATED_CHUNK_INACCESSIBLE_LEVEL = 34;
export const GENERATED_CHUNK_FORCED_LEVEL = GENERATED_CHUNK_ENTITY_TICKING_LEVEL;
export const GENERATED_CHUNK_LIGHT_LEVEL = GENERATED_CHUNK_BORDER_LEVEL;

export interface GeneratedChunkTicket {
  readonly source: GeneratedChunkTicketSource;
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly radius: number;
  readonly sourceRadius?: number;
  readonly level?: number;
  readonly id?: string;
}

export interface GeneratedChunkTicketBounds {
  readonly minChunkX: number;
  readonly maxChunkX: number;
  readonly minChunkZ: number;
  readonly maxChunkZ: number;
}

export interface GeneratedChunkTicketDebugRecord extends GeneratedChunkTicket {
  readonly chunkCount: number;
}

export interface GeneratedChunkTicketLevelDebugRecord {
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly level: number;
  readonly status: FullChunkStatus;
}

export class GeneratedChunkTicketSet {
  private readonly tickets = new Map<string, GeneratedChunkTicket>();
  private levelTracker = GeneratedChunkTicketLevelTracker.fromTickets([]);

  public replaceSource(source: GeneratedChunkTicketSource, tickets: readonly GeneratedChunkTicket[]): void {
    for (const [key, ticket] of this.tickets) {
      if (ticket.source === source) {
        this.tickets.delete(key);
      }
    }

    for (const ticket of tickets) {
      if (ticket.source !== source) {
        throw new Error(`Generated chunk ticket source ${ticket.source} did not match replacement source ${source}`);
      }
      if (ticket.radius < 0) {
        throw new Error(`Generated chunk ticket radius must be non-negative, got ${ticket.radius.toString()}`);
      }
      if (!Number.isInteger(ticket.radius)) {
        throw new Error(`Generated chunk ticket radius must be an integer, got ${ticket.radius.toString()}`);
      }
      if (ticket.sourceRadius !== undefined) {
        if (!Number.isInteger(ticket.sourceRadius)) {
          throw new Error(`Generated chunk ticket source radius must be an integer, got ${ticket.sourceRadius.toString()}`);
        }
        if (ticket.sourceRadius < 0) {
          throw new Error(`Generated chunk ticket source radius must be non-negative, got ${ticket.sourceRadius.toString()}`);
        }
        if (ticket.sourceRadius > ticket.radius) {
          throw new Error(
            `Generated chunk ticket source radius ${ticket.sourceRadius.toString()} exceeded ticket radius ${ticket.radius.toString()}`,
          );
        }
      }
      if (ticket.level !== undefined && !Number.isInteger(ticket.level)) {
        throw new Error(`Generated chunk ticket level must be an integer, got ${ticket.level.toString()}`);
      }

      this.tickets.set(generatedChunkTicketKey(ticket), ticket);
    }

    this.rebuildLevelTracker();
  }

  public clearSource(source: GeneratedChunkTicketSource): void {
    this.replaceSource(source, []);
  }

  public contains(chunkX: number, chunkZ: number): boolean {
    for (const ticket of this.tickets.values()) {
      if (
        Math.abs(chunkX - ticket.centerChunkX) <= ticket.radius
        && Math.abs(chunkZ - ticket.centerChunkZ) <= ticket.radius
      ) {
        return true;
      }
    }

    return false;
  }

  public getLevel(chunkX: number, chunkZ: number): number {
    return this.levelTracker.getTicketLevel(chunkX, chunkZ);
  }

  public getFullStatus(chunkX: number, chunkZ: number): FullChunkStatus {
    return fullStatusFromGeneratedTicketLevel(this.getLevel(chunkX, chunkZ));
  }

  public containsForSource(source: GeneratedChunkTicketSource, chunkX: number, chunkZ: number): boolean {
    for (const ticket of this.tickets.values()) {
      if (
        ticket.source === source
        && Math.abs(chunkX - ticket.centerChunkX) <= ticket.radius
        && Math.abs(chunkZ - ticket.centerChunkZ) <= ticket.radius
      ) {
        return true;
      }
    }

    return false;
  }

  public forEachLeveledChunk(callback: (chunkX: number, chunkZ: number) => void): void {
    this.levelTracker.forEachAccessibleChunk(callback);
  }

  public getBounds(): GeneratedChunkTicketBounds | undefined {
    let bounds: GeneratedChunkTicketBounds | undefined;
    for (const ticket of this.tickets.values()) {
      const ticketBounds: GeneratedChunkTicketBounds = {
        minChunkX: ticket.centerChunkX - ticket.radius,
        maxChunkX: ticket.centerChunkX + ticket.radius,
        minChunkZ: ticket.centerChunkZ - ticket.radius,
        maxChunkZ: ticket.centerChunkZ + ticket.radius,
      };
      bounds = bounds === undefined
        ? ticketBounds
        : {
          minChunkX: Math.min(bounds.minChunkX, ticketBounds.minChunkX),
          maxChunkX: Math.max(bounds.maxChunkX, ticketBounds.maxChunkX),
          minChunkZ: Math.min(bounds.minChunkZ, ticketBounds.minChunkZ),
          maxChunkZ: Math.max(bounds.maxChunkZ, ticketBounds.maxChunkZ),
        };
    }

    return bounds;
  }

  public getCoveredChunkCount(): number {
    const bounds = this.getBounds();
    if (bounds === undefined) {
      return 0;
    }

    let count = 0;
    for (let chunkZ = bounds.minChunkZ; chunkZ <= bounds.maxChunkZ; chunkZ++) {
      for (let chunkX = bounds.minChunkX; chunkX <= bounds.maxChunkX; chunkX++) {
        if (this.contains(chunkX, chunkZ)) {
          count++;
        }
      }
    }

    return count;
  }

  public getDebugRecords(): readonly GeneratedChunkTicketDebugRecord[] {
    return [...this.tickets.values()]
      .map((ticket) => ({
        ...ticket,
        chunkCount: (ticket.radius * 2 + 1) ** 2,
      }))
      .sort((left, right) =>
        left.source.localeCompare(right.source)
        || (left.id ?? "").localeCompare(right.id ?? "")
        || left.centerChunkZ - right.centerChunkZ
        || left.centerChunkX - right.centerChunkX
        || left.radius - right.radius
      );
  }

  public getDebugLevelRecords(): readonly GeneratedChunkTicketLevelDebugRecord[] {
    return this.levelTracker.getDebugRecords();
  }

  public getSignature(): string {
    return this.getDebugRecords()
      .map((ticket) => [
        ticket.source,
        ticket.id ?? "",
        ticket.centerChunkX.toString(),
        ticket.centerChunkZ.toString(),
        ticket.radius.toString(),
        ticket.sourceRadius?.toString() ?? "",
        ticket.level?.toString() ?? "",
      ].join(":"))
      .join("|");
  }

  private rebuildLevelTracker(): void {
    this.levelTracker = GeneratedChunkTicketLevelTracker.fromTickets(this.tickets.values());
  }
}

export function fullStatusFromGeneratedTicketLevel(level: number): FullChunkStatus {
  if (level <= GENERATED_CHUNK_ENTITY_TICKING_LEVEL) {
    return FullChunkStatus.ENTITY_TICKING;
  }
  if (level === GENERATED_CHUNK_TICKING_LEVEL) {
    return FullChunkStatus.TICKING;
  }
  if (level === GENERATED_CHUNK_BORDER_LEVEL) {
    return FullChunkStatus.BORDER;
  }
  return FullChunkStatus.INACCESSIBLE;
}

function generatedChunkTicketKey(ticket: GeneratedChunkTicket): string {
  return `${ticket.source}:${ticket.id ?? `${ticket.centerChunkX.toString()},${ticket.centerChunkZ.toString()},${ticket.radius.toString()},${ticket.sourceRadius?.toString() ?? ""},${ticket.level?.toString() ?? ""}`}`;
}

const GENERATED_CHUNK_TICKET_LEVEL_COUNT = GENERATED_CHUNK_INACCESSIBLE_LEVEL + 1;
const GENERATED_CHUNK_SOURCE_POS = 9223372036854775807n;

class GeneratedChunkTicketLevelTracker extends DynamicGraphMinFixedPoint {
  private readonly levels = new Map<bigint, number>();
  private readonly sourceLevels = new Map<bigint, number>();

  private constructor() {
    super(GENERATED_CHUNK_TICKET_LEVEL_COUNT);
  }

  public static fromTickets(tickets: Iterable<GeneratedChunkTicket>): GeneratedChunkTicketLevelTracker {
    const tracker = new GeneratedChunkTicketLevelTracker();
    for (const ticket of tickets) {
      if (ticket.level === undefined) {
        continue;
      }

      const sourceRadius = ticket.sourceRadius ?? 0;
      for (let chunkZ = ticket.centerChunkZ - sourceRadius; chunkZ <= ticket.centerChunkZ + sourceRadius; chunkZ++) {
        for (let chunkX = ticket.centerChunkX - sourceRadius; chunkX <= ticket.centerChunkX + sourceRadius; chunkX++) {
          tracker.setSourceLevel(ChunkPos.asLong(chunkX, chunkZ), ticket.level);
        }
      }
    }

    for (const [pos, level] of tracker.sourceLevels) {
      tracker.update(pos, level, true);
    }
    tracker.runAllUpdates();
    return tracker;
  }

  public getTicketLevel(chunkX: number, chunkZ: number): number {
    return this.getLevel(ChunkPos.asLong(chunkX, chunkZ));
  }

  public forEachAccessibleChunk(callback: (chunkX: number, chunkZ: number) => void): void {
    for (const pos of this.levels.keys()) {
      callback(ChunkPos.getX(pos), ChunkPos.getZ(pos));
    }
  }

  public getDebugRecords(): readonly GeneratedChunkTicketLevelDebugRecord[] {
    return [...this.levels]
      .map(([pos, level]) => ({
        chunkX: ChunkPos.getX(pos),
        chunkZ: ChunkPos.getZ(pos),
        level,
        status: fullStatusFromGeneratedTicketLevel(level),
      }))
      .sort((left, right) => left.chunkZ - right.chunkZ || left.chunkX - right.chunkX);
  }

  private setSourceLevel(pos: bigint, level: number): void {
    const previous = this.sourceLevels.get(pos);
    if (previous === undefined || level < previous) {
      this.sourceLevels.set(pos, level);
    }
  }

  private update(pos: bigint, level: number, decrease: boolean): void {
    this.checkEdge(GENERATED_CHUNK_SOURCE_POS, pos, level, decrease);
  }

  private runAllUpdates(): void {
    this.runUpdatesForGraph(Number.MAX_SAFE_INTEGER);
  }

  protected override isSource(pos: bigint): boolean {
    return pos === GENERATED_CHUNK_SOURCE_POS;
  }

  protected override checkNeighborsAfterUpdate(pos: bigint, level: number, decrease: boolean): void {
    const chunkX = ChunkPos.getX(pos);
    const chunkZ = ChunkPos.getZ(pos);
    for (let dz = -1; dz <= 1; dz++) {
      for (let dx = -1; dx <= 1; dx++) {
        if (dx === 0 && dz === 0) {
          continue;
        }

        this.checkNeighbor(pos, ChunkPos.asLong(chunkX + dx, chunkZ + dz), level, decrease);
      }
    }
  }

  protected override getComputedLevel(pos: bigint, excludedSource: bigint, candidateLevel: number): number {
    let level = candidateLevel;
    const chunkX = ChunkPos.getX(pos);
    const chunkZ = ChunkPos.getZ(pos);
    for (let dz = -1; dz <= 1; dz++) {
      for (let dx = -1; dx <= 1; dx++) {
        let neighbor = ChunkPos.asLong(chunkX + dx, chunkZ + dz);
        if (neighbor === pos) {
          neighbor = GENERATED_CHUNK_SOURCE_POS;
        }

        if (neighbor === excludedSource) {
          continue;
        }

        const computedLevel = this.computeLevelFromNeighbor(neighbor, pos, this.getLevel(neighbor));
        if (level > computedLevel) {
          level = computedLevel;
        }

        if (level === 0) {
          return level;
        }
      }
    }

    return level;
  }

  protected override getLevel(pos: bigint): number {
    return this.levels.get(pos) ?? GENERATED_CHUNK_INACCESSIBLE_LEVEL;
  }

  protected override setLevel(pos: bigint, level: number): void {
    if (level >= GENERATED_CHUNK_INACCESSIBLE_LEVEL) {
      this.levels.delete(pos);
      return;
    }

    this.levels.set(pos, level);
  }

  protected override computeLevelFromNeighbor(source: bigint, target: bigint, sourceLevel: number): number {
    return source === GENERATED_CHUNK_SOURCE_POS ? this.getLevelFromSource(target) : sourceLevel + 1;
  }

  private getLevelFromSource(pos: bigint): number {
    return this.sourceLevels.get(pos) ?? GENERATED_CHUNK_INACCESSIBLE_LEVEL;
  }
}
