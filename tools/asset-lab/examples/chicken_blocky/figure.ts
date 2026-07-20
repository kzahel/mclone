import { figure } from "../../src/dsl";

// A box-only hen using the vanilla chicken's compact cuboid hierarchy.
export default figure("chicken_blocky", ({
  mat,
  asciiTexture,
  part,
  box,
  bipedWalk,
  swing,
  followThrough,
}) => {
  mat("feather", "#f3ede2");
  mat("feather_shade", "#d9cfbd");
  mat("comb", "#cf3a32");
  mat("wattle", "#b8302a");
  mat("beak", "#f2a531");
  mat("leg", "#e8a23a");
  mat("foot", "#d98c25");

  asciiTexture("face", {
    palette: {
      ".": "#f3ede2",
      "e": "#1c1712",
      "r": "#cf3a32",
    },
    pixels: [
      "..rr..",
      "......",
      ".e..e.",
      ".e..e.",
      "......",
      "......",
    ],
  });

  asciiTexture("wing_feathers", {
    palette: {
      ".": "#d9cfbd",
      "f": "#f3ede2",
      "s": "#bfb5a4",
    },
    pixels: [
      "ffffff",
      "f....f",
      "fssss.",
      "f....f",
      ".ssss.",
      "......",
    ],
  });

  asciiTexture("foot_face", {
    palette: {
      ".": "#d98c25",
      "t": "#f2a531",
    },
    pixels: [
      "......",
      "t.tt.t",
    ],
  });

  part("body", box({
    at: [0, 0.75, 0.05],
    size: [0.6, 0.62, 0.72],
    material: "feather",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.06, -0.4],
    size: [0.5, 0.48, 0.22],
    material: "feather_shade",
  }));

  part("head", box({
    parent: "body",
    at: [0, 0.36, -0.39],
    size: [0.4, 0.48, 0.36],
    material: "feather",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("comb", box({
    parent: "head",
    at: [0, 0.31, -0.02],
    size: [0.12, 0.18, 0.28],
    material: "comb",
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.02, -0.28],
    size: [0.28, 0.14, 0.2],
    material: "beak",
  }));
  part("wattle", box({
    parent: "head",
    at: [0, -0.25, -0.19],
    size: [0.14, 0.18, 0.12],
    material: "wattle",
  }));

  part("wing_l", box({
    parent: "body",
    at: [-0.34, 0.02, 0],
    size: [0.08, 0.36, 0.54],
    material: "feather_shade",
    faces: {
      west: { texture: "wing_feathers" },
    },
    joint: { pivot: [0.04, 0.14, -0.2], axis: [0, 0, 1] },
  }));
  part("wing_r", box({
    parent: "body",
    at: [0.34, 0.02, 0],
    size: [0.08, 0.36, 0.54],
    material: "feather_shade",
    faces: {
      east: { texture: "wing_feathers" },
    },
    joint: { pivot: [-0.04, 0.14, -0.2], axis: [0, 0, 1] },
  }));

  part("tail", box({
    parent: "body",
    at: [0, 0.19, 0.45],
    rot: [38, 0, 0],
    size: [0.38, 0.42, 0.09],
    material: "feather",
    joint: { pivot: [0, -0.21, 0], axis: [1, 0, 0] },
  }));
  part("tail_top", box({
    parent: "tail",
    at: [0, 0.19, 0.04],
    rot: [12, 0, 0],
    size: [0.26, 0.3, 0.07],
    material: "feather_shade",
  }));

  part("leg_l", box({
    parent: "body",
    at: [-0.14, -0.48, 0.06],
    size: [0.09, 0.4, 0.1],
    material: "leg",
    joint: { pivot: [0, 0.2, 0], axis: [1, 0, 0] },
  }));
  part("leg_r", box({
    parent: "body",
    at: [0.14, -0.48, 0.06],
    size: [0.09, 0.4, 0.1],
    material: "leg",
    joint: { pivot: [0, 0.2, 0], axis: [1, 0, 0] },
  }));
  part("foot_l", box({
    parent: "leg_l",
    at: [0, -0.22, -0.05],
    size: [0.18, 0.06, 0.28],
    material: "foot",
    faces: {
      north: { texture: "foot_face" },
    },
  }));
  part("foot_r", box({
    parent: "leg_r",
    at: [0, -0.22, -0.05],
    size: [0.18, 0.06, 0.28],
    material: "foot",
    faces: {
      north: { texture: "foot_face" },
    },
  }));

  bipedWalk("walk", {
    fps: 12,
    duration: 0.78,
    cycleDistance: 0.5,
    loop: true,
    samples: 13,
    body: "body",
    bodyBob: 0.02,
    bodyBobCenter: 0.02,
    head: "head",
    headSwingDegrees: 3,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.58,
    swingDegrees: 22,
    tracks: [
      swing("wing_l", { axis: "z", degrees: 7, center: -3, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -7, center: 3, frequency: 2 }),
      swing("head", { axis: "x", degrees: 8, frequency: 2 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.12 }),
    ],
  });
});
