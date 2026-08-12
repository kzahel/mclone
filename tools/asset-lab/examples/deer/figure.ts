import { figure } from "../../src/dsl";

// A box-only white-tailed buck built on the reviewed long-legged quadruped
// grammar. The narrow body, large ears, white tail, and forked antlers keep it
// distinct from the heavier horse even at review-sheet scale.
export default figure("deer", ({
  mat,
  asciiTexture,
  clip,
  defaultClip,
  part,
  box,
  quadrupedWalk,
  followThrough,
  metadata,
  swing,
}) => {
  metadata({
    bodyPlans: ["quadruped"],
    disposition: "passive",
    groups: ["animal"],
    habitats: ["land"],
    scale: "large",
    themes: ["deer", "forest", "forest-edge", "herbivore", "temperate"],
  });

  mat("coat", "#9a6338");
  mat("coat_light", "#c28a58");
  mat("coat_dark", "#5a3823");
  mat("cream", "#ead7b4");
  mat("hoof", "#241a16");
  mat("antler", "#b99a70");
  mat("ear_inner", "#d59a82");

  asciiTexture("face", {
    palette: { ".": "#9a6338", "e": "#17100d", "c": "#ead7b4", "d": "#5a3823" },
    pixels: [
      "dd....dd",
      ".e....e.",
      "..c..c..",
      ".cccccc.",
      "..cccc..",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#ead7b4", "n": "#211611" },
    pixels: [
      "........",
      ".nn..nn.",
      "..nnnn..",
      "...nn...",
      "........",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#9a6338", "l": "#c28a58", "c": "#ead7b4" },
    pixels: [
      "llllllllllll",
      "l..........l",
      "............",
      "............",
      "..cccccccc..",
      ".cccccccccc.",
    ],
  });

  part("body", box({
    at: [0, 1.18, 0.04],
    size: [0.68, 0.6, 1.48],
    material: "coat",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("chest", box({
    parent: "body",
    at: [0, -0.18, -0.7],
    size: [0.54, 0.42, 0.2],
    material: "coat_light",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.34, 0.02],
    size: [0.52, 0.1, 1.02],
    material: "cream",
  }));
  part("neck", box({
    parent: "body",
    at: [0, 0.35, -0.69],
    rot: [-37, 0, 0],
    size: [0.4, 0.78, 0.4],
    material: "coat_light",
    joint: { pivot: [0, -0.39, 0.08], axis: [1, 0, 0] },
  }));
  part("throat", box({
    parent: "neck",
    at: [0, -0.02, -0.22],
    size: [0.27, 0.54, 0.08],
    material: "cream",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.48, -0.18],
    rot: [29, 0, 0],
    size: [0.48, 0.42, 0.52],
    material: "coat",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.13, -0.4],
    size: [0.3, 0.24, 0.3],
    material: "cream",
    faces: { north: { texture: "muzzle_face" } },
  }));

  for (const [side, x, lean] of [
    ["l", -0.24, -18],
    ["r", 0.24, 18],
  ] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.27, 0.06],
      rot: [-7, 0, lean],
      size: [0.16, 0.3, 0.09],
      material: "ear_inner",
      joint: { pivot: [0, -0.13, 0], axis: [1, 0, 0] },
    }));
  }

  // Three sparse beams per side make a forked buck silhouette without tiny
  // cylindrical tines or a dense ornamental rack.
  for (const [side, x, lean] of [
    ["l", -0.15, -15],
    ["r", 0.15, 15],
  ] as const) {
    part(`antler_root_${side}`, box({
      parent: "head",
      at: [x, 0.38, 0.08],
      rot: [-12, 0, lean],
      size: [0.075, 0.42, 0.075],
      material: "antler",
      joint: { pivot: [0, -0.19, 0], axis: [0, 0, 1] },
    }));
    part(`antler_outer_${side}`, box({
      parent: `antler_root_${side}`,
      at: [0, 0.34, 0],
      rot: [-8, 0, lean * 0.65],
      size: [0.065, 0.34, 0.065],
      material: "antler",
    }));
    part(`antler_tine_${side}`, box({
      parent: `antler_root_${side}`,
      at: [0, 0.16, -0.03],
      rot: [-28, 0, -lean * 1.7],
      size: [0.06, 0.25, 0.06],
      material: "antler",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.23, -0.5],
    ["fr", 0.23, -0.5],
    ["bl", -0.23, 0.51],
    ["br", 0.23, 0.51],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.39, z],
      size: [0.16, 0.4, 0.17],
      material: suffix.startsWith("b") ? "coat_dark" : "coat",
      joint: { pivot: [0, 0.2, 0], axis: [1, 0, 0] },
    }));
    part(`shin_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.38, 0],
      size: [0.125, 0.42, 0.135],
      material: "coat_dark",
      joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] },
    }));
    part(`hoof_${suffix}`, box({
      parent: `shin_${suffix}`,
      at: [0, -0.26, -0.035],
      size: [0.18, 0.12, 0.21],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, 0.17, 0.79],
    rot: [42, 0, 0],
    size: [0.2, 0.38, 0.18],
    material: "coat_dark",
    joint: { pivot: [0, 0.18, 0], axis: [0, 0, 1] },
  }));
  part("tail_flag", box({
    parent: "tail",
    at: [0, -0.23, -0.025],
    size: [0.16, 0.22, 0.14],
    material: "cream",
  }));

  quadrupedWalk("walk", {
    label: "Forest-edge walk",
    role: "locomotion",
    fps: 12,
    duration: 1.08,
    cycleDistance: 0.98,
    gait: "walk",
    loop: true,
    samples: 13,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.011,
    bodyBobCenter: 0.012,
    head: "head",
    headSwingDegrees: 2.4,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.66,
    swingDegrees: 18,
    tail: "tail",
    tailSwingDegrees: 7,
    tracks: [
      swing("shin_fl", { axis: "x", center: 3, degrees: 8, phase: 0.5 }),
      swing("shin_fr", { axis: "x", center: 3, degrees: 8, phase: 0 }),
      swing("shin_bl", { axis: "x", center: -3, degrees: 9, phase: 0 }),
      swing("shin_br", { axis: "x", center: -3, degrees: 9, phase: 0.5 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.12 }),
      followThrough("antler_root_l", { source: "head", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 2.5, overshoot: 0.35, lag: 0.1 }),
      followThrough("antler_root_r", { source: "head", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 2.5, overshoot: 0.35, lag: 0.1 }),
    ],
  });

  quadrupedWalk("flee", {
    label: "Flagged flight",
    role: "locomotion",
    fps: 24,
    duration: 0.62,
    cycleDistance: 1.42,
    gait: "bound",
    loop: true,
    samples: 17,
    contactParts: {
      frontLeft: "hoof_fl",
      frontRight: "hoof_fr",
      backLeft: "hoof_bl",
      backRight: "hoof_br",
    },
    body: "body",
    bodyBob: 0.055,
    bodyBobCenter: 0.06,
    head: "head",
    headSwingDegrees: 3.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.48,
    swingDegrees: 34,
    tail: "tail",
    tailSwingDegrees: 2,
    tracks: [
      swing("shin_fl", { axis: "x", center: 9, degrees: 20, phase: 0.5 }),
      swing("shin_fr", { axis: "x", center: 9, degrees: 20, phase: 0.5 }),
      swing("shin_bl", { axis: "x", center: -9, degrees: 22, phase: 0 }),
      swing("shin_br", { axis: "x", center: -9, degrees: 22, phase: 0 }),
      swing("tail", { axis: "x", center: -62, degrees: 3, phase: 0.5 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.7, lag: 0.08 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 12, overshoot: 0.7, lag: 0.08 }),
    ],
  });

  clip("idle", {
    label: "Relaxed watch",
    role: "idle",
    fps: 20,
    loop: true,
    keys: [
      ["body", 0, { at: [0, 0, 0] }],
      ["body", 1.2, { at: [0, 0.012, 0] }],
      ["body", 2.4, { at: [0, 0, 0] }],
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 1.2, { rot: [2, -2, 0] }],
      ["neck", 2.4, { rot: [0, 0, 0] }],
      ["ear_l", 0, { rot: [0, 0, 0] }],
      ["ear_l", 0.6, { rot: [-8, 0, 6] }],
      ["ear_l", 1.2, { rot: [0, 0, 0] }],
      ["ear_l", 2.4, { rot: [0, 0, 0] }],
      ["ear_r", 0, { rot: [0, 0, 0] }],
      ["ear_r", 1.2, { rot: [0, 0, 0] }],
      ["ear_r", 1.8, { rot: [-7, 0, -6] }],
      ["ear_r", 2.4, { rot: [0, 0, 0] }],
      ["tail", 0, { rot: [0, 0, 0] }],
      ["tail", 1.2, { rot: [0, 0, 5] }],
      ["tail", 2.4, { rot: [0, 0, 0] }],
    ],
  });

  clip("alert", {
    label: "Alert scan",
    role: "idle",
    fps: 20,
    loop: true,
    keys: [
      ["neck", 0, { rot: [-10, -7, 0] }],
      ["neck", 0.8, { rot: [-12, 7, 0] }],
      ["neck", 1.6, { rot: [-10, -7, 0] }],
      ["head", 0, { rot: [-7, -4, 0] }],
      ["head", 0.8, { rot: [-7, 4, 0] }],
      ["head", 1.6, { rot: [-7, -4, 0] }],
      ["ear_l", 0, { rot: [-11, 0, 9] }],
      ["ear_l", 0.8, { rot: [-5, 0, 3] }],
      ["ear_l", 1.6, { rot: [-11, 0, 9] }],
      ["ear_r", 0, { rot: [-5, 0, -3] }],
      ["ear_r", 0.8, { rot: [-11, 0, -9] }],
      ["ear_r", 1.6, { rot: [-5, 0, -3] }],
      ["tail", 0, { rot: [-18, 0, 0] }],
      ["tail", 1.6, { rot: [-18, 0, 0] }],
    ],
  });

  clip("graze", {
    label: "Ground browse",
    role: "idle",
    fps: 20,
    loop: true,
    keys: [
      ["neck", 0, { rot: [-82, 0, 0] }],
      ["neck", 0.8, { rot: [-88, 3, 0] }],
      ["neck", 1.6, { rot: [-82, 0, 0] }],
      ["head", 0, { rot: [78, 0, 0] }],
      ["head", 0.8, { rot: [84, -3, 0] }],
      ["head", 1.6, { rot: [78, 0, 0] }],
      ["leg_fl", 0, { rot: [-7, 0, 0] }],
      ["leg_fl", 1.6, { rot: [-7, 0, 0] }],
      ["leg_fr", 0, { rot: [7, 0, 0] }],
      ["leg_fr", 1.6, { rot: [7, 0, 0] }],
      ["ear_l", 0, { rot: [-5, 0, 5] }],
      ["ear_l", 0.8, { rot: [-10, 0, 9] }],
      ["ear_l", 1.6, { rot: [-5, 0, 5] }],
      ["ear_r", 0, { rot: [-10, 0, -9] }],
      ["ear_r", 0.8, { rot: [-5, 0, -5] }],
      ["ear_r", 1.6, { rot: [-10, 0, -9] }],
    ],
  });

  const appendBeddedPose = (keys: Parameters<typeof clip>[1]["keys"], time: number) => {
    keys.push(
      ["body", time, { at: [0, -0.46, 0.06], rot: [2, 0, 0] }],
      ["neck", time, { rot: [15, 0, 0] }],
      ["head", time, { rot: [-10, 0, 0] }],
      ["leg_fl", time, { rot: [-68, 0, 0] }],
      ["shin_fl", time, { rot: [118, 0, 0] }],
      ["leg_fr", time, { rot: [-65, 0, 0] }],
      ["shin_fr", time, { rot: [112, 0, 0] }],
      ["leg_bl", time, { rot: [67, 0, 0] }],
      ["shin_bl", time, { rot: [-116, 0, 0] }],
      ["leg_br", time, { rot: [63, 0, 0] }],
      ["shin_br", time, { rot: [-110, 0, 0] }],
      ["tail", time, { rot: [12, 0, 0] }],
    );
  };
  const appendStandingPose = (keys: Parameters<typeof clip>[1]["keys"], time: number) => {
    for (const name of ["body", "neck", "head", "leg_fl", "shin_fl", "leg_fr", "shin_fr", "leg_bl", "shin_bl", "leg_br", "shin_br", "tail"]) {
      keys.push([name, time, name === "body" ? { at: [0, 0, 0], rot: [0, 0, 0] } : { rot: [0, 0, 0] }]);
    }
  };

  const lieDownKeys: Parameters<typeof clip>[1]["keys"] = [];
  appendStandingPose(lieDownKeys, 0);
  lieDownKeys.push(
    ["body", 0.45, { at: [0, -0.22, 0.03], rot: [1, 0, 0] }],
    ["body", 0.9, { at: [0, -0.35, 0.05], rot: [2, 0, 0] }],
    ["leg_fl", 0.45, { rot: [-38, 0, 0] }],
    ["shin_fl", 0.45, { rot: [66, 0, 0] }],
    ["leg_fr", 0.45, { rot: [-35, 0, 0] }],
    ["shin_fr", 0.45, { rot: [62, 0, 0] }],
    ["leg_bl", 0.45, { rot: [36, 0, 0] }],
    ["shin_bl", 0.45, { rot: [-64, 0, 0] }],
    ["leg_br", 0.45, { rot: [34, 0, 0] }],
    ["shin_br", 0.45, { rot: [-60, 0, 0] }],
  );
  appendBeddedPose(lieDownKeys, 1.35);
  clip("lie_down", {
    label: "Lie down",
    role: "action",
    nextClip: "bedded_idle",
    fps: 24,
    loop: false,
    keys: lieDownKeys,
  });

  const beddedKeys: Parameters<typeof clip>[1]["keys"] = [];
  appendBeddedPose(beddedKeys, 0);
  appendBeddedPose(beddedKeys, 2.2);
  beddedKeys.push(
    ["head", 1.1, { rot: [-6, 3, 0] }],
    ["ear_l", 0, { rot: [0, 0, 0] }],
    ["ear_l", 1.1, { rot: [-8, 0, 7] }],
    ["ear_l", 2.2, { rot: [0, 0, 0] }],
    ["ear_r", 0, { rot: [0, 0, 0] }],
    ["ear_r", 1.1, { rot: [0, 0, 0] }],
    ["ear_r", 2.2, { rot: [0, 0, 0] }],
  );
  clip("bedded_idle", {
    label: "Bedded rest",
    role: "idle",
    fps: 20,
    loop: true,
    keys: beddedKeys,
  });

  const standUpKeys: Parameters<typeof clip>[1]["keys"] = [];
  appendBeddedPose(standUpKeys, 0);
  standUpKeys.push(
    ["body", 0.5, { at: [0, -0.28, 0.04], rot: [1, 0, 0] }],
    ["leg_fl", 0.5, { rot: [-35, 0, 0] }],
    ["shin_fl", 0.5, { rot: [62, 0, 0] }],
    ["leg_fr", 0.5, { rot: [-38, 0, 0] }],
    ["shin_fr", 0.5, { rot: [66, 0, 0] }],
    ["leg_bl", 0.5, { rot: [34, 0, 0] }],
    ["shin_bl", 0.5, { rot: [-60, 0, 0] }],
    ["leg_br", 0.5, { rot: [36, 0, 0] }],
    ["shin_br", 0.5, { rot: [-64, 0, 0] }],
  );
  appendStandingPose(standUpKeys, 1.2);
  clip("stand_up", {
    label: "Stand up",
    role: "action",
    nextClip: "idle",
    fps: 24,
    loop: false,
    keys: standUpKeys,
  });

  clip("hit", {
    label: "Hit reaction",
    role: "action",
    nextClip: "idle",
    fps: 30,
    loop: false,
    keys: [
      ["body", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["body", 0.12, { at: [0, 0.07, 0.06], rot: [-7, 0, 5] }],
      ["body", 0.28, { at: [0, 0.02, 0.02], rot: [2, 0, -2] }],
      ["body", 0.48, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.12, { rot: [-20, 7, 0] }],
      ["neck", 0.28, { rot: [6, -3, 0] }],
      ["neck", 0.48, { rot: [0, 0, 0] }],
      ["tail", 0, { rot: [0, 0, 0] }],
      ["tail", 0.12, { rot: [-48, 0, 0] }],
      ["tail", 0.48, { rot: [0, 0, 0] }],
    ],
  });

  clip("fall", {
    label: "Terminal fall",
    role: "action",
    fps: 24,
    loop: false,
    keys: [
      ["body", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["body", 0.35, { at: [0, -0.12, 0.08], rot: [4, 0, 22] }],
      ["body", 0.8, { at: [0, -0.56, 0.12], rot: [2, 0, 76] }],
      ["body", 1.2, { at: [0, -0.6, 0.12], rot: [2, 0, 84] }],
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.8, { rot: [24, 0, 0] }],
      ["neck", 1.2, { rot: [30, 0, 0] }],
      ["leg_fl", 0, { rot: [0, 0, 0] }],
      ["leg_fl", 0.8, { rot: [-42, 0, 0] }],
      ["leg_fl", 1.2, { rot: [-48, 0, 0] }],
      ["leg_fr", 0, { rot: [0, 0, 0] }],
      ["leg_fr", 0.8, { rot: [-28, 0, 0] }],
      ["leg_fr", 1.2, { rot: [-34, 0, 0] }],
      ["leg_bl", 0, { rot: [0, 0, 0] }],
      ["leg_bl", 0.8, { rot: [36, 0, 0] }],
      ["leg_bl", 1.2, { rot: [42, 0, 0] }],
      ["leg_br", 0, { rot: [0, 0, 0] }],
      ["leg_br", 0.8, { rot: [24, 0, 0] }],
      ["leg_br", 1.2, { rot: [30, 0, 0] }],
      ["tail", 0, { rot: [0, 0, 0] }],
      ["tail", 0.8, { rot: [-35, 0, 0] }],
      ["tail", 1.2, { rot: [-35, 0, 0] }],
    ],
  });

  defaultClip("idle");
});
