import { figure } from "../../src/dsl";

// A box-only American bison with a high layered shoulder hump, compact rear,
// low shaggy head, short horns, beard, and heavy planted forelegs.
export default figure("american_bison", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#5a3420");
  mat("fur_light", "#75482c");
  mat("fur_dark", "#2d2019");
  mat("fur_shadow", "#42291c");
  mat("muzzle", "#4a4038");
  mat("horn", "#d2c4a2");
  mat("horn_tip", "#665a49");
  mat("hoof", "#201a17");
  mat("eye", "#b88a48");

  asciiTexture("face", {
    palette: { ".": "#5a3420", "d": "#2d2019", "e": "#b88a48", "s": "#42291c" },
    pixels: [
      "dddddddd",
      "dd....dd",
      "d.e..e.d",
      "d.d..d.d",
      "ssssssss",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#4a4038", "n": "#1b1714", "l": "#665a49" },
    pixels: [
      "..llll..",
      ".llllll.",
      ".nn..nn.",
      "..nnnn..",
      "........",
      "........",
    ],
  });
  asciiTexture("shag_side", {
    palette: { ".": "#5a3420", "d": "#2d2019", "s": "#42291c", "l": "#75482c" },
    pixels: [
      "dddddddddddddd",
      "ddssssssssssdd",
      "dss........ssd",
      "ss..llllllllss",
      "....llllll....",
      "dddd......dddd",
    ],
  });
  asciiTexture("hoof_face", {
    palette: { "h": "#201a17", "c": "#0d0b0a" },
    pixels: [
      "hhhhhh",
      "hhcchh",
      "cccccc",
    ],
  });

  part("body", box({
    at: [0, 1.12, 0.1],
    size: [1.18, 0.82, 1.58],
    material: "fur",
    faces: {
      east: { texture: "shag_side" },
      west: { texture: "shag_side" },
    },
  }));
  part("rear", box({
    parent: "body",
    at: [0, -0.08, 0.56],
    size: [1.02, 0.72, 0.68],
    material: "fur_light",
  }));
  part("shoulder_hump", box({
    parent: "body",
    at: [0, 0.38, -0.46],
    rot: [-5, 0, 0],
    size: [1.28, 0.58, 0.78],
    material: "fur_dark",
  }));
  part("hump_crown", box({
    parent: "shoulder_hump",
    at: [0, 0.34, 0.03],
    size: [1.12, 0.2, 0.58],
    material: "fur_shadow",
  }));
  part("chest_shag", box({
    parent: "body",
    at: [0, -0.2, -0.76],
    size: [1.08, 0.66, 0.26],
    material: "fur_dark",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.44, 0.09],
    size: [0.9, 0.12, 1.12],
    material: "fur_shadow",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.02, -0.87],
    rot: [13, 0, 0],
    size: [0.78, 0.62, 0.52],
    material: "fur_dark",
    joint: { pivot: [0, 0, 0.22], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.02, -0.46],
    rot: [2, 0, 0],
    size: [0.86, 0.62, 0.58],
    material: "fur",
    faces: { north: { texture: "face" } },
  }));
  part("forelock", box({
    parent: "head",
    at: [0, 0.34, -0.03],
    size: [0.72, 0.2, 0.48],
    material: "fur_dark",
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.18, -0.4],
    size: [0.58, 0.3, 0.28],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("beard", box({
    parent: "muzzle",
    at: [0, -0.25, 0.02],
    rot: [8, 0, 0],
    size: [0.42, 0.32, 0.22],
    material: "fur_dark",
    joint: { pivot: [0, 0.15, 0], axis: [1, 0, 0] },
  }));
  for (const [side, x] of [["l", -0.43], ["r", 0.43]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.14, 0.04],
      rot: [0, 0, side === "l" ? -18 : 18],
      size: [0.25, 0.13, 0.12],
      material: "fur_light",
      joint: { pivot: [side === "l" ? 0.1 : -0.1, 0, 0], axis: [1, 0, 0] },
    }));
    part(`horn_${side}`, box({
      parent: "head",
      at: [side === "l" ? -0.35 : 0.35, 0.25, -0.02],
      rot: [-4, 0, side === "l" ? 66 : -66],
      size: [0.12, 0.26, 0.12],
      material: "horn",
    }));
    part(`horn_tip_${side}`, box({
      parent: `horn_${side}`,
      at: [0, 0.18, 0],
      rot: [0, 0, side === "l" ? 18 : -18],
      size: [0.075, 0.16, 0.075],
      material: "horn_tip",
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.39, -0.49, false],
    ["fr", 0.39, -0.49, false],
    ["bl", -0.37, 0.5, true],
    ["br", 0.37, 0.5, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.64, z],
      size: [rear ? 0.28 : 0.33, rear ? 0.56 : 0.64, rear ? 0.31 : 0.36],
      material: rear ? "fur_shadow" : "fur_dark",
      joint: { pivot: [0, rear ? 0.28 : 0.32, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.34 : -0.38, -0.06],
      size: [rear ? 0.33 : 0.38, 0.14, rear ? 0.38 : 0.43],
      material: "hoof",
      faces: { north: { texture: "hoof_face" } },
    }));
  }

  part("tail", box({
    parent: "rear",
    at: [0, 0.08, 0.39],
    rot: [-5, 0, 0],
    size: [0.1, 0.5, 0.1],
    material: "fur_shadow",
    joint: { pivot: [0, 0.24, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.31, 0.02],
    size: [0.2, 0.18, 0.18],
    material: "fur_dark",
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.3,
    cycleDistance: 0.72,
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
    bodyBob: 0.013,
    bodyBobCenter: 0.015,
    head: "head",
    headSwingDegrees: 1.8,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.75,
    swingDegrees: 12,
    tail: "tail",
    tailSwingDegrees: 5,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.35, lag: 0.15 }),
      followThrough("beard", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.65, lag: 0.18 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.6, lag: 0.18 }),
    ],
  });
});
