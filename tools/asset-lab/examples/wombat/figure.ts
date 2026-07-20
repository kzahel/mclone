import { figure } from "../../src/dsl";

// A box-only common wombat with a broad squared head, compact brown barrel,
// tiny ears, short powerful legs, wide digging paws, and a vestigial tail.
// Its slow walk emphasizes weight and a low center of gravity.
export default figure("wombat", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#735039");
  mat("fur_light", "#967052");
  mat("fur_dark", "#46352a");
  mat("belly", "#a98968");
  mat("muzzle", "#9a795d");
  mat("nose", "#241d19");
  mat("paw", "#4c3a2e");
  mat("claw", "#d5c29b");

  asciiTexture("face", {
    palette: { ".": "#967052", "d": "#46352a", "e": "#171310", "l": "#b18b68" },
    pixels: [
      "dd....dd",
      ".ee..ee.",
      ".ee..ee.",
      "..llll..",
      ".llllll.",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#9a795d", "n": "#241d19", "l": "#b89470" },
    pixels: [
      "ll....ll",
      ".lnnnnl.",
      "..nnnn..",
      "........",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#735039", "l": "#967052", "d": "#46352a", "b": "#a98968" },
    pixels: [
      "dddddddddddd",
      "dlllllllllld",
      "l..........l",
      "............",
      "..bbbbbbbb..",
      ".bbbbbbbbbb.",
    ],
  });
  asciiTexture("claw_front", {
    palette: { ".": "#4c3a2e", "c": "#d5c29b" },
    pixels: [
      "........",
      ".c.c.c..",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 0.72, 0.08],
    size: [0.98, 0.62, 1.34],
    material: "fur",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.36, -0.02],
    size: [0.74, 0.11, 0.94],
    material: "belly",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.02, 0.53],
    size: [1.0, 0.56, 0.48],
    material: "fur_light",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.05, -0.5],
    size: [1.02, 0.6, 0.42],
    material: "fur_light",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.02, -0.84],
    size: [0.78, 0.62, 0.58],
    material: "fur_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.25], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.16, -0.43],
    size: [0.62, 0.34, 0.34],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.29], ["r", 0.29]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.37, 0.08],
      size: [0.14, 0.16, 0.11],
      material: "fur_dark",
      joint: { pivot: [0, -0.06, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.34, -0.39],
    ["fr", 0.34, -0.39],
    ["bl", -0.34, 0.42],
    ["br", 0.34, 0.42],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.45, z],
      size: [0.22, 0.38, 0.23],
      material: "fur_dark",
      joint: { pivot: [0, 0.19, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.23, -0.07],
      size: [0.32, 0.11, 0.36],
      material: "paw",
      faces: { north: { texture: "claw_front" } },
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.02, 0.31],
    rot: [30, 0, 0],
    size: [0.12, 0.14, 0.12],
    material: "fur_dark",
    joint: { pivot: [0, 0.055, 0], axis: [0, 0, 1] },
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.34,
    cycleDistance: 0.62,
    gait: "walk",
    loop: true,
    samples: 21,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.015,
    bodyBobCenter: 0.017,
    head: "head",
    headSwingDegrees: 1.8,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.74,
    swingDegrees: 13,
    tail: "tail",
    tailSwingDegrees: 3,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.4, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.4, lag: 0.12 }),
    ],
  });
});
