export type JsonObject = Record<string, unknown>;

function hasOwn(value: JsonObject, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

export function expectJsonObject(value: unknown, context: string): JsonObject {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`Expected ${context} to be a JSON object`);
  }

  return value as JsonObject;
}

export function getAsJsonObject(value: JsonObject, key: string): JsonObject {
  if (!hasOwn(value, key)) {
    throw new Error(`Missing ${key}, expected to find an object`);
  }

  return expectJsonObject(value[key], key);
}

export function getAsJsonArray(value: JsonObject, key: string): readonly unknown[] {
  if (!hasOwn(value, key)) {
    throw new Error(`Missing ${key}, expected to find an array`);
  }

  const json = value[key];
  if (!Array.isArray(json)) {
    throw new Error(`Expected ${key} to be an array`);
  }

  return json;
}

export function getAsString(value: JsonObject, key: string, defaultValue?: string): string {
  if (!hasOwn(value, key)) {
    if (defaultValue !== undefined) {
      return defaultValue;
    }

    throw new Error(`Missing ${key}, expected to find a string`);
  }

  const json = value[key];
  if (typeof json !== "string") {
    throw new Error(`Expected ${key} to be a string`);
  }

  return json;
}

export function getAsBoolean(value: JsonObject, key: string, defaultValue?: boolean): boolean {
  if (!hasOwn(value, key)) {
    if (defaultValue !== undefined) {
      return defaultValue;
    }

    throw new Error(`Missing ${key}, expected to find a boolean`);
  }

  const json = value[key];
  if (typeof json !== "boolean") {
    throw new Error(`Expected ${key} to be a boolean`);
  }

  return json;
}

export function getAsNumber(value: JsonObject, key: string, defaultValue?: number): number {
  if (!hasOwn(value, key)) {
    if (defaultValue !== undefined) {
      return defaultValue;
    }

    throw new Error(`Missing ${key}, expected to find a number`);
  }

  return convertToNumber(value[key], key);
}

export function convertToNumber(value: unknown, context: string): number {
  if (typeof value !== "number") {
    throw new Error(`Expected ${context} to be a number`);
  }

  return value;
}

export function hasJsonValue(value: JsonObject, key: string): boolean {
  return hasOwn(value, key);
}
