export const WORLD_DB_NAME = "mclone-web-worlds";
export const WORLD_DB_VERSION = 2;
export const WORLD_CATALOG_STORE = "worlds";
export const WORLD_CHUNK_STORE = "chunks";
export const WORLD_ENTITY_CHUNK_STORE = "entityChunks";
export const WORLD_ID_INDEX = "worldId";

export const WEB_WORLD_CATALOG_SCHEMA_VERSION = 1;
export const WEB_WORLD_TARGET_MINECRAFT_VERSION = "1.17.1";
export const WEB_WORLD_BACKEND_LABEL = "web-indexeddb";

const LOCAL_WORLD_ID_MAX_LEN = 64;
const LOCAL_WORLD_DISPLAY_NAME_MAX_CHARS = 64;
const DEFAULT_LOCAL_WORLD_ID = "world";

export interface WebLocalWorldSummary {
  id: string;
  displayName: string;
  seed: number;
  createdUnixMillis: number;
  lastPlayedUnixMillis: number | null;
  storageSchemaVersion: number;
  targetMinecraftVersion: string;
  mcloneVersion: string | null;
  backendLabel: string | null;
  locked: boolean;
  compatible: boolean;
}

export interface WebLocalWorldCreateOptions {
  displayName: string;
  seed: number;
  requestedId?: string | null;
}

export function openWorldDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(WORLD_DB_NAME, WORLD_DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      ensureWorldRecordStore(db, request.transaction, WORLD_CHUNK_STORE);
      ensureWorldRecordStore(db, request.transaction, WORLD_ENTITY_CHUNK_STORE);
      ensureWorldCatalogStore(db);
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("failed to open IndexedDB world store"));
    request.onblocked = () => reject(new Error("IndexedDB world store upgrade was blocked"));
  });
}

export async function listIndexedDbCatalogWorlds(
  db: IDBDatabase,
): Promise<WebLocalWorldSummary[]> {
  const transaction = db.transaction(WORLD_CATALOG_STORE, "readonly");
  const records = await idbRequest<unknown[]>(
    transaction.objectStore(WORLD_CATALOG_STORE).getAll(),
  );
  await transactionDone(transaction);
  const worlds = records.map(normalizeCatalogSummary);
  worlds.sort((a, b) => {
    const aLastPlayed = a.lastPlayedUnixMillis ?? a.createdUnixMillis;
    const bLastPlayed = b.lastPlayedUnixMillis ?? b.createdUnixMillis;
    if (aLastPlayed !== bLastPlayed) return bLastPlayed - aLastPlayed;
    const nameOrder = a.displayName.localeCompare(b.displayName);
    if (nameOrder !== 0) return nameOrder;
    return a.id.localeCompare(b.id);
  });
  return worlds;
}

export async function createIndexedDbCatalogWorld(
  db: IDBDatabase,
  options: WebLocalWorldCreateOptions,
): Promise<WebLocalWorldSummary> {
  const displayName = normalizeDisplayName(options.displayName);
  const existing = await listIndexedDbCatalogWorlds(db);
  const existingIds = new Set(existing.map((world) => world.id));
  const id = options.requestedId
    ? validateLocalWorldId(options.requestedId)
    : availableLocalWorldIdFromDisplayName(displayName, existingIds);
  if (existingIds.has(id)) {
    throw new Error(`local world \`${id}\` already exists`);
  }

  const now = Date.now();
  const summary: WebLocalWorldSummary = {
    id,
    displayName,
    seed: normalizeSeed(options.seed),
    createdUnixMillis: now,
    lastPlayedUnixMillis: now,
    storageSchemaVersion: WEB_WORLD_CATALOG_SCHEMA_VERSION,
    targetMinecraftVersion: WEB_WORLD_TARGET_MINECRAFT_VERSION,
    mcloneVersion: null,
    backendLabel: WEB_WORLD_BACKEND_LABEL,
    locked: false,
    compatible: true,
  };

  const transaction = db.transaction(WORLD_CATALOG_STORE, "readwrite");
  transaction.objectStore(WORLD_CATALOG_STORE).add(summary);
  await transactionDone(transaction);
  return summary;
}

export async function openIndexedDbCatalogWorld(
  db: IDBDatabase,
  id: string,
): Promise<WebLocalWorldSummary> {
  const normalizedId = validateLocalWorldId(id);
  const summary = await getIndexedDbCatalogWorld(db, normalizedId);
  if (!summary) {
    throw new Error(`local world \`${normalizedId}\` was not found`);
  }
  const opened = {
    ...summary,
    lastPlayedUnixMillis: Date.now(),
  };
  await putIndexedDbCatalogSummary(db, opened);
  return opened;
}

