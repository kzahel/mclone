import type { WorldHostMessage } from "./world-messages";

export interface DrainedWorldHostMessages {
  readonly messages: readonly WorldHostMessage[];
  readonly remaining: WorldHostMessage[];
}

function isBulkChunkMessage(message: WorldHostMessage): boolean {
  return message.type === "chunk_snapshot"
    || message.type === "chunk_light_delta"
    || message.type === "entity_snapshot"
    || message.type === "world_progress";
}

export function drainWorldHostMessages(
  pendingMessages: readonly WorldHostMessage[],
  maxMessages: number | undefined,
): DrainedWorldHostMessages {
  if (maxMessages === undefined || maxMessages >= pendingMessages.length) {
    return {
      messages: [...pendingMessages],
      remaining: [],
    };
  }

  const limit = Math.max(0, Math.floor(maxMessages));
  if (limit <= 0) {
    return {
      messages: [],
      remaining: [...pendingMessages],
    };
  }

  const selectedIndices = new Set<number>();
  const selectedMessages: WorldHostMessage[] = [];

  const selectWhere = (predicate: (message: WorldHostMessage) => boolean): void => {
    for (let index = 0; index < pendingMessages.length && selectedMessages.length < limit; index++) {
      if (selectedIndices.has(index)) {
        continue;
      }

      const message = pendingMessages[index]!;
      if (!predicate(message)) {
        continue;
      }

      selectedIndices.add(index);
      selectedMessages.push(message);
    }
  };

  selectWhere((message) => !isBulkChunkMessage(message));
  selectWhere(() => true);

  return {
    messages: selectedMessages,
    remaining: pendingMessages.filter((_, index) => !selectedIndices.has(index)),
  };
}
