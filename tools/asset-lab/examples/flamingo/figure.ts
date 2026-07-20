import { figure } from "../../src/dsl";

// A box-only greater flamingo with an angled two-stage neck, small pink body,
// long two-stage stilt legs, and a black-tipped bent beak. Its slow biped walk
// keeps each broad foot planted for most of the cycle.
export default figure("flamingo", ({
  mat,
  asciiTexture,
  part,
  box,
  bipedWalk,
  swing,
  followThrough,
}) => {
  mat("pink", "#e78694");
  mat("pink_light", "#f2a8af");
  mat("pink_dark", "#c95f73");
  mat("coral", "#df6f72");
  mat("leg", "#d77d83");
  mat("beak", "#efc7b4");
  mat("beak_dark", "#30292a");
  mat("eye", "#171315");
  mat("foot", "#b95d67");

  asciiTexture("face", {
    palette: { ".": "#f2a8af", "e": "#171315", "d": "#c95f73" },
    pixels: [
      "dd....dd",
      ".ee..ee.",
      "........",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("wing_feathers", {
    palette: { ".": "#e78694", "l": "#f2a8af", "d": "#c95f73", "b": "#30292a" },
    pixels: [
      "llllllllll",
      "l........l",
      "..dddddd..",
      ".d......d.",
      "dddddddddd",
      "bbbbbbbbbb",
    ],
  });
  asciiTexture("foot_top", {
    palette: { ".": "#b95d67", "d": "#30292a" },
    pixels: [
      "........",
      "........",
      ".d.dd.d.",
      "dddddddd",
    ],
  });

  part("body", box({
    at: [0, 1.76, 0.08],
    size: [0.68, 0.62, 1.0],
    material: "pink",
  }));
  part("breast", box({
    parent: "body",
    at: [0, -0.03, -0.55],
    size: [0.54, 0.48, 0.18],
    material: "pink_light",
  }));
  part("wing_l", box({
    parent: "body",
    at: [-0.38, 0.03, 0.06],
    size: [0.09, 0.46, 0.76],
    material: "pink_dark",
    faces: { west: { texture: "wing_feathers" } },
    joint: { pivot: [0.04, 0.15, -0.22], axis: [0, 0, 1] },
  }));
  part("wing_r", box({
    parent: "body",
    at: [0.38, 0.03, 0.06],
    size: [0.09, 0.46, 0.76],
    material: "pink_dark",
    faces: { east: { texture: "wing_feathers" } },
    joint: { pivot: [-0.04, 0.15, -0.22], axis: [0, 0, 1] },
  }));

  part("neck_lower", box({
    parent: "body",
    at: [0, 0.45, -0.4],
    rot: [-24, 0, 0],
    size: [0.26, 0.84, 0.26],
    material: "pink_light",
    joint: { pivot: [0, -0.4, 0.07], axis: [1, 0, 0] },
  }));
  part("neck_upper", box({
    parent: "neck_lower",
    at: [0, 0.57, -0.09],
    rot: [28, 0, 0],
    size: [0.22, 0.68, 0.22],
    material: "pink_light",
  }));
  part("head", box({
    parent: "neck_upper",
    at: [0, 0.43, -0.13],
    rot: [-3, 0, 0],
    size: [0.38, 0.36, 0.46],
    material: "pink_light",
    faces: { north: { texture: "face" } },
  }));
  part("beak_base", box({
    parent: "head",
    at: [0, -0.08, -0.36],
    rot: [10, 0, 0],
    size: [0.34, 0.22, 0.32],
    material: "beak",
  }));
  part("beak_tip", box({
    parent: "beak_base",
    at: [0, -0.1, -0.22],
    rot: [24, 0, 0],
    size: [0.28, 0.2, 0.2],
    material: "beak_dark",
  }));

  for (const [side, x] of [["l", -0.17], ["r", 0.17]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.72, 0.08],
      size: [0.1, 0.82, 0.11],
      material: "leg",
      joint: { pivot: [0, 0.41, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.65, 0.07],
      rot: [4, 0, 0],
      size: [0.085, 0.58, 0.095],
      material: "coral",
    }));
    part(`foot_${side}`, box({
      parent: `shin_${side}`,
      at: [0, -0.33, -0.18],
      size: [0.28, 0.09, 0.62],
      material: "foot",
      faces: { up: { texture: "foot_top" } },
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.08, 0.62],
    rot: [20, 0, 0],
    size: [0.46, 0.34, 0.18],
    material: "pink_light",
    joint: { pivot: [0, -0.15, -0.05], axis: [1, 0, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0.2, 0.04],
    size: [0.34, 0.24, 0.12],
    material: "pink_dark",
  }));

  bipedWalk("stilt_walk", {
    fps: 18,
    duration: 1.54,
    cycleDistance: 0.68,
    loop: true,
    samples: 21,
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.013,
    head: "head",
    headSwingDegrees: 1.6,
    leftLeg: "leg_l",
    rightLeg: "leg_r",
    leftContact: "foot_l",
    rightContact: "foot_r",
    stanceRatio: 0.72,
    swingDegrees: 15,
    tracks: [
      swing("wing_l", { axis: "z", degrees: 3, center: -1, frequency: 2 }),
      swing("wing_r", { axis: "z", degrees: -3, center: 1, frequency: 2 }),
      followThrough("neck_lower", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3.5, overshoot: 0.4, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.45, lag: 0.14 }),
    ],
  });
});
