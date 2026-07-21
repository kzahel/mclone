import { figure } from "../../src/dsl";

// A box-only blue wildebeest with massive dark shoulders, narrow rear,
// lowered long face, hanging beard, sweeping three-stage horns, and black tail.
export default figure("wildebeest", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#756f62");
  mat("coat_light", "#999183");
  mat("coat_dark", "#3c3a35");
  mat("black", "#201f1c");
  mat("muzzle", "#514d45");
  mat("horn", "#786f5c");
  mat("horn_tip", "#35312a");
  mat("hoof", "#211f1c");
  mat("ear_inner", "#806861");

  asciiTexture("face", {
    palette: { ".": "#3c3a35", "b": "#201f1c", "e": "#c7a94a", "l": "#756f62" },
    pixels: [
      "bbbbbbbb",
      "b.e..e.b",
      "..e..e..",
      ".llllll.",
      "..llll..",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#514d45", "n": "#181714", "l": "#756f62" },
    pixels: [
      "..llll..",
      ".llllll.",
      ".nn..nn.",
      "..nnnn..",
      "........",
    ],
  });
  asciiTexture("shoulder_stripes", {
    palette: { ".": "#756f62", "d": "#3c3a35", "b": "#201f1c", "l": "#999183" },
    pixels: [
      "bbbbbbbbbbbbbb",
      "bddddddddddd.b",
      "d.d.d.d.d.d...",
      ".d.d.d.d.d....",
      "....llllllllll",
      "dddd........dd",
    ],
  });
  asciiTexture("leg_dark", {
    palette: { ".": "#756f62", "d": "#3c3a35", "b": "#201f1c" },
    pixels: ["....", ".d..", "d...", "dddd", "bbbb", "bbbb"],
  });

  part("body", box({
    at: [0, 1.14, 0.12],
    rot: [-3, 0, 0],
    size: [0.9, 0.68, 1.5],
    material: "coat",
    faces: {
      east: { texture: "shoulder_stripes" },
      west: { texture: "shoulder_stripes" },
    },
  }));
  part("shoulder_hump", box({
    parent: "body",
    at: [0, 0.3, -0.52],
    size: [1.02, 0.66, 0.72],
    material: "coat_dark",
    faces: {
      east: { texture: "shoulder_stripes" },
      west: { texture: "shoulder_stripes" },
    },
  }));
  part("rump", box({
    parent: "body",
    at: [0, -0.12, 0.59],
    size: [0.78, 0.56, 0.62],
    material: "coat_light",
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.22, -0.7],
    size: [0.84, 0.66, 0.28],
    material: "coat_dark",
  }));
  part("mane", box({
    parent: "shoulder_hump",
    at: [0, 0.39, 0.03],
    size: [0.18, 0.18, 0.66],
    material: "black",
  }));
  part("neck", box({
    parent: "shoulder_hump",
    at: [0, -0.04, -0.57],
    rot: [24, 0, 0],
    size: [0.58, 0.78, 0.5],
    material: "coat_dark",
    joint: { pivot: [0, 0.35, 0.12], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, -0.37, -0.28],
    rot: [-12, 0, 0],
    size: [0.58, 0.68, 0.52],
    material: "coat_dark",
    faces: { north: { texture: "face" } },
  }));
  part("forelock", box({
    parent: "head",
    at: [0, 0.38, -0.06],
    size: [0.42, 0.18, 0.38],
    material: "black",
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.2, -0.43],
    size: [0.48, 0.3, 0.36],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("beard", box({
    parent: "muzzle",
    at: [0, -0.3, 0.02],
    rot: [8, 0, 0],
    size: [0.32, 0.4, 0.22],
    material: "black",
    joint: { pivot: [0, 0.18, 0], axis: [1, 0, 0] },
  }));
  for (const [side, x] of [["l", -0.31], ["r", 0.31]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.22, 0.06],
      rot: [-3, 0, side === "l" ? -20 : 20],
      size: [0.25, 0.15, 0.12],
      material: "ear_inner",
      joint: { pivot: [side === "l" ? 0.1 : -0.1, 0, 0], axis: [1, 0, 0] },
    }));
    part(`horn_${side}_1`, box({
      parent: "head",
      at: [side === "l" ? -0.27 : 0.27, 0.3, 0.06],
      rot: [0, 0, side === "l" ? 62 : -62],
      size: [0.14, 0.38, 0.14],
      material: "horn",
    }));
    part(`horn_${side}_2`, box({
      parent: `horn_${side}_1`,
      at: [0, 0.31, 0],
      rot: [0, 0, side === "l" ? -47 : 47],
      size: [0.11, 0.32, 0.11],
      material: "horn",
    }));
    part(`horn_${side}_3`, box({
      parent: `horn_${side}_2`,
      at: [0, 0.26, 0],
      rot: [-8, 0, side === "l" ? -12 : 12],
      size: [0.075, 0.28, 0.075],
      material: "horn_tip",
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.31, -0.49, false],
    ["fr", 0.31, -0.49, false],
    ["bl", -0.29, 0.52, true],
    ["br", 0.29, 0.52, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, rear ? -0.61 : -0.64, z],
      size: [rear ? 0.18 : 0.23, rear ? 0.64 : 0.72, rear ? 0.19 : 0.24],
      material: rear ? "coat" : "coat_dark",
      faces: {
        east: { texture: "leg_dark" },
        west: { texture: "leg_dark" },
      },
      joint: { pivot: [0, rear ? 0.32 : 0.36, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.38 : -0.42, -0.05],
      size: [rear ? 0.23 : 0.28, 0.13, rear ? 0.29 : 0.34],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.07, 0.39],
    rot: [-5, 0, 0],
    size: [0.11, 0.62, 0.11],
    material: "black",
    joint: { pivot: [0, 0.29, 0], axis: [0, 0, 1] },
  }));
  part("tail_brush", box({
    parent: "tail",
    at: [0, -0.4, 0.03],
    size: [0.24, 0.28, 0.22],
    material: "black",
  }));

  quadrupedWalk("trudge", {
    fps: 18,
    duration: 1.16,
    cycleDistance: 0.82,
    gait: "walk",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.015,
    bodyBobCenter: 0.017,
    head: "neck",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.72,
    swingDegrees: 15,
    tail: "tail",
    tailSwingDegrees: 7,
    tracks: [
      followThrough("beard", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.65, lag: 0.17 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.11 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.11 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.65, lag: 0.18 }),
    ],
  });
});
