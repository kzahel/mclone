import { figure } from "../../src/dsl";

// A box-only bald eagle with a long dark body, white head and tail, broad
// two-stage wings, a stepped hooked beak, and tucked yellow talons. Sparse
// feather textures separate it from the smaller owl and bright macaw.
export default figure("eagle", ({
  mat,
  asciiTexture,
  part,
  box,
  wingFlap,
  followThrough,
}) => {
  mat("brown", "#4b3022");
  mat("brown_light", "#785139");
  mat("brown_dark", "#251b17");
  mat("white", "#eeeadd");
  mat("white_shadow", "#d5d0c3");
  mat("yellow", "#e1ad28");
  mat("yellow_dark", "#956b1b");

  asciiTexture("face", {
    palette: { ".": "#eeeadd", "e": "#171513", "s": "#d5d0c3" },
    pixels: [
      "ssssssss",
      "s......s",
      ".ee..ee.",
      "..e..e..",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("wing_coverts", {
    palette: { ".": "#4b3022", "l": "#785139", "d": "#251b17" },
    pixels: [
      "llllllllllll",
      "l..........l",
      "ll..ll..llll",
      "..ll..ll....",
      "ddd....ddddd",
      "d..dddd....d",
      "dd......dddd",
      "dddddddddddd",
    ],
  });
  asciiTexture("flight_feathers", {
    palette: { ".": "#4b3022", "l": "#785139", "d": "#251b17" },
    pixels: [
      "llllllllll",
      "l........l",
      "dd..dd..dd",
      "d.dd..dd.d",
      "dd..dd..dd",
      "d.dd..dd.d",
      "dddddddddd",
    ],
  });
  asciiTexture("tail_feathers", {
    palette: { ".": "#eeeadd", "s": "#d5d0c3" },
    pixels: [
      "ssssssssss",
      "s........s",
      "s.s.ss.s.s",
      ".s.s..s.s.",
      "s.s.ss.s.s",
      "ssssssssss",
    ],
  });

  part("body", box({
    at: [0, 0.92, 0.08],
    size: [0.72, 0.68, 1.02],
    material: "brown",
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.04, -0.56],
    size: [0.58, 0.54, 0.18],
    material: "brown_light",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.18, -0.56],
    size: [0.54, 0.48, 0.36],
    material: "white_shadow",
    joint: { pivot: [0, -0.18, 0.12], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.14, -0.34],
    size: [0.5, 0.46, 0.46],
    material: "white",
    faces: { north: { texture: "face" } },
  }));
  part("beak_upper", box({
    parent: "head",
    at: [0, -0.02, -0.35],
    rot: [12, 0, 0],
    size: [0.27, 0.22, 0.28],
    material: "yellow",
  }));
  part("beak_hook", box({
    parent: "beak_upper",
    at: [0, -0.14, -0.11],
    rot: [25, 0, 0],
    size: [0.18, 0.17, 0.15],
    material: "yellow_dark",
  }));

  part("wing_l", box({
    parent: "body",
    at: [-0.67, 0.12, 0.02],
    size: [1.08, 0.1, 0.82],
    material: "brown",
    faces: {
      up: { texture: "wing_coverts" },
      down: { texture: "wing_coverts" },
    },
    joint: { pivot: [0.54, 0, -0.12], axis: [0, 0, 1] },
  }));
  part("wing_r", box({
    parent: "body",
    at: [0.67, 0.12, 0.02],
    size: [1.08, 0.1, 0.82],
    material: "brown",
    faces: {
      up: { texture: "wing_coverts" },
      down: { texture: "wing_coverts" },
    },
    joint: { pivot: [-0.54, 0, -0.12], axis: [0, 0, 1] },
  }));
  part("wing_tip_l", box({
    parent: "wing_l",
    at: [-0.78, -0.015, 0.08],
    rot: [0, 7, 0],
    size: [0.58, 0.08, 0.66],
    material: "brown_dark",
    faces: {
      up: { texture: "flight_feathers" },
      down: { texture: "flight_feathers" },
    },
  }));
  part("wing_tip_r", box({
    parent: "wing_r",
    at: [0.78, -0.015, 0.08],
    rot: [0, -7, 0],
    size: [0.58, 0.08, 0.66],
    material: "brown_dark",
    faces: {
      up: { texture: "flight_feathers" },
      down: { texture: "flight_feathers" },
    },
  }));

  part("tail", box({
    parent: "body",
    at: [0, 0.02, 0.76],
    rot: [6, 0, 0],
    size: [0.68, 0.09, 0.72],
    material: "white",
    faces: {
      up: { texture: "tail_feathers" },
      down: { texture: "tail_feathers" },
    },
    joint: { pivot: [0, 0, -0.36], axis: [1, 0, 0] },
  }));
  part("leg_l", box({
    parent: "body",
    at: [-0.2, -0.4, -0.08],
    rot: [-38, 0, 0],
    size: [0.13, 0.3, 0.14],
    material: "yellow",
  }));
  part("leg_r", box({
    parent: "body",
    at: [0.2, -0.4, -0.08],
    rot: [-38, 0, 0],
    size: [0.13, 0.3, 0.14],
    material: "yellow",
  }));
  part("talon_l", box({
    parent: "leg_l",
    at: [0, -0.14, 0.1],
    size: [0.24, 0.08, 0.3],
    material: "yellow_dark",
  }));
  part("talon_r", box({
    parent: "leg_r",
    at: [0, -0.14, 0.1],
    size: [0.24, 0.08, 0.3],
    material: "yellow_dark",
  }));

  wingFlap("fly", {
    fps: 18,
    duration: 1.2,
    cycleDistance: 1.65,
    loop: true,
    samples: 17,
    body: "body",
    bodyBob: 0.05,
    degrees: 31,
    frequency: 1,
    leftWing: "wing_l",
    rightWing: "wing_r",
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.35, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.55, lag: 0.18 }),
    ],
  });
});
