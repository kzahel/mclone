import { figure } from "../../src/dsl";

// A box-only Eurasian red squirrel with deep haunches, short forelegs, tufted
// ears, and a three-stage plume tail nearly as large as the body. The bound
// gait synchronizes each leg pair and adds a clear airborne body arc.
export default figure("red_squirrel", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#b8582f");
  mat("fur_light", "#d7864d");
  mat("fur_dark", "#703623");
  mat("cream", "#ead7b0");
  mat("ear_inner", "#bd7467");
  mat("eye", "#17120f");
  mat("paw", "#503026");

  asciiTexture("face", {
    palette: { ".": "#b8582f", "e": "#17120f", "l": "#d7864d", "c": "#ead7b0" },
    pixels: [
      "ll....ll",
      ".ee..ee.",
      ".ee..ee.",
      "..cccc..",
      ".cc..cc.",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#ead7b0", "n": "#3a241e" },
    pixels: [
      "........",
      "...nn...",
      "..nnnn..",
      "...nn...",
      "........",
    ],
  });
  asciiTexture("tail_mottle", {
    palette: { ".": "#b8582f", "l": "#d7864d", "d": "#703623" },
    pixels: [
      "dddddddd",
      "dll..lld",
      "dl....ld",
      "d..ll..d",
      "dl....ld",
      "dll..lld",
      "dddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.72, 0.02],
    rot: [-5, 0, 0],
    size: [0.58, 0.48, 0.92],
    material: "fur",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.13, 0.35],
    size: [0.7, 0.62, 0.5],
    material: "fur_dark",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.27, -0.08],
    size: [0.42, 0.12, 0.62],
    material: "cream",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.22, -0.57],
    size: [0.48, 0.44, 0.44],
    material: "fur_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, -0.04, 0.19], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.29],
    size: [0.3, 0.18, 0.16],
    material: "cream",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.17], ["r", 0.17]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.34, 0.02],
      rot: [0, 0, side === "l" ? -8 : 8],
      size: [0.13, 0.36, 0.11],
      material: "ear_inner",
      joint: { pivot: [0, -0.16, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.19, -0.28, false],
    ["fr", 0.19, -0.28, false],
    ["bl", -0.25, 0.3, true],
    ["br", 0.25, 0.3, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, rear ? -0.3 : -0.39, z],
      rot: rear ? [-16, 0, 0] : [0, 0, 0],
      size: rear ? [0.28, 0.5, 0.38] : [0.13, 0.34, 0.15],
      material: rear ? "fur_dark" : "fur",
      joint: { pivot: [0, rear ? 0.25 : 0.17, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.35 : -0.2, rear ? -0.13 : -0.04],
      size: rear ? [0.26, 0.1, 0.5] : [0.18, 0.09, 0.23],
      material: "paw",
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.35, 0.35],
    rot: [48, 0, 0],
    size: [0.34, 0.66, 0.36],
    material: "fur_dark",
    faces: {
      east: { texture: "tail_mottle" },
      west: { texture: "tail_mottle" },
    },
    joint: { pivot: [0, -0.31, 0], axis: [0, 0, 1] },
  }));
  part("tail_plume", box({
    parent: "tail",
    at: [0, 0.52, 0.11],
    rot: [5, 0, 0],
    size: [0.5, 0.72, 0.48],
    material: "fur_light",
    faces: {
      east: { texture: "tail_mottle" },
      west: { texture: "tail_mottle" },
    },
  }));
  part("tail_tip", box({
    parent: "tail_plume",
    at: [0, 0.48, 0.03],
    rot: [-12, 0, 0],
    size: [0.38, 0.48, 0.4],
    material: "fur_dark",
    faces: {
      east: { texture: "tail_mottle" },
      west: { texture: "tail_mottle" },
    },
  }));

  quadrupedWalk("bound", {
    fps: 20,
    duration: 0.72,
    cycleDistance: 0.82,
    gait: "bound",
    loop: true,
    samples: 21,
    contactParts: {
      frontLeft: "foot_fl",
      frontRight: "foot_fr",
      backLeft: "foot_bl",
      backRight: "foot_br",
    },
    body: "body",
    bodyBob: 0.045,
    bodyBobCenter: 0.046,
    bodyBobPhase: 0.5,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.48,
    swingDegrees: 25,
    tail: "tail",
    tailSwingDegrees: 10,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 13, overshoot: 0.7, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 13, overshoot: 0.7, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 14, overshoot: 0.85, lag: 0.17 }),
    ],
  });
});
