import { figure } from "../../src/dsl";

// A low, narrow fox following the vanilla broad-head and heavy-tail grammar.
// White cheeks, black stockings, and the tail tip are texture or single boxes.
export default figure("fox", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#d96a22");
  mat("coat_shadow", "#a94316");
  mat("coat_dark", "#5a2414");
  mat("cream", "#f3ead8");
  mat("white", "#fff8e8");
  mat("black", "#171310");

  asciiTexture("face", {
    palette: {
      ".": "#d96a22",
      "d": "#5a2414",
      "e": "#f0c34e",
      "w": "#fff8e8",
    },
    pixels: [
      "dd....dd",
      ".e....e.",
      ".e....e.",
      "..wwww..",
      ".ww..ww.",
      ".wwwwww.",
      "..wwww..",
      "........",
    ],
  });
  asciiTexture("snout_face", {
    palette: { ".": "#fff8e8", "n": "#12100e", "s": "#d96a22" },
    pixels: [
      "........",
      "..ssss..",
      "..ssss..",
      "...nn...",
      "..nnnn..",
      "...nn...",
      "........",
      "........",
    ],
  });
  asciiTexture("leg_stocking", {
    palette: { ".": "#d96a22", "b": "#171310" },
    pixels: [
      "....",
      "....",
      "....",
      "bbbb",
      "bbbb",
      "bbbb",
    ],
  });

  part("body", box({
    at: [0, 0.7, 0],
    size: [0.58, 0.4, 1.18],
    material: "coat",
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.02, -0.53],
    size: [0.44, 0.36, 0.24],
    material: "white",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.21, -0.04],
    size: [0.4, 0.08, 0.66],
    material: "cream",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.13, -0.67],
    rot: [-17, 0, 0],
    size: [0.32, 0.3, 0.3],
    material: "coat",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.13, -0.31],
    rot: [8, 0, 0],
    size: [0.46, 0.36, 0.4],
    material: "coat",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.17], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.07, -0.33],
    size: [0.27, 0.17, 0.28],
    material: "white",
    faces: { north: { texture: "snout_face" } },
  }));
  part("ear_l", box({
    parent: "head",
    at: [-0.17, 0.28, -0.01],
    rot: [-5, 0, -11],
    size: [0.14, 0.34, 0.11],
    material: "coat_dark",
    joint: { pivot: [0, -0.15, 0], axis: [1, 0, 0] },
  }));
  part("ear_r", box({
    parent: "head",
    at: [0.17, 0.28, -0.01],
    rot: [-5, 0, 11],
    size: [0.14, 0.34, 0.11],
    material: "coat_dark",
    joint: { pivot: [0, -0.15, 0], axis: [1, 0, 0] },
  }));

  for (const [suffix, x, z, shadow] of [
    ["fl", -0.19, -0.34, false],
    ["fr", 0.19, -0.34, false],
    ["bl", -0.21, 0.4, true],
    ["br", 0.21, 0.4, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.35, z],
      size: [0.11, 0.42, 0.12],
      material: shadow ? "coat_shadow" : "coat",
      faces: {
        north: { texture: "leg_stocking" },
        south: { texture: "leg_stocking" },
        east: { texture: "leg_stocking" },
        west: { texture: "leg_stocking" },
      },
      joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.26, -0.04],
      size: [0.15, 0.08, 0.2],
      material: "black",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.27, 0.6],
    rot: [49, 0, 0],
    size: [0.25, 0.72, 0.28],
    material: "coat_shadow",
    joint: { pivot: [0, -0.36, 0], axis: [0, 0, 1] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0.47, 0.11],
    rot: [14, 0, 0],
    size: [0.28, 0.34, 0.31],
    material: "white",
  }));

  quadrupedWalk("walk", {
    fps: 12,
    duration: 0.82,
    cycleDistance: 0.86,
    gait: "trot",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.01,
    bodyBobCenter: 0.012,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.56,
    swingDegrees: 24,
    tail: "tail",
    tailSwingDegrees: 16,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.1 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.8, lag: 0.18 }),
    ],
  });
});
