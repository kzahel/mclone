import type { NoiseBiome } from "./noise-biome.ts";
import type { NoiseBiomeSource } from "./noise-biome-source.ts";
import { linearCongruentialGeneratorNext } from "../util/linear-congruential-generator.ts";

const SHA256_INITIAL_STATE = [
  0x6a09e667,
  0xbb67ae85,
  0x3c6ef372,
  0xa54ff53a,
  0x510e527f,
  0x9b05688c,
  0x1f83d9ab,
  0x5be0cd19,
] as const;

const SHA256_ROUND_CONSTANTS = [
  0x428a2f98,
  0x71374491,
  0xb5c0fbcf,
  0xe9b5dba5,
  0x3956c25b,
  0x59f111f1,
  0x923f82a4,
  0xab1c5ed5,
  0xd807aa98,
  0x12835b01,
  0x243185be,
  0x550c7dc3,
  0x72be5d74,
  0x80deb1fe,
  0x9bdc06a7,
  0xc19bf174,
  0xe49b69c1,
  0xefbe4786,
  0x0fc19dc6,
  0x240ca1cc,
  0x2de92c6f,
  0x4a7484aa,
  0x5cb0a9dc,
  0x76f988da,
  0x983e5152,
  0xa831c66d,
  0xb00327c8,
  0xbf597fc7,
  0xc6e00bf3,
  0xd5a79147,
  0x06ca6351,
  0x14292967,
  0x27b70a85,
  0x2e1b2138,
  0x4d2c6dfc,
  0x53380d13,
  0x650a7354,
  0x766a0abb,
  0x81c2c92e,
  0x92722c85,
  0xa2bfe8a1,
  0xa81a664b,
  0xc24b8b70,
  0xc76c51a3,
  0xd192e819,
  0xd6990624,
  0xf40e3585,
  0x106aa070,
  0x19a4c116,
  0x1e376c08,
  0x2748774c,
  0x34b0bcb5,
  0x391c0cb3,
  0x4ed8aa4a,
  0x5b9cca4f,
  0x682e6ff3,
  0x748f82ee,
  0x78a5636f,
  0x84c87814,
  0x8cc70208,
  0x90befffa,
  0xa4506ceb,
  0xbef9a3f7,
  0xc67178f2,
] as const;

function rotateRight(value: number, shift: number): number {
  return (value >>> shift) | (value << (32 - shift));
}

function floorMod(value: bigint, divisor: bigint): bigint {
  const remainder = value % divisor;
  return remainder >= 0n ? remainder : remainder + divisor;
}

