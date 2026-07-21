import { figure } from "../../src/dsl";

// A box-only big brown bat with an upright flight body, large stepped ears,
// broad three-stage membrane wings, tucked hind feet, and a tail membrane.
// Child wing stages inherit the fast animated root flap without separation.
export default figure("bat", ({
  mat,
  asciiTexture,
  part,
  box,
  wingFlap,
  followThrough,
}) => {
  mat("fur", "#4b342b");
  mat("fur_light", "#775246");
  mat("fur_dark", "#281e1a");
  mat("membrane", "#3b3030");
  mat("membrane_light", "#5d4544");
  mat("ear", "#6d4747");
  mat("ear_inner", "#9b6663");
  mat("muzzle", "#6a5145");
  mat("nose", "#181412");
  mat("claw", "#b9a58a");

  asciiTexture("face", {
    palette: { ".": "#4b342b", "l": "#775246", "e": "#d3a64b", "d": "#281e1a" },
    pixels: [
      "dd....dd",
      ".ee..ee.",
      ".ee..ee.",
      "..llll..",
      ".llllll.",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#6a5145", "n": "#181412" },
    pixels: [
      "........",
      ".nn..nn.",
      "..nnnn..",
      "...nn...",
    ],
  });
  asciiTexture("membrane_ribs", {
    palette: { ".": "#3b3030", "l": "#5d4544", "d": "#281e1a" },
    pixels: [
      "dddddddddddd",
      "d.l..l..l..d",
      "d..l..l..l.d",
      "d...l..l...d",
      "d....l.....d",
      "dddddddddddd",
    ],
  });
  asciiTexture("belly_fur", {
    palette: { ".": "#775246", "d": "#281e1a", "l": "#9b7664" },
    pixels: [
      "dd....dd",
      "dlllllld",
      ".llllll.",
      "..llll..",
      "...ll...",
      "........",
    ],
  });

  part("body", box({
    at: [0, 0.92, 0.05],
    size: [0.44, 0.64, 0.48],
    material: "fur",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.03, -0.29],
    size: [0.34, 0.5, 0.11],
    material: "fur_light",
    faces: { north: { texture: "belly_fur" } },
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.39, -0.12],
    size: [0.46, 0.42, 0.42],
    material: "fur",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, -0.17, 0.12], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.29],
    size: [0.3, 0.19, 0.18],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.18], ["r", 0.18]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.32, 0.04],
      rot: [0, 0, side === "l" ? -8 : 8],
      size: [0.18, 0.38, 0.11],
      material: "ear",
      joint: { pivot: [0, -0.15, 0], axis: [1, 0, 0] },
    }));
    part(`ear_inner_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0, -0.07],
      size: [0.09, 0.24, 0.035],
      material: "ear_inner",
    }));
  }

  for (const [side, direction] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({
      parent: "body",
      at: [direction * 0.58, 0.15, 0.03],
      size: [0.82, 0.06, 0.64],
      material: "membrane",
      faces: {
        up: { texture: "membrane_ribs" },
        down: { texture: "membrane_ribs" },
      },
      joint: { pivot: [-direction * 0.4, 0, -0.08], axis: [0, 0, 1] },
    }));
    part(`wing_mid_${side}`, box({
      parent: `wing_${side}`,
      at: [direction * 0.61, -0.01, 0.08],
      rot: [0, direction * -7, 0],
      size: [0.58, 0.05, 0.54],
      material: "membrane_light",
      faces: {
        up: { texture: "membrane_ribs" },
        down: { texture: "membrane_ribs" },
      },
    }));
    part(`wing_tip_${side}`, box({
      parent: `wing_mid_${side}`,
      at: [direction * 0.43, -0.01, 0.05],
      rot: [0, direction * -9, 0],
      size: [0.36, 0.04, 0.4],
      material: "fur_dark",
      faces: {
        up: { texture: "membrane_ribs" },
        down: { texture: "membrane_ribs" },
      },
    }));
  }

  for (const [side, x] of [["l", -0.15], ["r", 0.15]] as const) {
    part(`leg_${side}`, box({
      parent: "body",
      at: [x, -0.38, 0.03],
      rot: [-24, 0, side === "l" ? -8 : 8],
      size: [0.11, 0.3, 0.11],
      material: "fur_dark",
    }));
    part(`foot_${side}`, box({
      parent: `leg_${side}`,
      at: [0, -0.18, 0.08],
      size: [0.18, 0.08, 0.24],
      material: "claw",
    }));
  }
  part("tail_membrane", box({
    parent: "body",
    at: [0, -0.28, 0.35],
    rot: [8, 0, 0],
    size: [0.48, 0.055, 0.5],
    material: "membrane",
    faces: {
      up: { texture: "membrane_ribs" },
      down: { texture: "membrane_ribs" },
    },
    joint: { pivot: [0, 0, -0.23], axis: [1, 0, 0] },
  }));

  wingFlap("fly", {
    fps: 24,
    duration: 0.72,
    cycleDistance: 1.18,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.038,
    degrees: 42,
    frequency: 1,
    leftWing: "wing_l",
    rightWing: "wing_r",
    tracks: [
      followThrough("head", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.35, lag: 0.1 }),
      followThrough("tail_membrane", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.5, lag: 0.15 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.45, lag: 0.12 }),
    ],
  });
});
