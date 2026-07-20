import { figure } from "../../src/dsl";

// A box-only gemsbok oryx with a pale gray-tan body, black flank and leg
// markings, bold facial mask, and twin long straight two-stage horns.
export default figure("gemsbok_oryx", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("coat", "#b9aa8e");
  mat("coat_light", "#d8cdb4");
  mat("coat_shadow", "#8d7c64");
  mat("black", "#25221e");
  mat("white", "#eee7d8");
  mat("muzzle", "#c1af90");
  mat("horn", "#50483b");
  mat("horn_tip", "#292621");
  mat("hoof", "#201c19");
  mat("ear_inner", "#9b786b");

  asciiTexture("face_mask", {
    palette: { ".": "#eee7d8", "b": "#25221e", "e": "#d6b850", "t": "#b9aa8e" },
    pixels: [
      "tt....tt",
      "bb....bb",
      "b.e..e.b",
      "b.b..b.b",
      ".bb..bb.",
      "..bbbb..",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#c1af90", "b": "#25221e", "n": "#171411" },
    pixels: [
      "..bbbb..",
      ".b....b.",
      ".nn..nn.",
      "..nnnn..",
      "........",
      "........",
    ],
  });
  asciiTexture("flank_stripe", {
    palette: { ".": "#b9aa8e", "b": "#25221e", "l": "#d8cdb4", "w": "#eee7d8" },
    pixels: [
      "llllllllllllll",
      "l............l",
      "..............",
      "bbbbbbbbbbbbbb",
      "w............w",
      "wwwwwwwwwwwwww",
    ],
  });
  asciiTexture("leg_stocking", {
    palette: { ".": "#b9aa8e", "b": "#25221e", "w": "#eee7d8" },
    pixels: [
      "....",
      "....",
      "w..w",
      "bbbb",
      "bbbb",
      "b..b",
      "bbbb",
      "bbbb",
    ],
  });

  part("body", box({
    at: [0, 1.22, 0.04],
    size: [0.72, 0.58, 1.5],
    material: "coat",
    faces: {
      east: { texture: "flank_stripe" },
      west: { texture: "flank_stripe" },
    },
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.14, -0.67],
    size: [0.6, 0.46, 0.28],
    material: "coat_light",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.32, 0],
    size: [0.54, 0.1, 1.02],
    material: "white",
  }));
  part("rump_patch", box({
    parent: "body",
    at: [0, 0.01, 0.71],
    size: [0.66, 0.48, 0.16],
    material: "white",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.34, -0.69],
    rot: [-29, 0, 0],
    size: [0.42, 0.76, 0.4],
    material: "coat_shadow",
    joint: { pivot: [0, -0.37, 0.08], axis: [1, 0, 0] },
  }));
  part("throat", box({
    parent: "neck",
    at: [0, -0.01, -0.23],
    size: [0.26, 0.52, 0.09],
    material: "black",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.45, -0.18],
    rot: [24, 0, 0],
    size: [0.42, 0.46, 0.56],
    material: "white",
    faces: { north: { texture: "face_mask" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.14, -0.41],
    size: [0.32, 0.24, 0.3],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x, lean] of [
    ["l", -0.27, -22],
    ["r", 0.27, 22],
  ] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.24, 0.04],
      rot: [-5, 0, lean],
      size: [0.22, 0.16, 0.1],
      material: "ear_inner",
      joint: { pivot: [side === "l" ? 0.09 : -0.09, 0, 0], axis: [1, 0, 0] },
    }));
    part(`horn_${side}_1`, box({
      parent: "head",
      at: [side === "l" ? -0.13 : 0.13, 0.34, 0.09],
      rot: [24, 0, side === "l" ? -2 : 2],
      size: [0.075, 0.72, 0.075],
      material: "horn",
      joint: { pivot: [0, -0.34, 0], axis: [1, 0, 0] },
    }));
    part(`horn_${side}_2`, box({
      parent: `horn_${side}_1`,
      at: [0, 0.62, 0],
      rot: [-2, 0, 0],
      size: [0.055, 0.55, 0.055],
      material: "horn_tip",
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.24, -0.5, false],
    ["fr", 0.24, -0.5, false],
    ["bl", -0.25, 0.51, true],
    ["br", 0.25, 0.51, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.65, z],
      size: [0.14, 0.74, 0.15],
      material: rear ? "coat_shadow" : "coat",
      faces: {
        north: { texture: "leg_stocking" },
        south: { texture: "leg_stocking" },
        east: { texture: "leg_stocking" },
        west: { texture: "leg_stocking" },
      },
      joint: { pivot: [0, 0.37, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.43, -0.03],
      size: [0.18, 0.11, 0.23],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.05, 0.81],
    rot: [-4, 0, 0],
    size: [0.09, 0.6, 0.09],
    material: "black",
    joint: { pivot: [0, 0.29, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.38, 0.02],
    size: [0.18, 0.2, 0.17],
    material: "black",
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 0.96,
    cycleDistance: 1.08,
    gait: "walk",
    loop: true,
    samples: 17,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.011,
    bodyBobCenter: 0.013,
    head: "head",
    headSwingDegrees: 2.4,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.63,
    swingDegrees: 21,
    tail: "tail",
    tailSwingDegrees: 9,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.5, lag: 0.12 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.6, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 9, overshoot: 0.6, lag: 0.1 }),
      followThrough("horn_l_1", { source: "head", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 2, overshoot: 0.3, lag: 0.08 }),
      followThrough("horn_r_1", { source: "head", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 2, overshoot: 0.3, lag: 0.08 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.65, lag: 0.17 }),
    ],
  });
});
