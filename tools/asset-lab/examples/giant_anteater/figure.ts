import { figure } from "../../src/dsl";

// A box-only giant anteater with a long three-stage snout, bold black-and-white
// shoulder stripe, heavy clawed forefeet, and an enormous four-stage plume
// tail. The slow walk lets the tail sweep laterally behind the body.
export default figure("giant_anteater", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  swing,
  followThrough,
}) => {
  mat("fur", "#706b62");
  mat("fur_light", "#999188");
  mat("fur_dark", "#33312f");
  mat("black", "#1e1e1d");
  mat("white", "#e2ddd2");
  mat("muzzle", "#5f5a53");
  mat("nose", "#171615");
  mat("paw", "#37332f");
  mat("claw", "#d6c8a8");

  asciiTexture("face", {
    palette: { ".": "#706b62", "l": "#999188", "e": "#171615", "d": "#33312f" },
    pixels: [
      "dd....dd",
      ".ee..ee.",
      "..llll..",
      "........",
      "........",
    ],
  });
  asciiTexture("snout_tip", {
    palette: { ".": "#5f5a53", "n": "#171615" },
    pixels: [
      "........",
      "..nnnn..",
      ".nnnnnn.",
      "..nnnn..",
    ],
  });
  asciiTexture("shoulder_stripe", {
    palette: { ".": "#706b62", "b": "#1e1e1d", "w": "#e2ddd2", "l": "#999188" },
    pixels: [
      "bbbb........",
      "wbbbb.......",
      "wwbbbb......",
      ".wwbbbb.....",
      "..wwbbbb....",
      "...wwbbbb...",
      "....wwbbbb..",
      "lllllwwbbbbb",
    ],
  });
  asciiTexture("tail_plume", {
    palette: { ".": "#33312f", "l": "#706b62", "b": "#1e1e1d", "w": "#999188" },
    pixels: [
      "bbbbbbbbbbbb",
      "bll..ll..llb",
      "ll..ww..ww.l",
      "l..ww..ww..l",
      "..ww..ww....",
      "bbbbbbbbbbbb",
    ],
  });
  asciiTexture("claw_front", {
    palette: { ".": "#37332f", "c": "#d6c8a8" },
    pixels: [
      "........",
      ".c.c.c..",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 0.83, 0.08],
    size: [0.82, 0.58, 1.42],
    material: "fur",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.34, -0.03],
    size: [0.62, 0.11, 1.02],
    material: "fur_light",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.03, 0.55],
    size: [0.86, 0.54, 0.48],
    material: "fur_dark",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.08, -0.53],
    size: [0.86, 0.62, 0.5],
    material: "fur",
    faces: {
      east: { texture: "shoulder_stripe" },
      west: { texture: "shoulder_stripe" },
    },
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.08, -0.79],
    size: [0.48, 0.46, 0.38],
    material: "fur_dark",
    joint: { pivot: [0, 0, 0.16], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.02, -0.34],
    size: [0.4, 0.36, 0.38],
    material: "fur_light",
    faces: { north: { texture: "face" } },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.08, -0.38],
    size: [0.25, 0.22, 0.5],
    material: "muzzle",
  }));
  part("snout_mid", box({
    parent: "snout",
    at: [0, -0.02, -0.35],
    size: [0.19, 0.17, 0.3],
    material: "muzzle",
  }));
  part("snout_tip", box({
    parent: "snout_mid",
    at: [0, -0.01, -0.21],
    size: [0.14, 0.13, 0.14],
    material: "nose",
    faces: { north: { texture: "snout_tip" } },
  }));
  for (const [side, x] of [["l", -0.17], ["r", 0.17]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.22, 0.05],
      size: [0.12, 0.15, 0.09],
      material: "fur_dark",
      joint: { pivot: [0, -0.055, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z, front] of [
    ["fl", -0.28, -0.43, true],
    ["fr", 0.28, -0.43, true],
    ["bl", -0.29, 0.45, false],
    ["br", 0.29, 0.45, false],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.49, z],
      size: [front ? 0.2 : 0.18, 0.46, front ? 0.22 : 0.2],
      material: front ? "black" : "fur_dark",
      joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.28, -0.07],
      size: [front ? 0.34 : 0.28, 0.12, front ? 0.4 : 0.34],
      material: "paw",
      faces: { north: { texture: "claw_front" } },
    }));
  }

  part("tail_1", box({
    parent: "rump",
    at: [0, 0.14, 0.41],
    rot: [18, 0, 0],
    size: [0.58, 0.52, 0.62],
    material: "fur_dark",
    faces: {
      east: { texture: "tail_plume" },
      west: { texture: "tail_plume" },
    },
    joint: { pivot: [0, 0, -0.28], axis: [0, 1, 0] },
  }));
  for (const [index, width, height, length, material] of [
    [2, 0.68, 0.58, 0.66, "black"],
    [3, 0.6, 0.5, 0.6, "fur_dark"],
    [4, 0.42, 0.34, 0.48, "fur"],
  ] as const) {
    part(`tail_${index}`, box({
      parent: `tail_${index - 1}`,
      at: [0, -0.03, index === 2 ? 0.5 : 0.46],
      rot: [index === 4 ? -8 : 4, 0, 0],
      size: [width, height, length],
      material,
      faces: {
        east: { texture: "tail_plume" },
        west: { texture: "tail_plume" },
      },
      joint: { pivot: [0, 0, -length / 2], axis: [0, 1, 0] },
    }));
  }

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.48,
    cycleDistance: 0.64,
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
    bodyBob: 0.013,
    bodyBobCenter: 0.015,
    head: "neck",
    headSwingDegrees: 1.8,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.74,
    swingDegrees: 13,
    tracks: [
      swing("tail_1", { axis: "y", degrees: 6, phase: 0.5 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 9, overshoot: 0.4, lag: 0.14 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.4, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.4, lag: 0.12 }),
    ],
  });
});
