import { figure } from "../../src/dsl";

// A box-only meerkat in its signature upright sentry posture, with a narrow
// striped torso, dark eye patches, folded forepaws, planted feet, and long tail.
export default figure("meerkat", ({
  mat,
  asciiTexture,
  part,
  box,
  walkCycle,
  swing,
  bob,
  followThrough,
}) => {
  mat("fur", "#b69362");
  mat("fur_light", "#cfb889");
  mat("fur_dark", "#594737");
  mat("stripe", "#6f5942");
  mat("cream", "#e2d0aa");
  mat("black", "#24211d");
  mat("paw", "#4a3a2e");

  asciiTexture("face", {
    palette: { ".": "#b69362", "m": "#594737", "e": "#171411", "l": "#cfb889" },
    pixels: [
      "ll....ll",
      "mmm..mmm",
      "mme..emm",
      "mmm..mmm",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#cfb889", "n": "#24211d" },
    pixels: [
      "........",
      "..nnnn..",
      ".nnnnnn.",
      "...nn...",
      "........",
    ],
  });
  asciiTexture("back_stripes", {
    palette: { ".": "#b69362", "s": "#6f5942", "l": "#cfb889" },
    pixels: [
      "ssssssss",
      "........",
      "..ssss..",
      "........",
      ".ssssss.",
      "........",
      "..ssss..",
      "llllllll",
    ],
  });

  part("torso", box({
    at: [0, 1.0, 0.06],
    size: [0.46, 0.88, 0.4],
    material: "fur",
    faces: {
      east: { texture: "back_stripes" },
      west: { texture: "back_stripes" },
    },
  }));
  part("belly", box({
    parent: "torso",
    at: [0, -0.03, -0.24],
    size: [0.34, 0.68, 0.1],
    material: "cream",
  }));
  part("pelvis", box({
    parent: "torso",
    at: [0, -0.42, 0.12],
    size: [0.56, 0.42, 0.5],
    material: "fur_dark",
  }));
  part("neck", box({
    parent: "torso",
    at: [0, 0.49, -0.02],
    size: [0.34, 0.3, 0.32],
    material: "fur_light",
    joint: { pivot: [0, -0.13, 0], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.28, -0.08],
    size: [0.44, 0.42, 0.44],
    material: "fur",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.3],
    size: [0.28, 0.19, 0.18],
    material: "fur_light",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.19], ["r", 0.19]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.2, 0.02],
      rot: [0, 0, side === "l" ? -8 : 8],
      size: [0.14, 0.16, 0.1],
      material: "fur_dark",
      joint: { pivot: [0, -0.06, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [side, x, lean] of [
    ["l", -0.26, 23],
    ["r", 0.26, -23],
  ] as const) {
    part(`arm_${side}`, box({
      parent: "torso",
      at: [x, 0.09, -0.08],
      rot: [-17, 0, lean],
      size: [0.14, 0.48, 0.16],
      material: "fur_light",
      joint: { pivot: [0, 0.23, 0], axis: [1, 0, 0] },
    }));
    part(`hand_${side}`, box({
      parent: `arm_${side}`,
      at: [side === "l" ? 0.07 : -0.07, -0.28, -0.07],
      rot: [15, 0, side === "l" ? 28 : -28],
      size: [0.17, 0.18, 0.22],
      material: "paw",
    }));
  }

  for (const [side, x] of [["l", -0.18], ["r", 0.18]] as const) {
    part(`leg_${side}`, box({
      parent: "pelvis",
      at: [x, -0.35, 0.01],
      size: [0.18, 0.42, 0.2],
      material: "fur_dark",
      joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.25, -0.09],
      size: [0.23, 0.1, 0.36],
      material: "paw",
    }));
  }

  part("tail_1", box({
    parent: "pelvis",
    at: [0, -0.12, 0.31],
    rot: [-55, 0, 0],
    size: [0.18, 0.72, 0.18],
    material: "fur_dark",
    joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, -0.52, 0],
    rot: [-8, 0, 0],
    size: [0.15, 0.52, 0.15],
    material: "fur",
  }));
  part("tail_tip", box({
    parent: "tail_2",
    at: [0, -0.33, 0],
    size: [0.12, 0.2, 0.12],
    material: "black",
  }));

  walkCycle("sentry", {
    fps: 24,
    duration: 1.8,
    loop: true,
    samples: 25,
    tracks: [
      bob("torso", { axis: "y", amount: 0.012, center: 0.012, phase: 0.25 }),
      swing("torso", { axis: "z", degrees: 1.5, phase: 0.25 }),
      swing("head", { axis: "y", degrees: 14, frequency: 0.5 }),
      swing("arm_l", { axis: "x", degrees: 4, center: -17, phase: 0.5 }),
      swing("arm_r", { axis: "x", degrees: 4, center: -17, phase: 0 }),
      followThrough("ear_l", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.12 }),
      followThrough("ear_r", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.12 }),
      followThrough("tail_1", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.55, lag: 0.16 }),
    ],
  });
});
