import { figure } from "../../src/dsl";

// A broad, short-backed domestic dog. Floppy ears, a bright collar, and wide
// paws distinguish it from the taller wolf and narrow fox using only cuboids.
export default figure("dog", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
}) => {
  mat("fur", "#8b5f3e");
  mat("fur_dark", "#5b3928");
  mat("fur_light", "#c39161");
  mat("collar", "#2f69b1");
  mat("paw", "#3a2720");

  asciiTexture("face", {
    palette: { ".": "#8b5f3e", "e": "#120d0b", "m": "#c39161" },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "..mmmm..",
      "..mmmm..",
      "........",
      "........",
    ],
  });
  asciiTexture("snout", {
    palette: { ".": "#c39161", "n": "#120d0b" },
    pixels: [
      "........",
      "........",
      "...nn...",
      "..nnnn..",
      "........",
      "........",
    ],
  });
  asciiTexture("paw_front", {
    palette: { "p": "#3a2720", "c": "#d9b184" },
    pixels: [
      "pppppp",
      "pcpccp",
      "pppppp",
    ],
  });

  part("body", box({
    at: [0, 0.86, 0],
    size: [1.12, 0.58, 0.94],
    material: "fur",
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.02, -0.46],
    size: [1.02, 0.52, 0.2],
    material: "fur_light",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.13, -0.58],
    size: [0.5, 0.36, 0.28],
    material: "fur",
  }));
  part("collar", box({
    parent: "neck",
    at: [0, -0.04, -0.04],
    size: [0.54, 0.1, 0.31],
    material: "collar",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.16, -0.34],
    size: [0.56, 0.46, 0.48],
    material: "fur",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.21], axis: [1, 0, 0] },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.09, -0.34],
    size: [0.36, 0.2, 0.23],
    material: "fur_light",
    faces: { north: { texture: "snout" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.27, 0.12, 0],
    rot: [0, 0, 11],
    size: [0.16, 0.36, 0.12],
    material: "fur_dark",
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.27, 0.12, 0],
    rot: [0, 0, -11],
    size: [0.16, 0.36, 0.12],
    material: "fur_dark",
  }));

  for (const [suffix, x, z, dark] of [
    ["fl", -0.36, -0.3, false],
    ["fr", 0.36, -0.3, false],
    ["bl", -0.36, 0.3, true],
    ["br", 0.36, 0.3, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.49, z],
      size: [0.16, 0.44, 0.17],
      material: dark ? "fur_dark" : "fur",
      joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.27, -0.04],
      size: [0.21, 0.1, 0.25],
      material: "paw",
      faces: { north: { texture: "paw_front" } },
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.38, 0.48],
    rot: [55, 0, 0],
    size: [0.13, 0.5, 0.13],
    material: "fur_dark",
    joint: { pivot: [0, -0.25, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 0.9,
    cycleDistance: 0.88,
    gait: "trot",
    loop: true,
    samples: 11,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.014,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 3,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.56,
    swingDegrees: 22,
    tail: "tail",
    tailSwingDegrees: 16,
  });
});