function sha256(message: Uint8Array): Uint8Array {
  const bitLength = message.length * 8;
  const totalLength = Math.ceil((message.length + 9) / 64) * 64;
  const padded = new Uint8Array(totalLength);
  padded.set(message);
  padded[message.length] = 0x80;
  const view = new DataView(padded.buffer);
  view.setUint32(totalLength - 8, Math.floor(bitLength / 0x1_0000_0000), false);
  view.setUint32(totalLength - 4, bitLength >>> 0, false);

  const schedule = new Uint32Array(64);
  let h0: number = SHA256_INITIAL_STATE[0];
  let h1: number = SHA256_INITIAL_STATE[1];
  let h2: number = SHA256_INITIAL_STATE[2];
  let h3: number = SHA256_INITIAL_STATE[3];
  let h4: number = SHA256_INITIAL_STATE[4];
  let h5: number = SHA256_INITIAL_STATE[5];
  let h6: number = SHA256_INITIAL_STATE[6];
  let h7: number = SHA256_INITIAL_STATE[7];

  for (let blockOffset = 0; blockOffset < totalLength; blockOffset += 64) {
    for (let index = 0; index < 16; index++) {
      schedule[index] = view.getUint32(blockOffset + (index * 4), false);
    }

    for (let index = 16; index < 64; index++) {
      const s0 =
        rotateRight(schedule[index - 15]!, 7) ^
        rotateRight(schedule[index - 15]!, 18) ^
        (schedule[index - 15]! >>> 3);
      const s1 =
        rotateRight(schedule[index - 2]!, 17) ^
        rotateRight(schedule[index - 2]!, 19) ^
        (schedule[index - 2]! >>> 10);
      schedule[index] = (schedule[index - 16]! + s0 + schedule[index - 7]! + s1) >>> 0;
    }

    let a = h0;
    let b = h1;
    let c = h2;
    let d = h3;
    let e = h4;
    let f = h5;
    let g = h6;
    let h = h7;

    for (let index = 0; index < 64; index++) {
      const sum1 = rotateRight(e, 6) ^ rotateRight(e, 11) ^ rotateRight(e, 25);
      const choice = (e & f) ^ (~e & g);
      const temp1 = (h + sum1 + choice + SHA256_ROUND_CONSTANTS[index]! + schedule[index]!) >>> 0;
      const sum0 = rotateRight(a, 2) ^ rotateRight(a, 13) ^ rotateRight(a, 22);
      const majority = (a & b) ^ (a & c) ^ (b & c);
      const temp2 = (sum0 + majority) >>> 0;

      h = g;
      g = f;
      f = e;
      e = (d + temp1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (temp1 + temp2) >>> 0;
    }

    h0 = (h0 + a) >>> 0;
    h1 = (h1 + b) >>> 0;
    h2 = (h2 + c) >>> 0;
    h3 = (h3 + d) >>> 0;
    h4 = (h4 + e) >>> 0;
    h5 = (h5 + f) >>> 0;
    h6 = (h6 + g) >>> 0;
    h7 = (h7 + h) >>> 0;
  }

  const digest = new Uint8Array(32);
  const digestView = new DataView(digest.buffer);
  digestView.setUint32(0, h0, false);
  digestView.setUint32(4, h1, false);
  digestView.setUint32(8, h2, false);
  digestView.setUint32(12, h3, false);
  digestView.setUint32(16, h4, false);
  digestView.setUint32(20, h5, false);
  digestView.setUint32(24, h6, false);
  digestView.setUint32(28, h7, false);
  return digest;
}

function toLittleEndianLongBytes(seed: bigint): Uint8Array {
  const wrapped = BigInt.asIntN(64, seed);
  const bytes = new Uint8Array(8);
  for (let index = 0; index < bytes.length; index++) {
    bytes[index] = Number((wrapped >> BigInt(index * 8)) & 0xffn);
  }
  return bytes;
}

function fromLittleEndianLongBytes(bytes: Uint8Array): bigint {
  let value = 0n;
  for (let index = 0; index < 8; index++) {
    value |= BigInt(bytes[index]!) << BigInt(index * 8);
  }
  return BigInt.asIntN(64, value);
}

function getFiddle(seed: bigint): number {
  const scaled = Number(floorMod(seed >> 24n, 1024n));
  return ((scaled / 1024.0) - 0.5) * 0.9;
}

function getFiddledDistance(seed: bigint, x: number, y: number, z: number, scaleX: number, scaleY: number, scaleZ: number): number {
  let value = linearCongruentialGeneratorNext(seed, x);
  value = linearCongruentialGeneratorNext(value, y);
  value = linearCongruentialGeneratorNext(value, z);
  value = linearCongruentialGeneratorNext(value, x);
  value = linearCongruentialGeneratorNext(value, y);
  value = linearCongruentialGeneratorNext(value, z);
  const fiddleX = getFiddle(value);
  value = linearCongruentialGeneratorNext(value, seed);
  const fiddleY = getFiddle(value);
  value = linearCongruentialGeneratorNext(value, seed);
  const fiddleZ = getFiddle(value);
  return ((scaleZ + fiddleZ) ** 2) + ((scaleY + fiddleY) ** 2) + ((scaleX + fiddleX) ** 2);
}

export function obfuscateBiomeZoomSeed(seed: bigint): bigint {
  return fromLittleEndianLongBytes(sha256(toLittleEndianLongBytes(seed)).subarray(0, 8));
}

export function getFuzzyZoomedBiome(
  zoomSeed: bigint,
  blockX: number,
  blockY: number,
  blockZ: number,
  biomeSource: NoiseBiomeSource,
): NoiseBiome {
  const shiftedX = blockX - 2;
  const shiftedY = blockY - 2;
  const shiftedZ = blockZ - 2;
  const baseQuartX = shiftedX >> 2;
  const baseQuartY = shiftedY >> 2;
  const baseQuartZ = shiftedZ >> 2;
  const offsetX = (shiftedX & 3) / 4.0;
  const offsetY = (shiftedY & 3) / 4.0;
  const offsetZ = (shiftedZ & 3) / 4.0;

  let bestCorner = 0;
  let bestDistance = Number.POSITIVE_INFINITY;

  for (let corner = 0; corner < 8; corner++) {
    const useBaseX = (corner & 4) === 0;
    const useBaseY = (corner & 2) === 0;
    const useBaseZ = (corner & 1) === 0;
    const quartX = useBaseX ? baseQuartX : baseQuartX + 1;
    const quartY = useBaseY ? baseQuartY : baseQuartY + 1;
    const quartZ = useBaseZ ? baseQuartZ : baseQuartZ + 1;
    const scaleX = useBaseX ? offsetX : offsetX - 1.0;
    const scaleY = useBaseY ? offsetY : offsetY - 1.0;
    const scaleZ = useBaseZ ? offsetZ : offsetZ - 1.0;
    const distance = getFiddledDistance(zoomSeed, quartX, quartY, quartZ, scaleX, scaleY, scaleZ);
    if (distance < bestDistance) {
      bestCorner = corner;
      bestDistance = distance;
    }
  }

  const quartX = (bestCorner & 4) === 0 ? baseQuartX : baseQuartX + 1;
  const quartY = (bestCorner & 2) === 0 ? baseQuartY : baseQuartY + 1;
  const quartZ = (bestCorner & 1) === 0 ? baseQuartZ : baseQuartZ + 1;
  return biomeSource.getNoiseBiome(quartX, quartY, quartZ);
}

export function getBlockPositionBiome(seed: bigint, blockX: number, blockZ: number, biomeSource: NoiseBiomeSource): NoiseBiome {
  return getFuzzyZoomedBiome(obfuscateBiomeZoomSeed(seed), blockX, 0, blockZ, biomeSource);
}
