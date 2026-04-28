export type GeneratedChunkTicketSource =
  | "player_view"
  | "generation_dependency"
  | "light"
  | "entity"
  | "forced";

export interface GeneratedChunkTicket {
  readonly source: GeneratedChunkTicketSource;
  readonly centerChunkX: number;
  readonly centerChunkZ: number;
  readonly radius: number;
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

export class GeneratedChunkTicketSet {
  private readonly tickets = new Map<string, GeneratedChunkTicket>();

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

      this.tickets.set(generatedChunkTicketKey(ticket), ticket);
    }
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

  public getSignature(): string {
    return this.getDebugRecords()
      .map((ticket) => [
        ticket.source,
        ticket.id ?? "",
        ticket.centerChunkX.toString(),
        ticket.centerChunkZ.toString(),
        ticket.radius.toString(),
      ].join(":"))
      .join("|");
  }
}

function generatedChunkTicketKey(ticket: GeneratedChunkTicket): string {
  return `${ticket.source}:${ticket.id ?? `${ticket.centerChunkX.toString()},${ticket.centerChunkZ.toString()},${ticket.radius.toString()}`}`;
}