export async function deleteIndexedDbCatalogWorld(
  db: IDBDatabase,
  id: string,
  activeWorldId: string | null = null,
): Promise<WebLocalWorldSummary> {
  const normalizedId = validateLocalWorldId(id);
  if (activeWorldId !== null && normalizedId === validateLocalWorldId(activeWorldId)) {
    throw new Error(`cannot delete active local world \`${normalizedId}\`; quit to title first`);
  }
  const summary = await getIndexedDbCatalogWorld(db, normalizedId);
  if (!summary) {
    throw new Error(`local world \`${normalizedId}\` was not found`);
  }

  await clearIndexedDbWorldRecords(db, normalizedId);
  const transaction = db.transaction(WORLD_CATALOG_STORE, "readwrite");
  transaction.objectStore(WORLD_CATALOG_STORE).delete(normalizedId);
  await transactionDone(transaction);
  return summary;
}

export async function clearIndexedDbWorldRecords(
  db: IDBDatabase,
  worldId: string,
): Promise<void> {
  await Promise.all([
    clearIndexedDbStoreForWorld(db, WORLD_CHUNK_STORE, worldId),
    clearIndexedDbStoreForWorld(db, WORLD_ENTITY_CHUNK_STORE, worldId),
  ]);
}

function ensureWorldRecordStore(
  db: IDBDatabase,
  transaction: IDBTransaction | null,
  storeName: string,
): void {
  if (db.objectStoreNames.contains(storeName)) {
    const store = transaction?.objectStore(storeName);
    if (store && !store.indexNames.contains(WORLD_ID_INDEX)) {
      store.createIndex(WORLD_ID_INDEX, "worldId", { unique: false });
    }
    return;
  }
  const store = db.createObjectStore(storeName, { keyPath: ["worldId", "x", "z"] });
  store.createIndex(WORLD_ID_INDEX, "worldId", { unique: false });
}

function ensureWorldCatalogStore(db: IDBDatabase): void {
  if (!db.objectStoreNames.contains(WORLD_CATALOG_STORE)) {
    db.createObjectStore(WORLD_CATALOG_STORE, { keyPath: "id" });
  }
}

async function getIndexedDbCatalogWorld(
  db: IDBDatabase,
  id: string,
): Promise<WebLocalWorldSummary | null> {
  const transaction = db.transaction(WORLD_CATALOG_STORE, "readonly");
  const record = await idbRequest<unknown>(
    transaction.objectStore(WORLD_CATALOG_STORE).get(id),
  );
  await transactionDone(transaction);
  return record ? normalizeCatalogSummary(record) : null;
}

async function putIndexedDbCatalogSummary(
  db: IDBDatabase,
  summary: WebLocalWorldSummary,
): Promise<void> {
  const transaction = db.transaction(WORLD_CATALOG_STORE, "readwrite");
  transaction.objectStore(WORLD_CATALOG_STORE).put(summary);
  await transactionDone(transaction);
}

async function clearIndexedDbStoreForWorld(
  db: IDBDatabase,
  storeName: string,
  worldId: string,
): Promise<void> {
  const transaction = db.transaction(storeName, "readwrite");
  const store = transaction.objectStore(storeName);
  const request = store.index(WORLD_ID_INDEX).openKeyCursor(IDBKeyRange.only(worldId));
  request.onsuccess = () => {
    const cursor = request.result;
    if (!cursor) return;
    store.delete(cursor.primaryKey);
    cursor.continue();
  };
  await transactionDone(transaction);
}

function normalizeCatalogSummary(value: unknown): WebLocalWorldSummary {
  const record = (value ?? {}) as Record<string, unknown>;
  const storageSchemaVersion = Math.trunc(Number(record.storageSchemaVersion) || 0);
  const targetMinecraftVersion = String(
    record.targetMinecraftVersion ?? WEB_WORLD_TARGET_MINECRAFT_VERSION,
  );
  return {
    id: validateLocalWorldId(String(record.id ?? "")),
    displayName: normalizeDisplayName(String(record.displayName ?? "")),
    seed: normalizeSeed(record.seed),
    createdUnixMillis: normalizeUnixMillis(record.createdUnixMillis),
    lastPlayedUnixMillis: optionalUnixMillis(record.lastPlayedUnixMillis),
    storageSchemaVersion,
    targetMinecraftVersion,
    mcloneVersion: nullableString(record.mcloneVersion),
    backendLabel: nullableString(record.backendLabel) ?? WEB_WORLD_BACKEND_LABEL,
    locked: Boolean(record.locked),
    compatible: storageSchemaVersion === WEB_WORLD_CATALOG_SCHEMA_VERSION
      && targetMinecraftVersion === WEB_WORLD_TARGET_MINECRAFT_VERSION,
  };
}

