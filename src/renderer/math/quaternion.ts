export class Quaternion {
  public static readonly ONE = new Quaternion(0, 0, 0, 1);

  public constructor(
    private iValue: number,
    private jValue: number,
    private kValue: number,
    private rValue: number,
  ) {}

  public i(): number {
    return this.iValue;
  }

  public j(): number {
    return this.jValue;
  }

  public k(): number {
    return this.kValue;
  }

  public r(): number {
    return this.rValue;
  }

  public mul(value: Quaternion | number): void {
    if (typeof value === "number") {
      this.iValue *= value;
      this.jValue *= value;
      this.kValue *= value;
      this.rValue *= value;
      return;
    }

    const i = this.i();
    const j = this.j();
    const k = this.k();
    const r = this.r();
    const otherI = value.i();
    const otherJ = value.j();
    const otherK = value.k();
    const otherR = value.r();
    this.iValue = (((r * otherI) + (i * otherR)) + (j * otherK)) - (k * otherJ);
    this.jValue = (((r * otherJ) - (i * otherK)) + (j * otherR)) + (k * otherI);
    this.kValue = (((r * otherK) + (i * otherJ)) - (j * otherI)) + (k * otherR);
    this.rValue = (((r * otherR) - (i * otherI)) - (j * otherJ)) - (k * otherK);
  }

  public normalize(): void {
    const lengthSquared =
      (this.i() * this.i()) + (this.j() * this.j()) + (this.k() * this.k()) + (this.r() * this.r());
    if (lengthSquared > 1.0e-6) {
      const scale = fastInvSqrt(lengthSquared);
      this.iValue *= scale;
      this.jValue *= scale;
      this.kValue *= scale;
      this.rValue *= scale;
      return;
    }

    this.iValue = 0;
    this.jValue = 0;
    this.kValue = 0;
    this.rValue = 0;
  }

  public copy(): Quaternion {
    return new Quaternion(this.iValue, this.jValue, this.kValue, this.rValue);
  }
}

function fastInvSqrt(value: number): number {
  const buffer = new ArrayBuffer(4);
  const dataView = new DataView(buffer);
  const halfValue = 0.5 * value;
  dataView.setFloat32(0, value, true);
  let bits = dataView.getInt32(0, true);
  bits = 1_597_463_007 - (bits >> 1);
  dataView.setInt32(0, bits, true);
  const approximation = dataView.getFloat32(0, true);
  return approximation * (1.5 - ((halfValue * approximation) * approximation));
}
