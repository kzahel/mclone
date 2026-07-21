import { figure } from "../../src/dsl";

// A box-only strutting wild turkey tom with layered bronze wings, bare blue-red
// neck, snood and wattle, sturdy legs, and a seven-feather display fan.
export default figure("wild_turkey", ({
  mat,
  asciiTexture,
  part,
  box,
  bipedWalk,
  swing,
  followThrough,
}) => {
  mat("bronze", "#63422b");
  mat("bronze_light", "#8a6036");
  mat("bronze_dark", "#34271f");
  mat("black", "#1e1d1a");
  mat("cream", "#d8bf88");
  mat("neck_blue", "#55758a");
  mat("head_blue", "#718fa0");
  mat("red", "#a73730");
  mat("red_dark", "#6f2523");
  mat("beak", "#c79b55");
  mat("leg", "#a47856");

  asciiTexture("face", {
    palette: { ".": "#718fa0", "e": "#171411", "r": "#a73730", "l": "#9db1b8" },
    pixels: [
      "ll....ll",
      ".e....e.",
      "..r..r..",
      "...rr...",
      "..rrrr..",
      "........",
    ],
  });
  asciiTexture("wing_pattern", {
    palette: { ".": "#63422b", "l": "#8a6036", "d": "#34271f", "c": "#d8bf88" },
    pixels: [
      "cccccccccc",
      "cddddddddc",
      "ddlllllldd",
      "dll....lld",
      "ddlllllldd",
      "dddddddddd",
      "..........",
    ],
  });
  asciiTexture("tail_bands", {
    palette: { "b": "#63422b", "l": "#8a6036", "d": "#34271f", "c": "#d8bf88" },
    pixels: [
      "cccccccc",
      "cddddddc",
      "ddllllll",
      "dllbblld",
      "dllbblld",
      "ddllllld",
      "dddddddd",
      "bbbbbbbb",
      "bbbbbbbb",
      "bbbbbbbb",
    ],
  });
  asciiTexture("foot_front", {
    palette: { ".": "#a47856", "d": "#5a3f30" },
    pixels: [
      "........",
      ".d.dd.d.",
      "dddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.87, 0.05],
    size: [0.88, 0.78, 1.02],
    material: "bronze_dark",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.02, -0.58],
    size: [0.7, 0.66, 0.24],
    material: "bronze",
  }));
  for (const [side, x, face] of [
    ["l", -0.48, "west"],
    ["r", 0.48, "east"],
  ] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [x, 0.01, 0.02],
      size: [0.1, 0.58, 0.78],
      material: "bronze",
      faces: { [face]: { texture: "wing_pattern" } },
      joint: { pivot: [side === "l" ? 0.04 : -0.04, 0.2, -0.26], axis: [0, 0, 1] },
    }));
  }

  part("neck", box({
    parent: "body",
    at: [0, 0.5, -0.38],
    rot: [-5, 0, 0],
    size: [0.3, 0.74, 0.3],
    material: "neck_blue",
    joint: { pivot: [0, -0.35, 0.08], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.45, -0.06],
    rot: [4, 0, 0],
    size: [0.4, 0.38, 0.4],
    material: "head_blue",
    faces: { north: { texture: "face" } },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.05, -0.31],
    size: [0.28, 0.15, 0.22],
    material: "beak",
  }));
  part("snood", box({
    parent: "head",
    at: [0, 0.13, -0.29],
    rot: [20, 0, 0],
    size: [0.1, 0.28, 0.1],
    material: "red",
    joint: { pivot: [0, 0.12, 0], axis: [1, 0, 0] },
  }));
  part("wattle", box({
    parent: "neck",
    at: [0, 0.18, -0.2],
    size: [0.22, 0.38, 0.11],
    material: "red_dark",
    joint: { pivot: [0, 0.18, 0], axis: [1, 0, 0] },
  }));

  part("tail_root", box({
    parent: "body",
    at: [0, 0.12, 0.58],
    size: [0.66, 0.3, 0.2],
    material: "bronze_dark",
    joint: { pivot: [0, -0.13, -0.08], axis: [1, 0, 0] },
  }));
  for (const [index, x, y, depth, height, lean, material] of [
    [1, -0.62, 0.42, 0.09, 0.88, -22, "bronze"],
    [2, -0.43, 0.5, 0.105, 1.04, -15, "bronze_light"],
    [3, -0.22, 0.56, 0.12, 1.16, -8, "bronze"],
    [4, 0, 0.59, 0.135, 1.22, 0, "bronze_light"],
    [5, 0.22, 0.56, 0.12, 1.16, 8, "bronze"],
    [6, 0.43, 0.5, 0.105, 1.04, 15, "bronze_light"],
    [7, 0.62, 0.42, 0.09, 0.88, 22, "bronze"],
  ] as const) {
    part(`tail_feather_${index}`, box({
      parent: "tail_root",
      at: [x, y, depth],
      rot: [0, 0, lean],
      size: [0.23, height, 0.09],
      material,
      faces: {
        north: { texture: "tail_bands" },
        south: { texture: "tail_bands" },
      },
    }));
  }

  for (const [side, x] of [["l", -0.2], ["r", 0.2]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.59, -0.03],
      size: [0.12, 0.42, 0.13],
      material: "leg",
      joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.26, -0.09],
      size: [0.3, 0.1, 0.44],
      material: "leg",
      faces: { north: { texture: "foot_front" } },
    }));
  }

  bipedWalk("strut", {
    fps: 18,
    duration: 1.1,
    cycleDistance: 0.44,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.016,
    bodyBobCenter: 0.018,
    head: "head",
    headSwingDegrees: 3,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.68,
    swingDegrees: 16,
    tracks: [
      swing("body", { axis: "z", degrees: 2.5, phase: 0.25 }),
      swing("wing_l", { axis: "z", degrees: 4, center: -1, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -4, center: 1, frequency: 2 }),
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.11 }),
      followThrough("snood", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.75, lag: 0.15 }),
      followThrough("wattle", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.7, lag: 0.16 }),
      followThrough("tail_root", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.14 }),
    ],
  });
});
