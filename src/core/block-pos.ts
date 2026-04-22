import { Vec3i } from "./vec3i";

export class BlockPos extends Vec3i {
  public below(): BlockPos {
    return new BlockPos(this.getX(), this.getY() - 1, this.getZ());
  }
}
