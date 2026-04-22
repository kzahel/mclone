import { Direction } from "../../core/direction";

const FACE_INFO_MAX_Z = Direction.SOUTH.get3DDataValue();
const FACE_INFO_MAX_Y = Direction.UP.get3DDataValue();
const FACE_INFO_MAX_X = Direction.EAST.get3DDataValue();
const FACE_INFO_MIN_Z = Direction.NORTH.get3DDataValue();
const FACE_INFO_MIN_Y = Direction.DOWN.get3DDataValue();
const FACE_INFO_MIN_X = Direction.WEST.get3DDataValue();

export class FaceInfoVertexInfo {
  public constructor(
    public readonly xFace: number,
    public readonly yFace: number,
    public readonly zFace: number,
  ) {}
}

export class FaceInfo {
  private readonly infos: readonly FaceInfoVertexInfo[];

  public static readonly DOWN = new FaceInfo(
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MIN_Y, FACE_INFO_MAX_Z),
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MIN_Y, FACE_INFO_MIN_Z),
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MIN_Y, FACE_INFO_MIN_Z),
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MIN_Y, FACE_INFO_MAX_Z),
  );
  public static readonly UP = new FaceInfo(
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MAX_Y, FACE_INFO_MIN_Z),
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MAX_Y, FACE_INFO_MAX_Z),
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MAX_Y, FACE_INFO_MAX_Z),
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MAX_Y, FACE_INFO_MIN_Z),
  );
  public static readonly NORTH = new FaceInfo(
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MAX_Y, FACE_INFO_MIN_Z),
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MIN_Y, FACE_INFO_MIN_Z),
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MIN_Y, FACE_INFO_MIN_Z),
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MAX_Y, FACE_INFO_MIN_Z),
  );
  public static readonly SOUTH = new FaceInfo(
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MAX_Y, FACE_INFO_MAX_Z),
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MIN_Y, FACE_INFO_MAX_Z),
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MIN_Y, FACE_INFO_MAX_Z),
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MAX_Y, FACE_INFO_MAX_Z),
  );
  public static readonly WEST = new FaceInfo(
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MAX_Y, FACE_INFO_MIN_Z),
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MIN_Y, FACE_INFO_MIN_Z),
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MIN_Y, FACE_INFO_MAX_Z),
    new FaceInfoVertexInfo(FACE_INFO_MIN_X, FACE_INFO_MAX_Y, FACE_INFO_MAX_Z),
  );
  public static readonly EAST = new FaceInfo(
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MAX_Y, FACE_INFO_MAX_Z),
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MIN_Y, FACE_INFO_MAX_Z),
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MIN_Y, FACE_INFO_MIN_Z),
    new FaceInfoVertexInfo(FACE_INFO_MAX_X, FACE_INFO_MAX_Y, FACE_INFO_MIN_Z),
  );

  private static readonly BY_FACING = [
    FaceInfo.DOWN,
    FaceInfo.UP,
    FaceInfo.NORTH,
    FaceInfo.SOUTH,
    FaceInfo.WEST,
    FaceInfo.EAST,
  ] as const;

  private constructor(...infos: readonly FaceInfoVertexInfo[]) {
    this.infos = infos;
  }

  public static fromFacing(direction: Direction): FaceInfo {
    return FaceInfo.BY_FACING[direction.get3DDataValue()]!;
  }

  public getVertexInfo(index: number): FaceInfoVertexInfo {
    return this.infos[index]!;
  }
}

export namespace FaceInfo {
  export class Constants {
    public static readonly MAX_Z = FACE_INFO_MAX_Z;
    public static readonly MAX_Y = FACE_INFO_MAX_Y;
    public static readonly MAX_X = FACE_INFO_MAX_X;
    public static readonly MIN_Z = FACE_INFO_MIN_Z;
    public static readonly MIN_Y = FACE_INFO_MIN_Y;
    public static readonly MIN_X = FACE_INFO_MIN_X;
  }
}
