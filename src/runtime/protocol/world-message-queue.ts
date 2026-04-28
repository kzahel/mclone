import type { WorldHostMessage } from "./world-messages";

export interface DrainedWorldHostMessages {
  readonly messages: readonly WorldHostMessage[];
  readonly remaining: WorldHostMessage[];
}

function isCappedBulkMessage(message: WorldHostMessage): boolean {
  return message.type === "chunk_snapshot"
    || message.type === "chunk_light_delta"
    || message.type === "entity_update"
    || message.type === "entity_snapshot";
}

function findLatestWorldProgressIndex(messages: readonly WorldHostMessage[]): number | undefined {
  for (let index = messages.length - 1; index >= 0; index--) {
    if (messages[index]!.type === "world_progress") {
      return index;
    }
  }

  return undefined;
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
  const latestProgressIndex = findLatestWorldProgressIndex(pendingMessages);
  const latestProgress = latestProgressIndex === undefined ? undefined : pendingMessages[latestProgressIndex]!;
  if (limit <= 0) {
    return {
      messages: latestProgress === undefined ? [] : [latestProgress],
      remaining: pendingMessages.filter((message) => message.type !== "world_progress"),
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

  selectWhere((message) => message.type !== "world_progress" && !isCappedBulkMessage(message));
  selectWhere((message) => message.type !== "world_progress");

  return {
    messages: latestProgress === undefined ? selectedMessages : [latestProgress, ...selectedMessages],
    remaining: pendingMessages.filter((message, index) => message.type !== "world_progress" && !selectedIndices.has(index)),
  };
}
