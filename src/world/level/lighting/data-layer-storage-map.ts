import { DataLayer } from "../chunk/data-layer";
import { LIGHT_SELF_SOURCE } from "./section-tracker";

export abstract class DataLayerStorageMap<M extends DataLayerStorageMap<M>> {
  private readonly lastSectionKeys: bigint[] = [LIGHT_SELF_SOURCE, LIGHT_SELF_SOURCE];
  private readonly lastSections: Array<DataLayer | undefined> = [undefined, undefined];
  private cacheEnabled = true;

  protected constructor(protected readonly map: Map<bigint, DataLayer>) {}

  public abstract copy(): M;

  public copyDataLayer(section: bigint): void {
    const layer = this.map.get(section);
    if (layer === undefined) {
      throw new Error(`Cannot copy missing light data layer ${section.toString()}`);
    }
    this.map.set(section, layer.copy());
    this.clearCache();
  }

  public hasLayer(section: bigint): boolean {
    return this.map.has(section);
  }

  public getLayer(section: bigint): DataLayer | undefined {
    if (this.cacheEnabled) {
      for (let index = 0; index < 2; index++) {
        if (section === this.lastSectionKeys[index]) {
          return this.lastSections[index];
        }
      }
    }

    const layer = this.map.get(section);
    if (layer === undefined) {
      return undefined;
    }

    if (this.cacheEnabled) {
      this.lastSectionKeys[1] = this.lastSectionKeys[0]!;
      this.lastSections[1] = this.lastSections[0];
      this.lastSectionKeys[0] = section;
      this.lastSections[0] = layer;
    }

    return layer;
  }

  public removeLayer(section: bigint): DataLayer | undefined {
    const layer = this.map.get(section);
    this.map.delete(section);
    this.clearCache();
    return layer;
  }

  public setLayer(section: bigint, layer: DataLayer): void {
    this.map.set(section, layer);
    this.clearCache();
  }

  public clearCache(): void {
    for (let index = 0; index < 2; index++) {
      this.lastSectionKeys[index] = LIGHT_SELF_SOURCE;
      this.lastSections[index] = undefined;
    }
  }

  public disableCache(): void {
    this.cacheEnabled = false;
  }

  protected cloneLayerMap(): Map<bigint, DataLayer> {
    return new Map(this.map);
  }
}
