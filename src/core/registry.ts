import { ResourceLocation } from "./resource-location";

class SimpleRegistry<T extends object> implements Iterable<T> {
  private readonly valuesByKey = new Map<string, T>();
  private readonly keysByValue = new WeakMap<T, ResourceLocation>();

  public register(key: ResourceLocation, value: T): T {
    this.valuesByKey.set(key.toString(), value);
    this.keysByValue.set(value, key);
    return value;
  }

  public get(key: ResourceLocation): T | undefined {
    return this.valuesByKey.get(key.toString());
  }

  public getKey(value: T): ResourceLocation | undefined {
    return this.keysByValue.get(value);
  }

  public clear(): void {
    this.valuesByKey.clear();
  }

  public [Symbol.iterator](): Iterator<T> {
    return this.valuesByKey.values();
  }
}

export class Registry {
  public static readonly BLOCK = new SimpleRegistry<object>();

  public static register<T extends object>(registry: SimpleRegistry<T>, key: ResourceLocation | string, value: T): T {
    return registry.register(key instanceof ResourceLocation ? key : new ResourceLocation(key), value);
  }
}
