import { figure } from "../../src/dsl";

// A box-only mallard drake with a low gray body, chestnut breast, green head,
// white neck ring, yellow bill, blue wing speculum, and broad orange feet.
export default figure("mallard_duck", ({
  mat,
  asciiTexture,
  part,
  box,
  bipedWalk,
  swing,
  followThrough,
}) => {
  mat("body", "#96958b");
  mat("body_light", "#b6b3a6");
  mat("body_dark", "#5b5b55");
  mat("chest", "#70412c");
  mat("head", "#17604b");
  mat("head_light", "#24765d");
  mat("ring", "#eee9dc");
  mat("bill", "#dfad35");
  mat("bill_tip", "#7b6130");
  mat("orange", "#dd7f28");
  mat("tail", "#252725");

  asciiTexture("face", {
    palette: { ".": "#17604b", "l": "#24765d", "e": "#171411", "w": "#eee9dc" },
    pixels: [
      "ll....ll",
      ".we..ew.",
      ".we..ew.",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("breast", {
    palette: { ".": "#70412c", "d": "#4e2c21", "l": "#8b5438" },
    pixels: [
      "dd......dd",
      "d........d",
      ".llllllll.",
      "llllllllll",
      ".llllllll.",
      "..llllll..",
    ],
  });
  asciiTexture("wing_pattern", {
    palette: { ".": "#96958b", "d": "#5b5b55", "b": "#315ea7", "w": "#eee9dc" },
    pixels: [
      "dddddddddd",
      "d........d",
      "..wwwwww..",
      ".bbbbbbbb.",
      ".bbbbbbbb.",
      "..wwwwww..",
      "..........",
    ],
  });
  asciiTexture("foot_top", {
    palette: { ".": "#dd7f28", "d": "#8e491c" },
    pixels: [
      "........",
      ".d.dd.d.",
      "dddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.68, 0.04],
    size: [0.74, 0.54, 1.02],
    material: "body",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.02, -0.55],
    size: [0.62, 0.48, 0.22],
    material: "chest",
    faces: { north: { texture: "breast" } },
  }));
  part("neck_ring", box({
    parent: "body",
    at: [0, 0.27, -0.43],
    size: [0.5, 0.18, 0.42],
    material: "ring",
    joint: { pivot: [0, -0.07, 0.14], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck_ring",
    at: [0, 0.21, -0.07],
    size: [0.58, 0.46, 0.48],
    material: "head",
    faces: { north: { texture: "face" } },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.09, -0.37],
    size: [0.42, 0.14, 0.28],
    material: "bill",
  }));
  part("beak_tip", box({
    parent: "beak",
    at: [0, 0, -0.17],
    size: [0.28, 0.1, 0.09],
    material: "bill_tip",
  }));

  for (const [side, x, face] of [
    ["l", -0.4, "west"],
    ["r", 0.4, "east"],
  ] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [x, 0.01, 0.04],
      size: [0.09, 0.42, 0.76],
      material: "body_dark",
      faces: { [face]: { texture: "wing_pattern" } },
      joint: { pivot: [side === "l" ? 0.04 : -0.04, 0.14, -0.24], axis: [0, 0, 1] },
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.13, 0.58],
    rot: [38, 0, 0],
    size: [0.42, 0.36, 0.11],
    material: "tail",
    joint: { pivot: [0, -0.17, 0], axis: [1, 0, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0.22, 0.03],
    rot: [9, 0, 0],
    size: [0.28, 0.22, 0.08],
    material: "body_light",
  }));

  for (const [side, x] of [["l", -0.19], ["r", 0.19]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.42, 0.06],
      size: [0.11, 0.28, 0.12],
      material: "orange",
      joint: { pivot: [0, 0.14, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.19, -0.1],
      size: [0.34, 0.1, 0.46],
      material: "orange",
      faces: { up: { texture: "foot_top" } },
    }));
  }

  bipedWalk("waddle", {
    fps: 18,
    duration: 0.88,
    cycleDistance: 0.48,
    loop: true,
    samples: 17,
    body: "body",
    bodyBob: 0.018,
    bodyBobCenter: 0.02,
    head: "head",
    headSwingDegrees: 2.5,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.68,
    swingDegrees: 16,
    tracks: [
      swing("body", { axis: "z", degrees: 3.5, phase: 0.25 }),
      swing("wing_l", { axis: "z", degrees: 4, center: -2, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -4, center: 2, frequency: 2 }),
      followThrough("neck_ring", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.1 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.55, lag: 0.13 }),
    ],
  });
});
