export interface StringRepresentable {
  getSerializedName(): string;
}

export interface StringRepresentableClass<T extends StringRepresentable> {
  values(): readonly T[];
}
