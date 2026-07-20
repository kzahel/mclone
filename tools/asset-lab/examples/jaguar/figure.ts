import { figure } from "../../src/dsl";

// A box-only adult jaguar with overlapping shoulder and hip masses, short
// powerful legs, a broad cheeked head, and sparse rosette-patterned coat faces.
export default figure("jaguar", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#c88b2f");
  mat("coat_light", "#d6a13f");
  mat("coat_shadow", "#a76828");
  mat("rosette", "#332117");
  mat("cream", "#ead49b");
  mat("paw", "#4c311f");
  mat("ear_inner", "#a65f48");

  asciiTexture("face", {
    palette: { ".": "#c88b2f", "r": "#332117", "e": "#d5bc55", "c": "#ead49b" },
    pixels: [
      "rr....rr",
      "r.r..r.r",
      ".re..er.",
      "..r..r..",
      ".ccrrcc.",
      ".cccccc.",
      "..cccc..",
      "........",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#ead49b", "r": "#332117", "n": "#17110e" },
    pixels: [
      ".r....r.",
      "..rrrr..",
      "..nnnn..",
      ".nnnnnn.",
      "...nn...",
      "........",
    ],
  });
  asciiTexture("rosette_side", {
    palette: { ".": "#c88b2f", "r": "#332117", "l": "#dda94b", "c": "#ead49b" },
    pixels: [
      ".rr...rr...rr..",
      "r..r.r..r.r..r.",
      ".rr...rr...rr..",
      "...rr...rr...r.",
      "l.r..r.r..r..l.",
      "ccccccccccccccc",
    ],
  });
  asciiTexture("rosette_top", {
    palette: { ".": "#c88b2f", "r": "#332117", "l": "#dda94b" },
    pixels: [
      ".rr...rr..",
      "r..r.r..r.",
      ".rr...rr..",
      "...llll...",
      ".rr...rr..",
    ],
  });
  asciiTexture("rosette_patch", {
    palette: { ".": "#c88b2f", "r": "#332117" },
    pixels: [
      ".rr..rr.",
      "r..rr..r",
      ".rr..rr.",
      "...rr...",
      ".r....r.",
      "..rr....",
    ],
  });
  asciiTexture("leg_rosettes", {
    palette: { ".": "#c88b2f", "r": "#332117", "d": "#925722" },
    pixels: [
      ".rr.",
      "r..r",
      ".rr.",
      "....",
      ".rr.",
      "r..r",
      "dddd",
      "dddd",
    ],
  });
  asciiTexture("tail_rosettes", {
    palette: { ".": "#c88b2f", "r": "#332117" },
    pixels: [
      ".rr.",
      "r..r",
      ".rr.",
      "....",
      ".rr.",
      "rrrr",
      "....",
      "rrrr",
    ],
  });

  part("body", box({
    at: [0, 1.02, 0.04],
    size: [1.06, 0.66, 1.5],
    material: "coat",
    faces: {
      east: { texture: "rosette_side" },
      west: { texture: "rosette_side" },
      up: { texture: "rosette_top" },
    },
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.12, -0.52],
    size: [1.12, 0.64, 0.56],
    material: "coat_light",
    faces: {
      east: { texture: "rosette_patch" },
      west: { texture: "rosette_patch" },
    },
  }));
  part("hips", box({
    parent: "body",
    at: [0, 0.03, 0.52],
    size: [1.1, 0.67, 0.58],
    material: "coat_shadow",
    faces: {
      east: { texture: "rosette_patch" },
      west: { texture: "rosette_patch" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.35, -0.02],
    size: [0.78, 0.12, 1.02],
    material: "cream",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.12, -0.8],
    rot: [-9, 0, 0],
    size: [0.72, 0.46, 0.42],
    material: "coat_shadow",
    joint: { pivot: [0, -0.2, 0.14], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.08, -0.4],
    rot: [6, 0, 0],
    size: [0.76, 0.58, 0.58],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("brow", box({
    parent: "head",
    at: [0, 0.15, -0.32],
    size: [0.54, 0.07, 0.07],
    material: "coat_shadow",
  }));
  for (const [side, x] of [["l", -0.36], ["r", 0.36]] as const) {
    part(`cheek_${side}`, box({
      parent: "head",
      at: [x, -0.1, -0.05],
      size: [0.18, 0.38, 0.4],
      material: "coat_light",
    }));
  }
  part("muzzle", box({
    parent: "head",
    at: [0, -0.15, -0.42],
    size: [0.5, 0.26, 0.28],
    material: "cream",
    faces: { north: { texture: "snout_face" } },
  }));
  for (const [side, x] of [["l", -0.29], ["r", 0.29]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.35, 0],
      rot: [0, 0, side === "l" ? -5 : 5],
      size: [0.18, 0.2, 0.13],
      material: "rosette",
      joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.36, -0.49, false],
    ["fr", 0.36, -0.49, false],
    ["bl", -0.38, 0.49, true],
    ["br", 0.38, 0.49, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.56, z],
      size: [rear ? 0.25 : 0.23, rear ? 0.6 : 0.58, rear ? 0.27 : 0.25],
      material: rear ? "coat_shadow" : "coat",
      faces: {
        north: { texture: "leg_rosettes" },
        south: { texture: "leg_rosettes" },
      },
      joint: { pivot: [0, rear ? 0.3 : 0.29, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.36 : -0.35, -0.065],
      size: [rear ? 0.34 : 0.32, 0.12, rear ? 0.4 : 0.38],
      material: "paw",
    }));
  }

  part("tail_1", box({
    parent: "hips",
    at: [0, 0.28, 0.32],
    rot: [52, 0, 0],
    size: [0.13, 0.76, 0.13],
    material: "coat",
    faces: {
      north: { texture: "tail_rosettes" },
      south: { texture: "tail_rosettes" },
      east: { texture: "tail_rosettes" },
      west: { texture: "tail_rosettes" },
    },
    joint: { pivot: [0, -0.37, 0], axis: [0, 0, 1] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, 0.63, 0.04],
    rot: [18, 0, 0],
    size: [0.12, 0.5, 0.12],
    material: "coat_light",
    faces: {
      north: { texture: "tail_rosettes" },
      south: { texture: "tail_rosettes" },
      east: { texture: "tail_rosettes" },
      west: { texture: "tail_rosettes" },
    },
  }));
  part("tail_tip", box({
    parent: "tail_2",
    at: [0, 0.32, 0],
    size: [0.15, 0.16, 0.15],
    material: "rosette",
  }));

  quadrupedWalk("prowl", {
    fps: 18,
    duration: 1.08,
    cycleDistance: 0.88,
    gait: "walk",
    loop: true,
    samples: 17,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.68,
    swingDegrees: 20,
    tail: "tail_1",
    tailSwingDegrees: 10,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.13 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.11 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.11 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "z", axis: "z", degrees: 15, overshoot: 0.8, lag: 0.17 }),
    ],
  });
});
