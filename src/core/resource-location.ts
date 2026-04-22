import { ResourceLocationException } from "./resource-location-exception";

const NAMESPACE_SEPARATOR = ":";
const DEFAULT_NAMESPACE = "minecraft";

function isEmpty(value: string): boolean {
  return value.length === 0;
}

function validPathChar(char: string): boolean {
  return (
    char === "_" ||
    char === "-" ||
    (char >= "a" && char <= "z") ||
    (char >= "0" && char <= "9") ||
    char === "/" ||
    char === "."
  );
}

function validNamespaceChar(char: string): boolean {
  return char === "_" || char === "-" || (char >= "a" && char <= "z") || (char >= "0" && char <= "9") || char === ".";
}

function isValidPath(path: string): boolean {
  for (let index = 0; index < path.length; index++) {
    if (!validPathChar(path[index]!)) {
      return false;
    }
  }

  return true;
}

function isValidNamespace(namespace: string): boolean {
  for (let index = 0; index < namespace.length; index++) {
    if (!validNamespaceChar(namespace[index]!)) {
      return false;
    }
  }

  return true;
}

function decompose(location: string, separator: string): readonly [string, string] {
  const result: [string, string] = [DEFAULT_NAMESPACE, location];
  const separatorIndex = location.indexOf(separator);
  if (separatorIndex >= 0) {
    result[1] = location.substring(separatorIndex + 1);
    if (separatorIndex >= 1) {
      result[0] = location.substring(0, separatorIndex);
    }
  }

  return result;
}

export class ResourceLocation {
  public static readonly NAMESPACE_SEPARATOR = NAMESPACE_SEPARATOR;
  public static readonly DEFAULT_NAMESPACE = DEFAULT_NAMESPACE;
  public static readonly REALMS_NAMESPACE = "realms";

  public readonly namespace: string;
  public readonly path: string;

  public constructor(location: string);
  public constructor(namespace: string, path: string);
  public constructor(namespaceOrLocation: string, path?: string) {
    const components = path === undefined ? decompose(namespaceOrLocation, NAMESPACE_SEPARATOR) : [namespaceOrLocation, path];
    this.namespace = isEmpty(components[0]!) ? DEFAULT_NAMESPACE : components[0]!;
    this.path = components[1]!;

    if (!isValidNamespace(this.namespace)) {
      throw new ResourceLocationException(`Non [a-z0-9_.-] character in namespace of location: ${this.namespace}:${this.path}`);
    }

    if (!isValidPath(this.path)) {
      throw new ResourceLocationException(`Non [a-z0-9/._-] character in path of location: ${this.namespace}:${this.path}`);
    }
  }

  public static of(value: string, separator: string): ResourceLocation {
    const components = decompose(value, separator);
    return new ResourceLocation(components[0]!, components[1]!);
  }

  public static tryParse(value: string): ResourceLocation | undefined {
    try {
      return new ResourceLocation(value);
    } catch (error) {
      if (error instanceof ResourceLocationException) {
        return undefined;
      }

      throw error;
    }
  }

  public static isAllowedInResourceLocation(char: string): boolean {
    return (
      (char >= "0" && char <= "9") ||
      (char >= "a" && char <= "z") ||
      char === "_" ||
      char === ":" ||
      char === "/" ||
      char === "." ||
      char === "-"
    );
  }

  public static isValidResourceLocation(value: string): boolean {
    const components = decompose(value, NAMESPACE_SEPARATOR);
    return isValidNamespace(isEmpty(components[0]!) ? DEFAULT_NAMESPACE : components[0]!) && isValidPath(components[1]!);
  }

  public static validPathChar(char: string): boolean {
    return validPathChar(char);
  }

  public getPath(): string {
    return this.path;
  }

  public getNamespace(): string {
    return this.namespace;
  }

  public equals(other: unknown): boolean {
    return other instanceof ResourceLocation && this.namespace === other.namespace && this.path === other.path;
  }

  public hashCode(): string {
    return `${this.namespace}:${this.path}`;
  }

  public compareTo(other: ResourceLocation): number {
    const pathCompare = this.path.localeCompare(other.path);
    if (pathCompare === 0) {
      return this.namespace.localeCompare(other.namespace);
    }

    return pathCompare;
  }

  public toDebugFileName(): string {
    return this.toString().replaceAll("/", "_").replaceAll(":", "_");
  }

  public toString(): string {
    return `${this.namespace}:${this.path}`;
  }
}
