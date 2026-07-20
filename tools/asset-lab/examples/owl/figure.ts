import { figure } from "../../src/dsl";

// A box-only great horned owl with a broad facial disc, compact body, layered
// flight feathers, tucked talons, and small ear tufts. Wing tips are children
// of the animated roots so the two-stage silhouette cannot separate in flight.
export default figure("owl", ({
  mat,
  asciiTexture,
  part,
  box,
  wingFlap,
  followThrough,
}) => {
  mat("feather", "#755039");
  mat("feather_light", "#b58a61");
  mat("feather_dark", "#3b2a22");
  mat("disc", "#d4b98b");
  mat("beak", "#d5a23a");
  mat("talon", "#b88936");

  asciiTexture("face", {
    palette: { ".": "#d4b98b", "r": "#755039", "d": "#3b2a22", "e": "#e8b63c" },
    pixels: [
      "rr....rr",
      "r.dddd.r",
      ".de..ed.",
      ".dd..dd.",
      "..d..d..",
      "...rr...",
      "..rrrr..",
      "........",
    ],
  });
  asciiTexture("breast", {
    palette: { ".": "#b58a61", "d": "#3b2a22", "l": "#d4b98b" },
    pixels: [
      "l.l..l.l",
      ".d....d.",
      "..l..l..",
      ".d....d.",
      "l..ll..l",
      ".d....d.",
      "..l..l..",
      "........",
    ],
  });
  asciiTexture("wing_feathers", {
    palette: { ".": "#755039", "l": "#b58a61", "d": "#3b2a22" },
    pixels: [
      "dddddddddd",
      "dllllllll.",
      "d........d",
      "ddlllllldd",
      "d........d",
      "dddllllldd",
      "d........d",
      "dddddddddd",
    ],
  });
  asciiTexture("wing_tip", {
    palette: { ".": "#755039", "d": "#3b2a22", "l": "#b58a61" },
    pixels: [
      "dddddddd",
      "dlllll.d",
      "d.....dd",
      "ddlll.dd",
      "d....ddd",
      "dddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.92, 0.05],
    size: [0.72, 0.82, 0.78],
    material: "feather",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.04, -0.43],
    size: [0.58, 0.68, 0.16],
    material: "feather_light",
    faces: { north: { texture: "breast" } },
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.43, -0.22],
    size: [0.76, 0.58, 0.52],
    material: "disc",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, -0.22, 0.16], axis: [1, 0, 0] },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.09, -0.34],
    rot: [14, 0, 0],
    size: [0.14, 0.18, 0.18],
    material: "beak",
  }));
  part("ear_tuft_l", box({
    parent: "head",
    at: [-0.27, 0.36, 0.02],
    rot: [0, 0, -14],
    size: [0.12, 0.28, 0.12],
    material: "feather_dark",
  }));
  part("ear_tuft_r", box({
    parent: "head",
    at: [0.27, 0.36, 0.02],
    rot: [0, 0, 14],
    size: [0.12, 0.28, 0.12],
    material: "feather_dark",
  }));

  part("wing_l", box({
    parent: "body",
    at: [-0.55, 0.12, 0.02],
    size: [0.82, 0.09, 0.72],
    material: "feather",
    faces: {
      up: { texture: "wing_feathers" },
      down: { texture: "wing_feathers" },
    },
    joint: { pivot: [0.41, 0, -0.1], axis: [0, 0, 1] },
  }));
  part("wing_r", box({
    parent: "body",
    at: [0.55, 0.12, 0.02],
    size: [0.82, 0.09, 0.72],
    material: "feather",
    faces: {
      up: { texture: "wing_feathers" },
      down: { texture: "wing_feathers" },
    },
    joint: { pivot: [-0.41, 0, -0.1], axis: [0, 0, 1] },
  }));
  part("wing_tip_l", box({
    parent: "wing_l",
    at: [-0.61, -0.01, 0.06],
    rot: [0, 8, 0],
    size: [0.48, 0.075, 0.58],
    material: "feather_dark",
    faces: {
      up: { texture: "wing_tip" },
      down: { texture: "wing_tip" },
    },
  }));
  part("wing_tip_r", box({
    parent: "wing_r",
    at: [0.61, -0.01, 0.06],
    rot: [0, -8, 0],
    size: [0.48, 0.075, 0.58],
    material: "feather_dark",
    faces: {
      up: { texture: "wing_tip" },
      down: { texture: "wing_tip" },
    },
  }));

  part("tail", box({
    parent: "body",
    at: [0, 0.02, 0.55],
    rot: [8, 0, 0],
    size: [0.5, 0.09, 0.5],
    material: "feather_dark",
    joint: { pivot: [0, 0, -0.25], axis: [1, 0, 0] },
  }));
  part("leg_l", box({
    parent: "body",
    at: [-0.18, -0.45, -0.02],
    rot: [-35, 0, 0],
    size: [0.12, 0.26, 0.12],
    material: "talon",
  }));
  part("leg_r", box({
    parent: "body",
    at: [0.18, -0.45, -0.02],
    rot: [-35, 0, 0],
    size: [0.12, 0.26, 0.12],
    material: "talon",
  }));
  part("talon_l", box({
    parent: "leg_l",
    at: [0, -0.12, 0.08],
    size: [0.2, 0.08, 0.24],
    material: "feather_dark",
  }));
  part("talon_r", box({
    parent: "leg_r",
    at: [0, -0.12, 0.08],
    size: [0.2, 0.08, 0.24],
    material: "feather_dark",
  }));

  wingFlap("fly", {
    fps: 18,
    duration: 1.05,
    cycleDistance: 1.25,
    loop: true,
    samples: 17,
    body: "body",
    bodyBob: 0.045,
    degrees: 34,
    frequency: 1,
    leftWing: "wing_l",
    rightWing: "wing_r",
    tracks: [
      followThrough("head", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.35, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.18 }),
    ],
  });
});
