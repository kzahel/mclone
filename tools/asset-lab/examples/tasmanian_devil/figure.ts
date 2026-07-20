import { figure } from "../../src/dsl";

// A box-only Tasmanian devil with a black barrel, white chest band, oversized
// jaw, red inner ears, short strong legs, and a thick tapered tail. A quick
// grounded lope contrasts with the heavier wombat walk.
export default figure("tasmanian_devil", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#242322");
  mat("fur_light", "#3a3835");
  mat("fur_dark", "#151514");
  mat("white", "#e3ded2");
  mat("muzzle", "#514942");
  mat("nose", "#0f0f0e");
  mat("mouth", "#5e2d30");
  mat("ear", "#7d4545");
  mat("ear_inner", "#b76868");
  mat("paw", "#191817");
  mat("claw", "#d8caa9");

  asciiTexture("face", {
    palette: { ".": "#242322", "l": "#3a3835", "e": "#d0a458", "d": "#151514" },
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
    palette: { ".": "#514942", "n": "#0f0f0e", "m": "#5e2d30" },
    pixels: [
      "........",
      ".nn..nn.",
      "..nnnn..",
      ".mmmmmm.",
      "mmmmmmmm",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#242322", "l": "#3a3835", "d": "#151514", "w": "#e3ded2" },
    pixels: [
      "dddddddddddd",
      "d..........d",
      "..llllllll..",
      "............",
      "wwww........",
      "wwwww.......",
    ],
  });
  asciiTexture("claw_front", {
    palette: { ".": "#191817", "c": "#d8caa9" },
    pixels: [
      "........",
      ".c.c.c..",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 0.75, 0.08],
    size: [0.82, 0.58, 1.28],
    material: "fur",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("chest_band", box({
    parent: "body",
    at: [0, -0.17, -0.7],
    size: [0.62, 0.28, 0.12],
    material: "white",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.08, -0.43],
    size: [0.86, 0.58, 0.46],
    material: "fur_light",
  }));
  for (const [side, x] of [["l", -0.46], ["r", 0.46]] as const) {
    part(`shoulder_mark_${side}`, box({
      parent: "shoulders",
      at: [x, -0.11, 0.03],
      size: [0.06, 0.22, 0.3],
      material: "white",
    }));
  }
  part("rump", box({
    parent: "body",
    at: [0, -0.01, 0.48],
    size: [0.8, 0.54, 0.5],
    material: "fur_dark",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.1, -0.82],
    size: [0.72, 0.6, 0.58],
    material: "fur",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.25], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.17, -0.42],
    size: [0.56, 0.3, 0.34],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("lower_jaw", box({
    parent: "muzzle",
    at: [0, -0.21, 0.01],
    size: [0.5, 0.13, 0.3],
    material: "mouth",
  }));
  for (const [side, x] of [["l", -0.29], ["r", 0.29]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.36, 0.05],
      rot: [0, 0, side === "l" ? -8 : 8],
      size: [0.2, 0.28, 0.12],
      material: "ear",
      joint: { pivot: [0, -0.1, 0], axis: [1, 0, 0] },
    }));
    part(`ear_inner_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0, -0.075],
      size: [0.11, 0.17, 0.035],
      material: "ear_inner",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.29, -0.38],
    ["fr", 0.29, -0.38],
    ["bl", -0.29, 0.4],
    ["br", 0.29, 0.4],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.46, z],
      size: [0.19, 0.44, 0.21],
      material: "fur_dark",
      joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.26, -0.07],
      size: [0.28, 0.12, 0.34],
      material: "paw",
      faces: { north: { texture: "claw_front" } },
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, -0.02, 0.39],
    size: [0.3, 0.26, 0.54],
    material: "fur_dark",
    joint: { pivot: [0, 0, -0.25], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, -0.03, 0.42],
    size: [0.19, 0.17, 0.4],
    material: "fur",
  }));

  quadrupedWalk("lope", {
    fps: 20,
    duration: 0.86,
    cycleDistance: 0.68,
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
    bodyBob: 0.014,
    bodyBobCenter: 0.016,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.6,
    swingDegrees: 18,
    tail: "tail",
    tailSwingDegrees: 8,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.11 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.11 }),
    ],
  });
});
