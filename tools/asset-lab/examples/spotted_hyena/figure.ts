import { figure } from "../../src/dsl";

// A box-only spotted hyena with tall heavy shoulders, a lower compact rear,
// rounded ears, blunt dark muzzle, raised dorsal mane, and short brush tail.
export default figure("spotted_hyena", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#b79a5c");
  mat("coat_light", "#d1b979");
  mat("coat_shadow", "#806b43");
  mat("mane", "#3d3426");
  mat("muzzle", "#554a3b");
  mat("nose", "#171613");
  mat("paw", "#302b23");
  mat("ear_inner", "#765448");

  asciiTexture("face", {
    palette: { ".": "#b79a5c", "d": "#3d3426", "e": "#d8b74f", "l": "#d1b979" },
    pixels: [
      "dd....dd",
      "d.e..e.d",
      "..e..e..",
      "...ll...",
      "..llll..",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#554a3b", "n": "#171613", "l": "#806b43" },
    pixels: [
      "..llll..",
      ".llllll.",
      "..nnnn..",
      ".nnnnnn.",
      "........",
    ],
  });
  asciiTexture("spotted_side", {
    palette: { ".": "#b79a5c", "s": "#554731", "d": "#3d3426", "l": "#d1b979" },
    pixels: [
      "dddddddddddddd",
      "d..ss...s...sd",
      "..s...ss..s...",
      ".ss.s...ss.s..",
      "...s..ss...s..",
      "llll....llllll",
    ],
  });
  asciiTexture("leg_spots", {
    palette: { ".": "#b79a5c", "s": "#554731", "d": "#302b23" },
    pixels: [
      ".s..",
      "...s",
      "s...",
      "....",
      ".s..",
      "dddd",
    ],
  });

  part("body", box({
    at: [0, 1, 0.12],
    rot: [-4, 0, 0],
    size: [0.74, 0.58, 1.32],
    material: "coat",
    faces: {
      east: { texture: "spotted_side" },
      west: { texture: "spotted_side" },
    },
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.2, -0.5],
    size: [0.84, 0.72, 0.62],
    material: "coat_shadow",
    faces: {
      east: { texture: "spotted_side" },
      west: { texture: "spotted_side" },
    },
  }));
  part("rump", box({
    parent: "body",
    at: [0, -0.14, 0.53],
    size: [0.68, 0.48, 0.58],
    material: "coat_light",
  }));
  part("mane_front", box({
    parent: "shoulders",
    at: [0, 0.42, -0.05],
    size: [0.28, 0.18, 0.54],
    material: "mane",
  }));
  part("mane_back", box({
    parent: "body",
    at: [0, 0.39, 0.2],
    rot: [-5, 0, 0],
    size: [0.2, 0.15, 0.74],
    material: "mane",
  }));
  part("neck", box({
    parent: "shoulders",
    at: [0, 0.03, -0.48],
    rot: [14, 0, 0],
    size: [0.52, 0.58, 0.48],
    material: "coat_shadow",
    joint: { pivot: [0, 0, 0.21], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.03, -0.41],
    rot: [4, 0, 0],
    size: [0.58, 0.48, 0.5],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.4],
    size: [0.38, 0.26, 0.34],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("jaw", box({
    parent: "muzzle",
    at: [0, -0.17, 0.03],
    size: [0.34, 0.1, 0.3],
    material: "mane",
  }));
  for (const [side, x] of [["l", -0.23], ["r", 0.23]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.3, 0.02],
      rot: [-4, 0, side === "l" ? -10 : 10],
      size: [0.2, 0.25, 0.14],
      material: "ear_inner",
      joint: { pivot: [0, -0.11, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.27, -0.43, false],
    ["fr", 0.27, -0.43, false],
    ["bl", -0.24, 0.48, true],
    ["br", 0.24, 0.48, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, rear ? -0.48 : -0.52, z],
      size: [rear ? 0.16 : 0.19, rear ? 0.46 : 0.58, rear ? 0.17 : 0.2],
      material: rear ? "coat_shadow" : "coat",
      faces: {
        east: { texture: "leg_spots" },
        west: { texture: "leg_spots" },
      },
      joint: { pivot: [0, rear ? 0.23 : 0.29, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.28 : -0.34, -0.05],
      size: [rear ? 0.22 : 0.25, 0.11, rear ? 0.27 : 0.3],
      material: "paw",
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.03, 0.36],
    rot: [-12, 0, 0],
    size: [0.18, 0.48, 0.18],
    material: "coat_shadow",
    joint: { pivot: [0, 0.22, 0], axis: [0, 0, 1] },
  }));
  part("tail_brush", box({
    parent: "tail",
    at: [0, -0.3, 0.02],
    size: [0.28, 0.24, 0.26],
    material: "mane",
  }));

  quadrupedWalk("lope", {
    fps: 18,
    duration: 0.78,
    cycleDistance: 0.9,
    gait: "trot",
    loop: true,
    samples: 17,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.018,
    bodyBobCenter: 0.02,
    head: "neck",
    headSwingDegrees: 3.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.57,
    swingDegrees: 24,
    tail: "tail",
    tailSwingDegrees: 10,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.1 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.7, lag: 0.16 }),
    ],
  });
});
