import { figure } from "../../src/dsl";

// A box-only adult emperor penguin with an upright dark body, pale belly,
// yellow cheek patches, hanging flippers, short legs, and broad webbed feet.
// Its biped gait adds a lateral body roll to produce a restrained waddle.
export default figure("penguin", ({
  mat,
  asciiTexture,
  part,
  box,
  bipedWalk,
  swing,
  followThrough,
}) => {
  mat("black", "#202426");
  mat("black_light", "#363d40");
  mat("white", "#ece9dc");
  mat("cream", "#e5d6ad");
  mat("yellow", "#e7ae3d");
  mat("orange", "#d9822f");
  mat("beak_dark", "#4c3325");
  mat("eye", "#171412");

  asciiTexture("face", {
    palette: { ".": "#202426", "e": "#ece9dc", "p": "#171412", "y": "#e7ae3d" },
    pixels: [
      "........",
      ".ep..pe.",
      ".ep..pe.",
      "yy....yy",
      "y......y",
      "........",
    ],
  });
  asciiTexture("belly", {
    palette: { ".": "#ece9dc", "c": "#e5d6ad", "d": "#363d40" },
    pixels: [
      "dd......dd",
      "d........d",
      ".cccccccc.",
      "cccccccccc",
      "cccccccccc",
      ".cccccccc.",
      "..cccccc..",
      "...cccc...",
    ],
  });
  asciiTexture("foot_top", {
    palette: { ".": "#d9822f", "d": "#4c3325" },
    pixels: [
      "........",
      "........",
      ".d.dd.d.",
      "dddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.84, 0.03],
    size: [0.68, 1.08, 0.62],
    material: "black",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.08, -0.35],
    size: [0.52, 0.8, 0.14],
    material: "white",
    faces: { north: { texture: "belly" } },
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.6, -0.04],
    size: [0.58, 0.52, 0.54],
    material: "black_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, -0.22, 0.06], axis: [1, 0, 0] },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.08, -0.36],
    size: [0.32, 0.16, 0.2],
    material: "orange",
  }));
  part("beak_tip", box({
    parent: "beak",
    at: [0, 0, -0.13],
    size: [0.22, 0.12, 0.12],
    material: "beak_dark",
  }));

  part("flipper_l", box({
    parent: "body",
    at: [-0.43, -0.02, 0.02],
    rot: [4, 0, -7],
    size: [0.16, 0.72, 0.42],
    material: "black_light",
    joint: { pivot: [0.06, 0.31, -0.08], axis: [0, 0, 1] },
  }));
  part("flipper_r", box({
    parent: "body",
    at: [0.43, -0.02, 0.02],
    rot: [4, 0, 7],
    size: [0.16, 0.72, 0.42],
    material: "black_light",
    joint: { pivot: [-0.06, 0.31, -0.08], axis: [0, 0, 1] },
  }));
  part("tail", box({
    parent: "body",
    at: [0, -0.28, 0.4],
    rot: [-18, 0, 0],
    size: [0.32, 0.3, 0.18],
    material: "black",
    joint: { pivot: [0, 0.13, -0.05], axis: [1, 0, 0] },
  }));

  for (const [side, x] of [["l", -0.18], ["r", 0.18]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.65, 0.02],
      size: [0.13, 0.28, 0.14],
      material: "orange",
      joint: { pivot: [0, 0.14, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.18, -0.1],
      size: [0.34, 0.1, 0.48],
      material: "orange",
      faces: { up: { texture: "foot_top" } },
    }));
  }

  bipedWalk("waddle", {
    fps: 18,
    duration: 1.02,
    cycleDistance: 0.4,
    loop: true,
    samples: 17,
    body: "body",
    bodyBob: 0.018,
    bodyBobCenter: 0.02,
    head: "head",
    headSwingDegrees: 2,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.7,
    swingDegrees: 13,
    tracks: [
      swing("body", { axis: "z", degrees: 4.5, phase: 0.25 }),
      swing("flipper_l", { axis: "z", degrees: 5, center: -4, phase: 0.5 }),
      swing("flipper_r", { axis: "z", degrees: -5, center: 4, phase: 0 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.4, lag: 0.12 }),
    ],
  });
});
