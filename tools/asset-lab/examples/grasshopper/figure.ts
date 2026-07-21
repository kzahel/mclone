import { figure } from "../../src/dsl";

// A box-only meadow grasshopper with striped abdomen, folded veined wings,
// long antennae, four small walking legs, and folded three-stage spring hind
// legs.
export default figure("grasshopper", ({
  mat,
  asciiTexture,
  part,
  box,
  walkCycle,
  contactSwing,
  bob,
  followThrough,
}) => {
  mat("green", "#648449");
  mat("green_light", "#87a85d");
  mat("green_dark", "#405c37");
  mat("olive", "#6e6e3e");
  mat("brown", "#66513c");
  mat("brown_dark", "#3b332b");
  mat("eye", "#7b512f");

  asciiTexture("face", {
    palette: { ".": "#648449", "e": "#7b512f", "h": "#d4b36c", "d": "#405c37" },
    pixels: [
      "........",
      ".ee..ee.",
      ".eh..he.",
      ".ee..ee.",
      "..dddd..",
      "........",
    ],
  });
  asciiTexture("abdomen_bands", {
    palette: { ".": "#648449", "l": "#87a85d", "d": "#405c37", "o": "#6e6e3e" },
    pixels: [
      "llllllllll",
      "l........l",
      "dddddddddd",
      "..........",
      "oooooooooo",
      "..........",
      "dddddddddd",
    ],
  });
  asciiTexture("wing_veins", {
    palette: { ".": "#6e6e3e", "l": "#a1a76c", "d": "#405c37", "b": "#66513c" },
    pixels: [
      "llllllllll",
      "l........l",
      ".d.d.d.d..",
      "..d.d.d.d.",
      ".d.d.d.d..",
      "bbbbbbbbbb",
    ],
  });

  part("abdomen", box({
    at: [0, 0.72, 0.22],
    size: [0.44, 0.4, 0.78],
    material: "green",
    faces: {
      up: { texture: "abdomen_bands" },
      east: { texture: "abdomen_bands" },
      west: { texture: "abdomen_bands" },
    },
  }));
  part("thorax", box({
    parent: "abdomen",
    at: [0, 0.03, -0.5],
    size: [0.5, 0.42, 0.4],
    material: "green_dark",
  }));
  part("head", box({
    parent: "thorax",
    at: [0, -0.03, -0.36],
    size: [0.46, 0.34, 0.34],
    material: "green",
    faces: { north: { texture: "face" } },
  }));
  part("mouth", box({
    parent: "head",
    at: [0, -0.12, -0.24],
    size: [0.24, 0.12, 0.18],
    material: "brown",
  }));

  for (const [side, x, roll] of [["l", -0.14, 20], ["r", 0.14, -20]] as const) {
    part(`antenna_${side}`, box({
      parent: "head",
      at: [x, 0.22, -0.08],
      rot: [-24, 0, roll],
      size: [0.035, 0.4, 0.035],
      material: "brown_dark",
      joint: { pivot: [0, -0.19, 0], axis: [0, 0, 1] },
    }));
    part(`antenna_${side}_tip`, box({
      parent: `antenna_${side}`,
      at: [0, 0.29, -0.03],
      rot: [-18, 0, side === "l" ? 7 : -7],
      size: [0.028, 0.3, 0.028],
      material: "brown_dark",
    }));
    part(`wing_${side}`, box({
      parent: "abdomen",
      at: [side === "l" ? -0.2 : 0.2, 0.22, 0.06],
      rot: [4, side === "l" ? -4 : 4, 0],
      size: [0.14, 0.06, 0.68],
      material: "olive",
      faces: {
        up: { texture: "wing_veins" },
        east: { texture: "wing_veins" },
        west: { texture: "wing_veins" },
      },
    }));
  }

  for (const [row, z, yaw] of [["front", -0.17, 18], ["mid", 0.04, -8]] as const) {
    for (const [side, x, roll] of [["l", -0.29, 24], ["r", 0.29, -24]] as const) {
      part(`leg_${row}_${side}`, box({
        parent: "thorax",
        at: [x, -0.12, z],
        rot: [0, side === "l" ? -yaw : yaw, roll],
        size: [0.28, 0.05, 0.055],
        material: "brown",
        joint: {
          pivot: [side === "l" ? 0.13 : -0.13, 0, 0],
          axis: [0, 1, 0],
        },
      }));
      part(`leg_${row}_${side}_lower`, box({
        parent: `leg_${row}_${side}`,
        at: [side === "l" ? -0.24 : 0.24, -0.02, 0],
        rot: [0, 0, side === "l" ? 28 : -28],
        size: [0.24, 0.045, 0.05],
        material: "brown_dark",
        joint: {
          pivot: [side === "l" ? 0.11 : -0.11, 0, 0],
          axis: [0, 1, 0],
        },
      }));
    }
  }

  // Each hind leg forms the characteristic grasshopper Z: a thick femur runs
  // back to a raised knee, a slim tibia folds forward and down, and a short
  // tarsus rests flat. The shin pivots from its rear end so it cannot read as
  // a detached prong rising behind the abdomen.
  for (const [side, x, yaw] of [["l", -0.32, -8], ["r", 0.32, 8]] as const) {
    part(`hind_thigh_${side}`, box({
      parent: "abdomen",
      at: [x, -0.04, 0.26],
      rot: [-12, yaw, 0],
      size: [0.18, 0.18, 0.62],
      material: "green_light",
      joint: { pivot: [0, 0, -0.29], axis: [1, 0, 0] },
    }));
    part(`hind_shin_${side}`, box({
      parent: `hind_thigh_${side}`,
      at: [side === "l" ? -0.015 : 0.015, 0, 0.02],
      rot: [-38, side === "l" ? -3 : 3, 0],
      size: [0.075, 0.075, 0.6],
      material: "green_dark",
      joint: { pivot: [0, 0, 0.29], axis: [1, 0, 0] },
    }));
    part(`hind_foot_${side}`, box({
      parent: `hind_shin_${side}`,
      at: [0, 0, -0.43],
      rot: [50, 0, 0],
      size: [0.085, 0.05, 0.28],
      material: "brown_dark",
      joint: { pivot: [0, 0, 0.13], axis: [1, 0, 0] },
    }));
  }

  walkCycle("hop", {
    fps: 24,
    duration: 0.72,
    loop: true,
    samples: 23,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.92,
      direction: [0, 0, -1],
      units: "figure",
    },
    tracks: [
      bob("abdomen", { axis: "y", amount: 0.085, center: 0.085, phase: 0.5 }),
      contactSwing("hind_thigh_l", { axis: "x", degrees: 14, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("hind_thigh_r", { axis: "x", degrees: 14, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("hind_shin_l", { axis: "x", degrees: -18, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("hind_shin_r", { axis: "x", degrees: -18, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_front_l", { axis: "y", degrees: 14, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_front_r", { axis: "y", degrees: 14, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_mid_l", { axis: "y", degrees: 12, phase: 0.75, stanceRatio: 0.5 }),
      contactSwing("leg_mid_r", { axis: "y", degrees: 12, phase: 0.75, stanceRatio: 0.5 }),
      followThrough("antenna_l", { source: "abdomen", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.75, lag: 0.13 }),
      followThrough("antenna_r", { source: "abdomen", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.75, lag: 0.13 }),
    ],
  });
});
