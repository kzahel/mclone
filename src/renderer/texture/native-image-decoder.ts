import { NativeImage } from "./native-image";

export interface NativeImageDecoder {
  decode(blob: Blob): Promise<NativeImage>;
}

export async function loadNativeImageFromBlobWithDecoder(
  blob: Blob,
  decoder: NativeImageDecoder,
): Promise<NativeImage> {
  return decoder.decode(blob);
}

export async function loadNativeImageFromUrlWithDecoder(
  url: string,
  decoder: NativeImageDecoder,
): Promise<NativeImage> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Unable to load image ${url}: ${response.status} ${response.statusText}`);
  }

  return decoder.decode(await response.blob());
}
