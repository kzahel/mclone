import { Heightmap } from "../../heightmap";
import type { DecoratorConfiguration } from "./decorator-configuration";

export class HeightmapConfiguration implements DecoratorConfiguration {
  public constructor(public readonly heightmap: Heightmap.Types) {}
}
