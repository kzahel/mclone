import { figure } from "../../src/dsl";

// A box-only rabbit using the vanilla cuboid anatomy: compact body, separate
// haunches, long rear feet, short forelegs, upright ears, and a small tail.
export default figure("rabbit", ({
  mat,
  asciiTexture,
  part,
  box,
  walkCycle,
  bob,
  contactSwing,
  followThrough,
}) => {
  mat("fur", "#f1f0ec");
  mat("fur_shadow", "#d9d8d2");
  mat("fur_light", "#fbfbf9");
  mat("ear_inner", "#e7a9b4");
  mat("foot", "#e7e5df");

  asciiTexture("face", {
    palette: {
      ".": "#f1f0ec",
      "e": "#2a2622",
      "n": "#e08a98",
    },
    pixels: [
      "........",
      ".ee..ee.",
      ".ee..ee.",
      "........",
      "........",
      "...nn...",
      "........",
      "........",
    ],
  });

  asciiTexture("muzzle_face", {
    palette: {
      ".": "#fbfbf9",
      "n": "#e08a98",
      "m": "#8a6468",
    },
    pixels: [
      "..nn..",
      "..nn..",
      ".m..m.",
      "......",
    ],
  });

  part("body", box({
    at: [0, 0.68, 0.06],
    rot: [-7, 0, 0],
    size: [0.68, 0.52, 0.88],
    material: "fur",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.12, 0.35],
    size: [0.72, 0.62, 0.5],
    material: "fur_shadow",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.27, -0.06],
    size: [0.5, 0.12, 0.62],
    material: "fur_light",
  }));

  part("head", box({
    parent: "body",
    at: [0, 0.2, -0.6],
    size: [0.48, 0.42, 0.42],
    material: "fur",
    faces: {
      north: { texture: "face" },
    },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.1, -0.28],
    size: [0.34, 0.18, 0.18],
    material: "fur_light",
    faces: {
      north: { texture: "muzzle_face" },
    },
  }));

  part("ear_l", box({
    parent: "head",
    at: [-0.13, 0.52, 0.03],
    rot: [4, 0, -8],
    size: [0.15, 0.68, 0.12],
    material: "fur",
    joint: { pivot: [0, -0.34, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.13, 0.52, 0.03],
    rot: [4, 0, 8],
    size: [0.15, 0.68, 0.12],
    material: "fur",
    joint: { pivot: [0, -0.34, 0], axis: [1, 0, 0] },
  }));
  part("ear_inner_l", box({
    parent: "ear_l",
    at: [0, 0, -0.072],
    size: [0.08, 0.54, 0.025],
    material: "ear_inner",
  }));
  part("ear_inner_r", box({
    parent: "ear_r",
    at: [0, 0, -0.072],
    size: [0.08, 0.54, 0.025],
    material: "ear_inner",
  }));

  part("leg_fl", box({
    parent: "body",
    at: [-0.17, -0.44, -0.27],
    size: [0.14, 0.34, 0.16],
    material: "fur",
    joint: { pivot: [0, 0.17, 0], axis: [1, 0, 0] },
  }));
  part("leg_fr", box({
    parent: "body",
    at: [0.17, -0.44, -0.27],
    size: [0.14, 0.34, 0.16],
    material: "fur",
    joint: { pivot: [0, 0.17, 0], axis: [1, 0, 0] },
  }));
  part("paw_fl", box({
    parent: "leg_fl",
    at: [0, -0.19, -0.04],
    size: [0.18, 0.1, 0.24],
    material: "foot",
  }));
  part("paw_fr", box({
    parent: "leg_fr",
    at: [0, -0.19, -0.04],
    size: [0.18, 0.1, 0.24],
    material: "foot",
  }));

  part("leg_bl", box({
    parent: "body",
    at: [-0.26, -0.28, 0.3],
    rot: [-18, 0, 0],
    size: [0.28, 0.5, 0.42],
    material: "fur_shadow",
    joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
  }));
  part("leg_br", box({
    parent: "body",
    at: [0.26, -0.28, 0.3],
    rot: [-18, 0, 0],
    size: [0.28, 0.5, 0.42],
    material: "fur_shadow",
    joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
  }));
  part("foot_bl", box({
    parent: "leg_bl",
    at: [0, -0.35, -0.15],
    size: [0.24, 0.1, 0.54],
    material: "foot",
  }));
  part("foot_br", box({
    parent: "leg_br",
    at: [0, -0.35, -0.15],
    size: [0.24, 0.1, 0.54],
    material: "foot",
  }));

  part("tail", box({
    parent: "rump",
    at: [0, 0.04, 0.34],
    rot: [-12, 0, 0],
    size: [0.24, 0.24, 0.24],
    material: "fur_light",
  }));

  walkCycle("hop", {
    fps: 24,
    duration: 0.66,
    samples: 21,
    loop: true,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.8,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("body", { axis: "y", amount: 0.06, center: 0.06, phase: 0.5 }),
      contactSwing("leg_fl", { axis: "x", degrees: 24, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_fr", { axis: "x", degrees: 24, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_bl", { axis: "x", degrees: 30, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_br", { axis: "x", degrees: 30, phase: 0.75, stanceRatio: 0.5 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 16, overshoot: 0.85, lag: 0.16 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 16, overshoot: 0.85, lag: 0.16 }),
    ],
  });
});
