import { figure } from "../../src/dsl";

// A box-only adult giraffe with an extreme upright neck, sparse mane ridge,
// ossicones, large ears, long legs, and texture-painted coat patches. The
// reviewed quadruped gait stays grounded despite the unusually tall rig.
export default figure("giraffe", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#d8a64d");
  mat("coat_light", "#edc878");
  mat("spot", "#87502b");
  mat("mane", "#5b3523");
  mat("muzzle", "#d8b57d");
  mat("ear_inner", "#bd7d67");
  mat("ossicone", "#70432a");
  mat("hoof", "#33231d");

  asciiTexture("face", {
    palette: { ".": "#d8a64d", "s": "#87502b", "e": "#17120f", "l": "#edc878" },
    pixels: [
      "ss....ss",
      ".se..es.",
      "..e..e..",
      "...ll...",
      "..llll..",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#d8b57d", "n": "#4b3022" },
    pixels: [
      "........",
      ".nn..nn.",
      ".nn..nn.",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("body_spots", {
    palette: { ".": "#d8a64d", "s": "#87502b", "l": "#edc878" },
    pixels: [
      "ss...sss..ss",
      "ss...sss..ss",
      "...ss...ss..",
      ".ssss..sss..",
      ".ssss......s",
      ".....sss...s",
      "ss...sss....",
      "ss........ss",
    ],
  });
  asciiTexture("neck_spots", {
    palette: { ".": "#d8a64d", "s": "#87502b" },
    pixels: [
      "ss..ss",
      "ss....",
      "..sss.",
      "..sss.",
      "s.....",
      "sss.ss",
      "....ss",
      ".sss..",
      ".sss..",
      "....ss",
      "ss..ss",
      "ss....",
    ],
  });

  part("body", box({
    at: [0, 1.55, 0.06],
    size: [0.76, 0.66, 1.42],
    material: "coat",
    faces: {
      east: { texture: "body_spots" },
      west: { texture: "body_spots" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.36, 0.05],
    size: [0.62, 0.1, 1.04],
    material: "coat_light",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.95, -0.55],
    rot: [-9, 0, 0],
    size: [0.4, 1.55, 0.42],
    material: "coat",
    faces: {
      east: { texture: "neck_spots" },
      west: { texture: "neck_spots" },
    },
    joint: { pivot: [0, -0.775, 0.06], axis: [1, 0, 0] },
  }));
  part("mane", box({
    parent: "neck",
    at: [0, 0.02, 0.25],
    size: [0.13, 1.24, 0.12],
    material: "mane",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.91, -0.12],
    rot: [8, 0, 0],
    size: [0.48, 0.42, 0.58],
    material: "coat_light",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.1, -0.44],
    size: [0.38, 0.26, 0.32],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.3, 0.2, 0.04],
    rot: [0, 0, -18],
    size: [0.3, 0.15, 0.11],
    material: "ear_inner",
    joint: { pivot: [0.13, 0, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.3, 0.2, 0.04],
    rot: [0, 0, 18],
    size: [0.3, 0.15, 0.11],
    material: "ear_inner",
    joint: { pivot: [-0.13, 0, 0], axis: [1, 0, 0] },
  }));

  for (const [side, x] of [["l", -0.13], ["r", 0.13]] as const) {
    part(`ossicone_${side}`, box({
      parent: "head",
      at: [x, 0.34, 0.08],
      rot: [-4, 0, 0],
      size: [0.08, 0.28, 0.08],
      material: "ossicone",
    }));
    part(`ossicone_tip_${side}`, box({
      parent: `ossicone_${side}`,
      at: [0, 0.18, 0],
      size: [0.13, 0.12, 0.13],
      material: "mane",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.25, -0.48],
    ["fr", 0.25, -0.48],
    ["bl", -0.25, 0.5],
    ["br", 0.25, 0.5],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.86, z],
      size: [0.17, 1.05, 0.18],
      material: "coat",
      faces: {
        east: { texture: "neck_spots" },
        west: { texture: "neck_spots" },
      },
      joint: { pivot: [0, 0.525, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.59, -0.02],
      size: [0.21, 0.13, 0.24],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, -0.02, 0.78],
    rot: [-8, 0, 0],
    size: [0.08, 0.68, 0.08],
    material: "coat",
    joint: { pivot: [0, 0.34, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.42, 0.02],
    size: [0.18, 0.22, 0.16],
    material: "mane",
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.28,
    cycleDistance: 1.08,
    gait: "walk",
    loop: true,
    samples: 17,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.013,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.64,
    swingDegrees: 17,
    tail: "tail",
    tailSwingDegrees: 8,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 2.5, overshoot: 0.35, lag: 0.12 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.5, lag: 0.14 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.5, lag: 0.14 }),
    ],
  });
});
