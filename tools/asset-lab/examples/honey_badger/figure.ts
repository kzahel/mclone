import { figure } from "../../src/dsl";

// A box-only adult honey badger with a broad pale mantle, black underbody,
// heavy digging paws, determined trot, and separate alternating digging swipe.
export default figure("honey_badger", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  part,
  quadrupedWalk,
  swing,
}) => {
  mat("black", "#252827");
  mat("black_light", "#3d413e");
  mat("black_dark", "#171918");
  mat("mantle", "#c7c5b8");
  mat("mantle_dark", "#92968e");
  mat("muzzle", "#4b4b44");
  mat("claw", "#d5c7aa");

  asciiTexture("mantle_pattern", {
    palette: { ".": "#c7c5b8", "d": "#92968e", "b": "#252827" },
    pixels: [
      "dddddddddddd",
      "d..........d",
      "............",
      "..dddddddd..",
      "bbbbbbbbbbbb",
      "bbbbbbbbbbbb",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#252827", "l": "#3d413e", "e": "#111211", "m": "#4b4b44" },
    pixels: ["ll....ll", ".le..el.", "..mmmm..", ".mmmmmm.", "........"],
  });
  asciiTexture("paw_top", {
    palette: { ".": "#252827", "c": "#d5c7aa", "d": "#171918" },
    pixels: ["...ccc..", "..ccccc.", "dddddddd", "dd....dd"],
  });

  part("body", box({
    at: [0, 0.66, 0.05],
    size: [0.76, 0.5, 1.18],
    material: "black",
  }));
  part("mantle", box({
    parent: "body",
    at: [0, 0.29, 0.02],
    size: [0.8, 0.18, 1.08],
    material: "mantle",
    faces: { east: { texture: "mantle_pattern" }, west: { texture: "mantle_pattern" } },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.29, 0.02],
    size: [0.58, 0.12, 0.84],
    material: "black_dark",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.03, -0.52],
    size: [0.82, 0.5, 0.34],
    material: "black_light",
  }));
  part("shoulder_cap", box({
    parent: "shoulders",
    at: [0, 0.3, 0],
    size: [0.86, 0.14, 0.32],
    material: "mantle_dark",
  }));
  part("head", box({
    parent: "shoulders",
    at: [0, -0.02, -0.38],
    size: [0.64, 0.42, 0.48],
    material: "black",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.2], axis: [1, 0, 0] },
  }));
  part("head_cap", box({
    parent: "head",
    at: [0, 0.24, 0.02],
    size: [0.66, 0.14, 0.42],
    material: "mantle",
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.34],
    size: [0.42, 0.22, 0.24],
    material: "muzzle",
  }));
  part("nose", box({
    parent: "muzzle",
    at: [0, -0.01, -0.15],
    size: [0.28, 0.15, 0.1],
    material: "black_dark",
  }));
  for (const [side, x] of [["l", -0.24], ["r", 0.24]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.19, 0.07],
      size: [0.12, 0.12, 0.09],
      material: "black_dark",
      joint: { pivot: [0, -0.045, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.24, -0.36],
    ["fr", 0.24, -0.36],
    ["bl", -0.24, 0.38],
    ["br", 0.24, 0.38],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.36, z],
      size: [0.17, 0.32, 0.18],
      material: "black_dark",
      joint: { pivot: [0, 0.15, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.21, -0.07],
      size: [0.25, 0.12, suffix.startsWith("f") ? 0.34 : 0.28],
      material: "black",
      faces: { up: { texture: "paw_top" } },
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.02, 0.72],
    rot: [8, 0, 0],
    size: [0.3, 0.26, 0.46],
    material: "black",
    joint: { pivot: [0, 0, -0.2], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0.01, 0.36],
    rot: [5, 0, 0],
    size: [0.22, 0.18, 0.34],
    material: "mantle_dark",
  }));

  quadrupedWalk("determined_trot", {
    label: "Determined trot",
    fps: 20,
    duration: 0.82,
    cycleDistance: 0.7,
    gait: "trot",
    loop: true,
    samples: 19,
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
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.62,
    swingDegrees: 18,
    tail: "tail",
    tailSwingDegrees: 7,
    tracks: [
      swing("shoulders", { axis: "z", degrees: 2.5, phase: 0.25 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.11 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.11 }),
      followThrough("tail_tip", { source: "tail", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 10, overshoot: 0.65, lag: 0.15 }),
    ],
  });
  clip("digging_swipe", {
    label: "Digging swipe",
    role: "action",
    nextClip: "determined_trot",
    fps: 30,
    loop: false,
    keys: [
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.22, { rot: [14, 0, 0] }],
      ["head", 0.74, { rot: [18, 0, 0] }],
      ["head", 1.12, { rot: [0, 0, 0] }],
      ["leg_fl", 0, { rot: [0, 0, 0] }],
      ["leg_fl", 0.18, { rot: [52, 0, 0] }],
      ["leg_fl", 0.34, { rot: [-8, 0, 0] }],
      ["leg_fl", 0.54, { rot: [48, 0, 0] }],
      ["leg_fl", 0.72, { rot: [-6, 0, 0] }],
      ["leg_fl", 1.12, { rot: [0, 0, 0] }],
      ["leg_fr", 0, { rot: [0, 0, 0] }],
      ["leg_fr", 0.18, { rot: [-6, 0, 0] }],
      ["leg_fr", 0.36, { rot: [50, 0, 0] }],
      ["leg_fr", 0.54, { rot: [-8, 0, 0] }],
      ["leg_fr", 0.74, { rot: [46, 0, 0] }],
      ["leg_fr", 0.94, { rot: [-4, 0, 0] }],
      ["leg_fr", 1.12, { rot: [0, 0, 0] }],
      ["shoulders", 0, { rot: [0, 0, 0] }],
      ["shoulders", 0.34, { rot: [4, 0, -4] }],
      ["shoulders", 0.72, { rot: [4, 0, 4] }],
      ["shoulders", 1.12, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("determined_trot");
});
