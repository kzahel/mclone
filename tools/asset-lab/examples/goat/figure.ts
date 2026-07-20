import { figure } from "../../src/dsl";

// A box-only billy goat. Stepped horns, a hanging beard, splayed ears, and
// narrow legs carry the species read without curved primitives.
export default figure("goat", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#e7e0d2");
  mat("coat_shadow", "#cdc4b1");
  mat("muzzle", "#c9bda6");
  mat("ear_inner", "#b8a98f");
  mat("horn", "#a89574");
  mat("hoof", "#2b2420");
  mat("beard", "#8d7a5f");

  asciiTexture("face", {
    palette: { ".": "#e7e0d2", "e": "#16110f", "b": "#8d7a5f" },
    pixels: [
      "b......b",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#c9bda6", "n": "#3a2f26" },
    pixels: [
      "........",
      "........",
      "..n..n..",
      "..n..n..",
      "........",
      "........",
    ],
  });
  asciiTexture("cloven_front", {
    palette: { "h": "#2b2420", "g": "#0f0d0c" },
    pixels: [
      "hhhhhh",
      "hhgghh",
      "hhgghh",
    ],
  });

  part("body", box({
    at: [0, 0.96, 0],
    size: [0.86, 0.62, 1.12],
    material: "coat",
  }));
  part("saddle", box({
    parent: "body",
    at: [0, 0.33, 0.06],
    size: [0.78, 0.1, 0.92],
    material: "coat_shadow",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.32, 0.04],
    size: [0.7, 0.08, 0.86],
    material: "coat_shadow",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.2, -0.56],
    rot: [-14, 0, 0],
    size: [0.44, 0.46, 0.5],
    material: "coat",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.14, -0.36],
    size: [0.42, 0.42, 0.48],
    material: "coat",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.21], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.1, -0.34],
    size: [0.3, 0.24, 0.26],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.29, 0.11, 0.03],
    rot: [0, 0, -35],
    size: [0.28, 0.11, 0.13],
    material: "ear_inner",
    joint: { pivot: [0.12, 0, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.29, 0.11, 0.03],
    rot: [0, 0, 35],
    size: [0.28, 0.11, 0.13],
    material: "ear_inner",
    joint: { pivot: [-0.12, 0, 0], axis: [1, 0, 0] },
  }));

  for (const [side, x, yaw] of [["l", -0.13, 8], ["r", 0.13, -8]] as const) {
    part(`horn_${side}`, box({
      parent: "head",
      at: [x, 0.29, 0.13],
      rot: [31, 0, yaw],
      size: [0.12, 0.32, 0.12],
      material: "horn",
    }));
    part(`horn_${side}_tip`, box({
      parent: `horn_${side}`,
      at: [0, 0.21, 0.07],
      rot: [28, 0, 0],
      size: [0.075, 0.25, 0.075],
      material: "horn",
    }));
  }

  part("beard", box({
    parent: "muzzle",
    at: [0, -0.22, -0.05],
    rot: [12, 0, 0],
    size: [0.15, 0.28, 0.11],
    material: "beard",
    joint: { pivot: [0, 0.14, 0], axis: [1, 0, 0] },
  }));
  part("beard_tip", box({
    parent: "beard",
    at: [0, -0.2, 0],
    size: [0.09, 0.16, 0.08],
    material: "beard",
  }));

  for (const [suffix, x, z] of [
    ["fl", -0.3, -0.34],
    ["fr", 0.3, -0.34],
    ["bl", -0.3, 0.36],
    ["br", 0.3, 0.36],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.52, z],
      size: [0.15, 0.48, 0.16],
      material: "coat",
      joint: { pivot: [0, 0.24, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.3, -0.03],
      size: [0.17, 0.1, 0.22],
      material: "hoof",
      faces: { north: { texture: "cloven_front" } },
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.31, 0.59],
    rot: [-48, 0, 0],
    size: [0.13, 0.3, 0.13],
    material: "coat",
    joint: { pivot: [0, -0.15, 0], axis: [1, 0, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0.19, -0.03],
    size: [0.17, 0.15, 0.17],
    material: "coat_shadow",
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1,
    cycleDistance: 0.72,
    gait: "walk",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.01,
    bodyBobCenter: 0.012,
    head: "head",
    headSwingDegrees: 2.5,
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
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 13, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 13, overshoot: 0.6, lag: 0.12 }),
      followThrough("beard", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.7, lag: 0.16 }),
    ],
  });
});
