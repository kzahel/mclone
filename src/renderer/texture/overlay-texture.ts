export class OverlayTexture {
  public static readonly NO_WHITE_U = 0;
  public static readonly RED_OVERLAY_V = 3;
  public static readonly WHITE_OVERLAY_V = 10;
  public static readonly NO_OVERLAY = OverlayTexture.pack(0, 10);

  public static u(u: number): number {
    return Math.trunc(u * 15.0);
  }

  public static v(hurt: boolean): number {
    return hurt ? OverlayTexture.RED_OVERLAY_V : OverlayTexture.WHITE_OVERLAY_V;
  }

  public static pack(u: number, v: number): number;
  public static pack(u: number, hurt: boolean): number;
  public static pack(u: number, vOrHurt: number | boolean): number {
    const v = typeof vOrHurt === "boolean" ? OverlayTexture.v(vOrHurt) : vOrHurt;
    return u | (v << 16);
  }
}
