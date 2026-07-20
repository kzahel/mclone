import { figure } from "../../src/dsl";

// A box-only adult cheetah with a narrow waist, deep chest, raised hips, long
// legs, a small tear-marked head, and a counterbalancing two-stage tail.
export default figure("cheetah", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#d8a23e");
  mat("coat_light", "#e5b957");
  mat("coat_shadow", "#b7792c");
  mat("spot", "#30251b");
  mat("cream", "#eee0b8");
  mat("paw", "#5b4228");
  mat("ear_inner", "#9a5b45");

  asciiTexture("face", {
    palette: { ".": "#d8a23e", "s": "#30251b", "e": "#d6c25d", "c": "#eee0b8" },
    pixels: [
      "s......s",
      "..s..s..",
      "..e..e..",
      "..s..s..",
      "..s..s..",
      "..sccs..",
      ".cccccc.",
      "........",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#eee0b8", "s": "#30251b", "n": "#18130f" },
    pixels: [
      ".s....s.",
      "..ssss..",
      "..nnnn..",
      "...nn...",
      "........",
      "........",
    ],
  });
  asciiTexture("coat_side", {
    palette: { ".": "#d8a23e", "s": "#30251b", "l": "#e5b957", "c": "#eee0b8" },
    pixels: [
      ".s..s...s..s..",
      "...s..s...s...",
      "s...s...s...s.",
      "..s...s...s...",
      "l...s...s...l.",
      "cccccccccccccc",
    ],
  });
  asciiTexture("coat_top", {
    palette: { ".": "#d8a23e", "s": "#30251b", "l": "#e5b957" },
    pixels: [
      ".s..s..s..",
      "...s..s...",
      "s...ll...s",
      "..s....s..",
      ".s..s..s..",
    ],
  });
  asciiTexture("leg_spots", {
    palette: { ".": "#d8a23e", "s": "#30251b", "d": "#b7792c" },
    pixels: [
      ".s..",
      "...s",
      "s...",
      "..s.",
      "....",
      ".s..",
      "dddd",
      "dddd",
    ],
  });
  asciiTexture("tail_spots", {
    palette: { ".": "#d8a23e", "s": "#30251b" },
    pixels: [
      ".s.s",
      "s.s.",
      "....",
      ".s..",
      "ssss",
      "....",
      "ssss",
      "ssss",
    ],
  });

  part("body", box({
    at: [0, 1.12, 0.04],
    size: [0.8, 0.52, 1.64],
    material: "coat",
    faces: {
      east: { texture: "coat_side" },
      west: { texture: "coat_side" },
      up: { texture: "coat_top" },
    },
  }));
  part("deep_chest", box({
    parent: "body",
    at: [0, -0.14, -0.63],
    size: [0.68, 0.48, 0.38],
    material: "coat_light",
  }));
  part("raised_hips", box({
    parent: "body",
    at: [0, 0.12, 0.58],
    size: [0.82, 0.42, 0.48],
    material: "coat_shadow",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.29, -0.02],
    size: [0.54, 0.1, 1.02],
    material: "cream",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.09, -0.87],
    rot: [-10, 0, 0],
    size: [0.46, 0.46, 0.42],
    material: "coat_light",
    joint: { pivot: [0, -0.2, 0.15], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.06, -0.37],
    rot: [6, 0, 0],
    size: [0.54, 0.46, 0.52],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.13, -0.36],
    size: [0.36, 0.21, 0.23],
    material: "cream",
    faces: { north: { texture: "snout_face" } },
  }));
  for (const [side, x] of [["l", -0.21], ["r", 0.21]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.27, 0],
      rot: [0, 0, side === "l" ? -7 : 7],
      size: [0.15, 0.19, 0.11],
      material: "spot",
      joint: { pivot: [0, -0.075, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.27, -0.54, false],
    ["fr", 0.27, -0.54, false],
    ["bl", -0.28, 0.55, true],
    ["br", 0.28, 0.55, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.68, z],
      size: [rear ? 0.16 : 0.14, 0.72, rear ? 0.18 : 0.16],
      material: rear ? "coat_shadow" : "coat",
      faces: {
        north: { texture: "leg_spots" },
        south: { texture: "leg_spots" },
      },
      joint: { pivot: [0, 0.36, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.41, -0.055],
      size: [rear ? 0.21 : 0.19, 0.1, rear ? 0.32 : 0.29],
      material: "paw",
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.3, 0.83],
    rot: [58, 0, 0],
    size: [0.1, 0.82, 0.1],
    material: "coat",
    faces: {
      north: { texture: "tail_spots" },
      south: { texture: "tail_spots" },
      east: { texture: "tail_spots" },
      west: { texture: "tail_spots" },
    },
    joint: { pivot: [0, -0.4, 0], axis: [0, 0, 1] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, 0.69, 0.04],
    rot: [18, 0, 0],
    size: [0.09, 0.58, 0.09],
    material: "coat_light",
    faces: {
      north: { texture: "tail_spots" },
      south: { texture: "tail_spots" },
      east: { texture: "tail_spots" },
      west: { texture: "tail_spots" },
    },
  }));
  part("tail_tip", box({
    parent: "tail_2",
    at: [0, 0.36, 0],
    size: [0.11, 0.17, 0.11],
    material: "spot",
  }));

  quadrupedWalk("sprint", {
    fps: 24,
    duration: 0.72,
    cycleDistance: 1.45,
    gait: "bound",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.026,
    bodyBobCenter: 0.029,
    head: "head",
    headSwingDegrees: 2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.48,
    swingDegrees: 32,
    tail: "tail_1",
    tailSwingDegrees: 13,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.55, lag: 0.08 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.5, lag: 0.08 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.5, lag: 0.08 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "z", axis: "z", degrees: 18, overshoot: 0.85, lag: 0.12 }),
    ],
  });
});