function normalizeDisplayName(displayName: string): string {
  const trimmed = displayName.trim();
  if (trimmed.length === 0) {
    throw new Error("world display name cannot be empty");
  }
  if ([...trimmed].length > LOCAL_WORLD_DISPLAY_NAME_MAX_CHARS) {
    throw new Error(
      `world display name is too long; maximum is ${LOCAL_WORLD_DISPLAY_NAME_MAX_CHARS} characters`,
    );
  }
  return trimmed;
}

function validateLocalWorldId(value: string): string {
  const id = value.trim();
  if (id.length === 0) {
    throw new Error("id cannot be empty");
  }
  if (id.length > LOCAL_WORLD_ID_MAX_LEN) {
    throw new Error(`invalid local world id \`${id}\`: id cannot exceed ${LOCAL_WORLD_ID_MAX_LEN} bytes`);
  }
  if (!/^[a-z0-9]/.test(id)) {
    throw new Error(`invalid local world id \`${id}\`: id must start with a lowercase ASCII letter or digit`);
  }
  if (!/[a-z0-9]$/.test(id)) {
    throw new Error(`invalid local world id \`${id}\`: id must end with a lowercase ASCII letter or digit`);
  }
  if (!/^[a-z0-9_-]+$/.test(id)) {
    throw new Error(
      `invalid local world id \`${id}\`: id may only contain lowercase ASCII letters, digits, '-' and '_'`,
    );
  }
  return id;
}

function availableLocalWorldIdFromDisplayName(displayName: string, existingIds: Set<string>): string {
  const base = slugFromDisplayName(displayName);
  if (!existingIds.has(base)) {
    return base;
  }
  for (let suffixIndex = 2; suffixIndex <= Number.MAX_SAFE_INTEGER; suffixIndex += 1) {
    const candidate = suffixedWorldIdCandidate(base, suffixIndex);
    if (!existingIds.has(candidate)) {
      return candidate;
    }
  }
  throw new Error("exhausted local world id suffix space");
}

function slugFromDisplayName(displayName: string): string {
  let slug = "";
  let pendingSeparator = false;

  for (const ch of displayName.trim()) {
    if (/^[a-zA-Z0-9]$/.test(ch)) {
      const needsSeparator = pendingSeparator && slug.length > 0;
      const needed = 1 + (needsSeparator ? 1 : 0);
      if (slug.length + needed > LOCAL_WORLD_ID_MAX_LEN) {
        break;
      }
      if (needsSeparator) {
        slug += "-";
      }
      slug += ch.toLowerCase();
      pendingSeparator = false;
    } else {
      pendingSeparator = slug.length > 0;
    }
  }

  return slug.length === 0 ? DEFAULT_LOCAL_WORLD_ID : slug;
}

function suffixedWorldIdCandidate(base: string, suffixIndex: number): string {
  const suffix = `-${suffixIndex}`;
  const maxPrefixLen = Math.max(1, LOCAL_WORLD_ID_MAX_LEN - suffix.length);
  let prefix = base.slice(0, maxPrefixLen);
  while (prefix.endsWith("-") || prefix.endsWith("_")) {
    prefix = prefix.slice(0, -1);
  }
  if (prefix.length === 0) {
    prefix = DEFAULT_LOCAL_WORLD_ID.slice(0, maxPrefixLen);
  }
  return `${prefix}${suffix}`;
}

function normalizeSeed(value: unknown): number {
  const seed = Number(value);
  if (!Number.isSafeInteger(seed)) {
    throw new Error(`invalid local world seed ${JSON.stringify(value)}`);
  }
  return seed;
}

function normalizeUnixMillis(value: unknown): number {
  const millis = Math.trunc(Number(value));
  if (!Number.isFinite(millis) || millis < 0) {
    throw new Error(`invalid local world timestamp ${JSON.stringify(value)}`);
  }
  return millis;
}

function optionalUnixMillis(value: unknown): number | null {
  if (value === null || typeof value === "undefined") {
    return null;
  }
  return normalizeUnixMillis(value);
}

function nullableString(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function idbRequest<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

function transactionDone(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"));
    transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
  });
}
