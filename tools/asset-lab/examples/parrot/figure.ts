import { figure } from "../../src/dsl";

// A box-only scarlet macaw whose identity comes from a pale cheek patch,
// stepped hooked beak, saturated wing bands, and a long two-feather tail.
// Two-stage wings retain a broad readable silhouette through the flap cycle.
export default figure("parrot", ({
  mat,
  asciiTexture,
  part,
  box,
  wingFlap,
  followThrough,
}) => {
  mat("scarlet", "#cf352e");
  mat("scarlet_dark", "#8f2627");
  mat("blue", "#2768b5");
  mat("blue_dark", "#19447c");
  mat("yellow", "#efbd38");
  mat("cheek", "#eee4cf");
  mat("beak", "#d8c7a9");
  mat("beak_dark", "#2a2525");
  mat("foot", "#72665d");

  asciiTexture("face", {
    palette: { ".": "#cf352e", "c": "#eee4cf", "e": "#171516", "l": "#d9b348" },
    pixels: [
      "........",
      ".cccccc.",
      ".ce..ec.",
      ".cc..cc.",
      "..c..c..",
      "...ll...",
      "........",
      "........",
    ],
  });
  asciiTexture("wing_bands", {
    palette: { "r": "#cf352e", "y": "#efbd38", "b": "#2768b5", "d": "#19447c" },
    pixels: [
      "rrrrrrrrrr",
      "rrrrrrrrrr",
      "yyyyyyyyyy",
      "yyyyyyyyyy",
      "bbbbbbbbbb",
      "bbdbbdbbbb",
      "bdbbdbbdbb",
      "dddddddddd",
    ],
  });
  asciiTexture("flight_feathers", {
    palette: { "b": "#2768b5", "d": "#19447c", "y": "#efbd38" },
    pixels: [
      "yyyyyyyy",
      "bbbbbbbb",
      "bbdbbdbb",
      "bdbbdbbd",
      "dbbdbbdd",
      "dddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.82, 0.04],
    size: [0.54, 0.7, 0.7],
    material: "scarlet",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.03, -0.39],
    size: [0.42, 0.58, 0.16],
    material: "scarlet_dark",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.39, -0.27],
    size: [0.52, 0.52, 0.46],
    material: "scarlet",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, -0.2, 0.14], axis: [1, 0, 0] },
  }));
  part("beak_upper", box({
    parent: "head",
    at: [0, -0.02, -0.35],
    rot: [12, 0, 0],
    size: [0.28, 0.24, 0.28],
    material: "beak",
  }));
  part("beak_tip", box({
    parent: "beak_upper",
    at: [0, -0.14, -0.12],
    rot: [22, 0, 0],
    size: [0.2, 0.18, 0.16],
    material: "beak_dark",
  }));

  part("wing_l", box({
    parent: "body",
    at: [-0.47, 0.08, 0.02],
    size: [0.72, 0.075, 0.66],
    material: "scarlet",
    faces: {
      up: { texture: "wing_bands" },
      down: { texture: "wing_bands" },
    },
    joint: { pivot: [0.36, 0, -0.1], axis: [0, 0, 1] },
  }));
  part("wing_r", box({
    parent: "body",
    at: [0.47, 0.08, 0.02],
    size: [0.72, 0.075, 0.66],
    material: "scarlet",
    faces: {
      up: { texture: "wing_bands" },
      down: { texture: "wing_bands" },
    },
    joint: { pivot: [-0.36, 0, -0.1], axis: [0, 0, 1] },
  }));
  part("wing_tip_l", box({
    parent: "wing_l",
    at: [-0.52, -0.01, 0.06],
    rot: [0, 8, 0],
    size: [0.4, 0.06, 0.54],
    material: "blue",
    faces: {
      up: { texture: "flight_feathers" },
      down: { texture: "flight_feathers" },
    },
  }));
  part("wing_tip_r", box({
    parent: "wing_r",
    at: [0.52, -0.01, 0.06],
    rot: [0, -8, 0],
    size: [0.4, 0.06, 0.54],
    material: "blue",
    faces: {
      up: { texture: "flight_feathers" },
      down: { texture: "flight_feathers" },
    },
  }));

  part("tail_l", box({
    parent: "body",
    at: [-0.11, -0.02, 0.78],
    rot: [8, 4, 0],
    size: [0.16, 0.08, 1.08],
    material: "blue_dark",
    joint: { pivot: [0, 0, -0.54], axis: [1, 0, 0] },
  }));
  part("tail_r", box({
    parent: "body",
    at: [0.11, -0.02, 0.78],
    rot: [8, -4, 0],
    size: [0.16, 0.08, 1.08],
    material: "scarlet_dark",
    joint: { pivot: [0, 0, -0.54], axis: [1, 0, 0] },
  }));
  part("leg_l", box({
    parent: "body",
    at: [-0.14, -0.39, -0.01],
    rot: [-42, 0, 0],
    size: [0.1, 0.24, 0.1],
    material: "foot",
  }));
  part("leg_r", box({
    parent: "body",
    at: [0.14, -0.39, -0.01],
    rot: [-42, 0, 0],
    size: [0.1, 0.24, 0.1],
    material: "foot",
  }));
  part("foot_l", box({
    parent: "leg_l",
    at: [0, -0.11, 0.08],
    size: [0.18, 0.07, 0.22],
    material: "beak_dark",
  }));
  part("foot_r", box({
    parent: "leg_r",
    at: [0, -0.11, 0.08],
    size: [0.18, 0.07, 0.22],
    material: "beak_dark",
  }));

  wingFlap("fly", {
    fps: 20,
    duration: 0.82,
    cycleDistance: 1.35,
    loop: true,
    samples: 17,
    body: "body",
    bodyBob: 0.04,
    degrees: 38,
    frequency: 1,
    leftWing: "wing_l",
    rightWing: "wing_r",
    tracks: [
      followThrough("head", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.4, lag: 0.1 }),
      followThrough("tail_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.18 }),
      followThrough("tail_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.18 }),
    ],
  });
});
