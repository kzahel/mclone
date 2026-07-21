import { figure } from "../../src/dsl";

// A box-only rock pigeon with a stocky slate body, iridescent neck, pale cere,
// barred broad wings, and a three-feather dark-banded tail.
export default figure("pigeon", ({
  mat,
  asciiTexture,
  part,
  box,
  wingFlap,
  followThrough,
}) => {
  mat("slate", "#6d7680");
  mat("slate_light", "#858d94");
  mat("slate_dark", "#444b54");
  mat("charcoal", "#292e34");
  mat("neck", "#59636a");
  mat("beak", "#a9998c");
  mat("cere", "#ded5ca");
  mat("foot", "#bd7779");

  asciiTexture("face", {
    palette: {
      ".": "#6d7680",
      "d": "#444b54",
      "e": "#d77835",
      "p": "#17191b",
    },
    pixels: [
      "dddddddd",
      "d.e..e.d",
      "d.p..p.d",
      "........",
      "........",
      "dddddddd",
    ],
  });
  asciiTexture("neck_shimmer", {
    palette: {
      ".": "#59636a",
      "g": "#3f806c",
      "p": "#6d527b",
      "d": "#444b54",
    },
    pixels: [
      "..gg....",
      ".ggpp...",
      "ggppgg..",
      ".ppggpp.",
      "..ggpp..",
      "dddddddd",
    ],
  });
  asciiTexture("wing_bars", {
    palette: {
      ".": "#6d7680",
      "l": "#858d94",
      "d": "#444b54",
      "c": "#292e34",
    },
    pixels: [
      "llllllllll",
      "l........l",
      "..dddddddd",
      "..........",
      "dddddddd..",
      "..c..c..c.",
      "cccccccccc",
    ],
  });
  asciiTexture("tail_band", {
    palette: { ".": "#59636a", "d": "#444b54", "c": "#292e34" },
    pixels: [
      "........",
      "........",
      "dddddddd",
      "cccccccc",
      "cccccccc",
    ],
  });

  part("body", box({
    at: [0, 0.84, 0.06],
    size: [0.72, 0.7, 0.94],
    material: "slate",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.04, -0.49],
    size: [0.58, 0.56, 0.2],
    material: "slate_light",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.26, -0.45],
    size: [0.44, 0.52, 0.4],
    material: "neck",
    faces: {
      east: { texture: "neck_shimmer" },
      west: { texture: "neck_shimmer" },
    },
    joint: { pivot: [0, -0.2, 0.13], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.22, -0.28],
    size: [0.5, 0.46, 0.48],
    material: "slate",
    faces: { north: { texture: "face" } },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.05, -0.34],
    size: [0.21, 0.14, 0.22],
    material: "beak",
  }));
  part("cere", box({
    parent: "beak",
    at: [0, 0.09, 0.02],
    size: [0.19, 0.075, 0.11],
    material: "cere",
  }));

  for (const [side, x] of [["l", -0.59], ["r", 0.59]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [x, 0.1, 0.03],
      size: [0.9, 0.085, 0.74],
      material: "slate_light",
      faces: {
        up: { texture: "wing_bars" },
        down: { texture: "wing_bars" },
      },
      joint: {
        pivot: [side === "l" ? 0.44 : -0.44, 0, -0.1],
        axis: [0, 0, 1],
      },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_${side}`,
      at: [side === "l" ? -0.65 : 0.65, -0.01, 0.07],
      rot: [0, side === "l" ? 7 : -7, 0],
      size: [0.46, 0.065, 0.56],
      material: "slate_dark",
      faces: {
        up: { texture: "wing_bars" },
        down: { texture: "wing_bars" },
      },
    }));
  }

  for (const [name, x, yaw] of [
    ["tail_l", -0.18, 5],
    ["tail_c", 0, 0],
    ["tail_r", 0.18, -5],
  ] as const) {
    part(name, box({
      parent: "body",
      at: [x, 0.01, 0.72],
      rot: [7, yaw, 0],
      size: [0.23, 0.08, name === "tail_c" ? 0.68 : 0.62],
      material: name === "tail_c" ? "slate_dark" : "neck",
      faces: {
        up: { texture: "tail_band" },
        down: { texture: "tail_band" },
      },
      joint: { pivot: [0, 0, -0.3], axis: [1, 0, 0] },
    }));
  }

  for (const [side, x] of [["l", -0.19], ["r", 0.19]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.42, -0.04],
      rot: [-38, 0, 0],
      size: [0.11, 0.26, 0.11],
      material: "foot",
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.12, 0.09],
      size: [0.2, 0.07, 0.27],
      material: "charcoal",
    }));
  }

  wingFlap("fly", {
    fps: 18,
    duration: 1.02,
    cycleDistance: 1.25,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.042,
    degrees: 34,
    frequency: 1,
    leftWing: "wing_l",
    rightWing: "wing_r",
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.11 }),
      followThrough("tail_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.17 }),
      followThrough("tail_c", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.6, lag: 0.18 }),
      followThrough("tail_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.17 }),
    ],
  });
});
