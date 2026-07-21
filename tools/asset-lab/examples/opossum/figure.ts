import { figure } from "../../src/dsl";

// A box-only Virginia opossum with a gray body, white pointed face, black ears,
// pink feet and nose, and a long six-stage bare prehensile tail carried high.
// The cautious walk gives the tail a delayed sway without breaking its chain.
export default figure("opossum", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("fur", "#85827d");
  mat("fur_light", "#aaa69e");
  mat("fur_dark", "#4c4b49");
  mat("white", "#ddd9cf");
  mat("black", "#242424");
  mat("pink", "#c68886");
  mat("pink_light", "#dda6a0");
  mat("pink_dark", "#9e6464");

  asciiTexture("face", {
    palette: { ".": "#ddd9cf", "g": "#85827d", "e": "#171717", "p": "#c68886" },
    pixels: [
      "gg....gg",
      ".ee..ee.",
      "..pppp..",
      ".pppppp.",
      "........",
      "........",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#ddd9cf", "p": "#c68886", "n": "#6e4243" },
    pixels: [
      "........",
      "..pppp..",
      ".ppnnpp.",
      "..nnnn..",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#85827d", "l": "#aaa69e", "d": "#4c4b49", "w": "#ddd9cf" },
    pixels: [
      "dddddddddddd",
      "dlllllllllld",
      "l..........l",
      "............",
      "..wwwwwwww..",
      "...wwwwww...",
    ],
  });
  asciiTexture("pink_paw", {
    palette: { ".": "#c68886", "l": "#dda6a0", "d": "#9e6464" },
    pixels: [
      "........",
      ".l.ll.l.",
      "dddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.67, 0.06],
    size: [0.7, 0.48, 1.18],
    material: "fur",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.29, -0.02],
    size: [0.52, 0.1, 0.84],
    material: "fur_light",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.03, -0.48],
    size: [0.72, 0.45, 0.38],
    material: "fur_dark",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.08, -0.72],
    size: [0.5, 0.44, 0.46],
    material: "white",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.19], axis: [1, 0, 0] },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.12, -0.34],
    size: [0.32, 0.22, 0.26],
    material: "white",
  }));
  part("snout_tip", box({
    parent: "snout",
    at: [0, -0.03, -0.19],
    size: [0.2, 0.15, 0.14],
    material: "pink",
    faces: { north: { texture: "snout_face" } },
  }));
  for (const [side, x] of [["l", -0.21], ["r", 0.21]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.28, 0.06],
      rot: [0, 0, side === "l" ? -8 : 8],
      size: [0.18, 0.24, 0.11],
      material: "black",
      joint: { pivot: [0, -0.09, 0], axis: [1, 0, 0] },
    }));
    part(`ear_inner_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0, -0.07],
      size: [0.1, 0.14, 0.035],
      material: "pink_dark",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.23, -0.34],
    ["fr", 0.23, -0.34],
    ["bl", -0.24, 0.36],
    ["br", 0.24, 0.36],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.38, z],
      size: [0.13, 0.34, 0.15],
      material: "fur_dark",
      joint: { pivot: [0, 0.17, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.21, -0.05],
      size: [0.22, 0.1, 0.28],
      material: "pink",
      faces: { north: { texture: "pink_paw" } },
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.22, 0.64],
    rot: [48, 0, 0],
    size: [0.19, 0.48, 0.19],
    material: "fur_dark",
    joint: { pivot: [0, -0.23, 0], axis: [0, 0, 1] },
  }));
  for (const [index, material, width, height, tilt] of [
    [2, "pink_dark", 0.17, 0.42, 9],
    [3, "pink", 0.15, 0.4, 8],
    [4, "pink_light", 0.13, 0.37, 3],
    [5, "pink", 0.11, 0.34, -7],
    [6, "pink_light", 0.08, 0.28, -12],
  ] as const) {
    part(`tail_${index}`, box({
      parent: `tail_${index - 1}`,
      at: [0, index === 2 ? 0.35 : 0.31, 0.035],
      rot: [tilt, 0, 0],
      size: [width, height, width],
      material,
    }));
  }

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.14,
    cycleDistance: 0.66,
    gait: "walk",
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
    headSwingDegrees: 2.6,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.68,
    swingDegrees: 17,
    tail: "tail_1",
    tailSwingDegrees: 8,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.5, lag: 0.11 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.5, lag: 0.11 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "z", axis: "z", degrees: 12, overshoot: 0.75, lag: 0.16 }),
    ],
  });
});
