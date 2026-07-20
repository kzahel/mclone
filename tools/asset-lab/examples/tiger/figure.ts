import { figure } from "../../src/dsl";

// A box-only tiger built from the sparse Minecraft feline grammar. Stripes and
// facial detail live primarily in pixel textures instead of extra geometry.
export default figure("tiger", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  swing,
  followThrough,
}) => {
  mat("coat", "#e8751a");
  mat("coat_light", "#f09a36");
  mat("coat_shadow", "#b84d13");
  mat("stripe", "#15110e");
  mat("white", "#fff3dc");
  mat("ear_inner", "#d37a5f");
  mat("paw", "#3a2215");

  asciiTexture("face", {
    palette: {
      ".": "#e8751a",
      "s": "#15110e",
      "e": "#f0c44d",
      "w": "#fff3dc",
    },
    pixels: [
      "ss....ss",
      "s.s..s.s",
      ".se..es.",
      "..s..s..",
      ".wwssww.",
      ".wwwwww.",
      "..wwww..",
      "........",
    ],
  });

  asciiTexture("snout_face", {
    palette: {
      ".": "#fff3dc",
      "s": "#15110e",
      "n": "#16100d",
      "c": "#f4d8a7",
    },
    pixels: [
      "..cccc..",
      ".cccccc.",
      ".s....s.",
      "..nnnn..",
      "...nn...",
      "........",
      "........",
      "........",
    ],
  });

  asciiTexture("body_side", {
    palette: {
      ".": "#e8751a",
      "o": "#f09a36",
      "s": "#15110e",
      "w": "#fff3dc",
    },
    pixels: [
      "..s..s..s..s",
      ".ss..s..ss..",
      ".s...ss..s..",
      "oo...oo...oo",
      "oww..ww..wwo",
      "wwwwwwwwwwww",
    ],
  });

  asciiTexture("body_top", {
    palette: {
      ".": "#e8751a",
      "s": "#15110e",
      "o": "#f09a36",
    },
    pixels: [
      "s..s..s..s",
      ".s..ss..s.",
      "..oooooo..",
      ".s..ss..s.",
      "s..s..s..s",
    ],
  });

  asciiTexture("leg_stripes", {
    palette: {
      ".": "#e8751a",
      "s": "#15110e",
      "o": "#b84d13",
    },
    pixels: [
      "s..s",
      ".ss.",
      "....",
      "s..s",
      ".ss.",
      "....",
      "oooo",
      "oooo",
    ],
  });

  asciiTexture("tail_rings", {
    palette: {
      ".": "#e8751a",
      "s": "#15110e",
    },
    pixels: [
      "ssss",
      "ssss",
      "....",
      "....",
      "ssss",
      "ssss",
      "....",
      "ssss",
    ],
  });

  asciiTexture("paw_face", {
    palette: {
      "p": "#3a2215",
      "c": "#f7ead4",
    },
    pixels: [
      "pppppp",
      "pppppp",
      "pcpccp",
    ],
  });

  part("body", box({
    at: [0, 1, 0.04],
    size: [1.08, 0.66, 1.72],
    material: "coat",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
      up: { texture: "body_top" },
    },
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.2, -0.77],
    size: [0.72, 0.44, 0.24],
    material: "white",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.35, -0.02],
    size: [0.7, 0.1, 1.16],
    material: "white",
  }));

  // A broad low head, projecting muzzle, and tiny square ears carry the feline
  // silhouette without the rounded version's neck, cheek, and brow clusters.
  part("head", box({
    parent: "body",
    at: [0, 0.06, -1.04],
    size: [0.74, 0.58, 0.62],
    material: "coat_light",
    faces: {
      north: { texture: "face" },
    },
    joint: { pivot: [0, 0, 0.26], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.13, -0.42],
    size: [0.5, 0.25, 0.26],
    material: "white",
    faces: {
      north: { texture: "snout_face" },
    },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.25, 0.35, -0.02],
    rot: [0, 0, -5],
    size: [0.18, 0.2, 0.13],
    material: "stripe",
    joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.25, 0.35, -0.02],
    rot: [0, 0, 5],
    size: [0.18, 0.2, 0.13],
    material: "stripe",
    joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] },
  }));
  part("ear_inner_l", box({
    parent: "ear_l",
    at: [0, -0.01, -0.08],
    size: [0.1, 0.11, 0.035],
    material: "ear_inner",
  }));
  part("ear_inner_r", box({
    parent: "ear_r",
    at: [0, -0.01, -0.08],
    size: [0.1, 0.11, 0.035],
    material: "ear_inner",
  }));

  for (const [suffix, x, z, shadow] of [
    ["fl", -0.36, -0.53, false],
    ["fr", 0.36, -0.53, false],
    ["bl", -0.37, 0.52, true],
    ["br", 0.37, 0.52, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.58, z],
      size: [0.22, 0.64, 0.23],
      material: shadow ? "coat_shadow" : "coat",
      faces: {
        north: { texture: "leg_stripes" },
        south: { texture: "leg_stripes" },
        east: { texture: "leg_stripes" },
        west: { texture: "leg_stripes" },
      },
      joint: { pivot: [0, 0.32, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.36, -0.055],
      size: [0.31, 0.12, 0.35],
      material: "paw",
      faces: {
        north: { texture: "paw_face" },
      },
    }));
  }

  // Two long cuboids echo the vanilla ocelot tail rig. Pixel rings carry the
  // tiger marking while the second segment supplies an independent trailing arc.
  part("tail_1", box({
    parent: "body",
    // The bottom pivot is embedded just inside the body's rear face, so the
    // raised tail has no daylight between its root and the rump.
    at: [0, 0.48, 0.84],
    rot: [55, 0, 0],
    size: [0.13, 0.76, 0.13],
    material: "coat",
    faces: {
      north: { texture: "tail_rings" },
      south: { texture: "tail_rings" },
      east: { texture: "tail_rings" },
      west: { texture: "tail_rings" },
    },
    joint: { pivot: [0, -0.38, 0], axis: [0, 0, 1] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    // Attach the second segment's bottom pivot to tail_1's top endpoint.
    at: [0, 0.67, 0],
    rot: [25, 0, 0],
    size: [0.12, 0.58, 0.12],
    material: "coat",
    faces: {
      north: { texture: "tail_rings" },
      south: { texture: "tail_rings" },
      east: { texture: "tail_rings" },
      west: { texture: "tail_rings" },
    },
    joint: { pivot: [0, -0.29, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", box({
    parent: "tail_2",
    at: [0, 0.38, 0],
    size: [0.16, 0.18, 0.16],
    material: "stripe",
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 1.02,
    cycleDistance: 1.02,
    gait: "walk",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.011,
    bodyBobCenter: 0.013,
    head: "head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.66,
    swingDegrees: 19,
    tail: "tail_1",
    tailSwingDegrees: 11,
    tracks: [
      swing("head", { axis: "x", degrees: 1.8, phase: 0.5 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.4, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.4, lag: 0.12 }),
      followThrough("tail_1", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.65, lag: 0.18 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "z", axis: "z", degrees: 15, overshoot: 0.8, lag: 0.18 }),
    ],
  });
});
