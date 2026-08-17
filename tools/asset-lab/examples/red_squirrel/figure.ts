import { figure } from "../../src/dsl";

// A box-only Eurasian red squirrel with deep haunches, short forelegs, tufted
// ears, and a three-stage plume tail nearly as large as the body. The bound
// gait synchronizes each leg pair and adds a clear airborne body arc.
export default figure("red_squirrel", ({
  mat,
  asciiTexture,
  part,
  box,
  clip,
  defaultClip,
  geometryException,
  metadata,
  quadrupedWalk,
  followThrough,
}) => {
  metadata({
    bodyPlans: ["quadruped"],
    disposition: "passive",
    groups: ["animal"],
    habitats: ["land"],
    scale: "small",
    themes: ["squirrel", "woodland", "arboreal", "mast", "cache", "temperate"],
  });
  for (const climbingTailPart of ["tail", "tail_plume", "tail_tip"]) {
    geometryException({
      rule: "ground-penetration",
      parts: [climbingTailPart],
      reason: "The named tail segment trails below the squirrel's feet only during the elevated vertical trunk-climb clip; authoritative support keeps the actor above horizontal terrain.",
    });
  }
  mat("fur", "#b8582f");
  mat("fur_light", "#d7864d");
  mat("fur_dark", "#703623");
  mat("cream", "#ead7b0");
  mat("ear_inner", "#bd7467");
  mat("eye", "#17120f");
  mat("paw", "#503026");

  asciiTexture("face", {
    palette: { ".": "#b8582f", "e": "#17120f", "l": "#d7864d", "c": "#ead7b0" },
    pixels: [
      "ll....ll",
      ".ee..ee.",
      ".ee..ee.",
      "..cccc..",
      ".cc..cc.",
      "........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#ead7b0", "n": "#3a241e" },
    pixels: [
      "........",
      "...nn...",
      "..nnnn..",
      "...nn...",
      "........",
    ],
  });
  asciiTexture("tail_mottle", {
    palette: { ".": "#b8582f", "l": "#d7864d", "d": "#703623" },
    pixels: [
      "dddddddd",
      "dll..lld",
      "dl....ld",
      "d..ll..d",
      "dl....ld",
      "dll..lld",
      "dddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.72, 0.02],
    rot: [-5, 0, 0],
    size: [0.58, 0.48, 0.92],
    material: "fur",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.13, 0.35],
    size: [0.7, 0.62, 0.5],
    material: "fur_dark",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.27, -0.08],
    size: [0.42, 0.12, 0.62],
    material: "cream",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.22, -0.57],
    size: [0.48, 0.44, 0.44],
    material: "fur_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, -0.04, 0.19], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.29],
    size: [0.3, 0.18, 0.16],
    material: "cream",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.17], ["r", 0.17]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.34, 0.02],
      rot: [0, 0, side === "l" ? -8 : 8],
      size: [0.13, 0.36, 0.11],
      material: "ear_inner",
      joint: { pivot: [0, -0.16, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z, rear] of [
    ["fl", -0.19, -0.28, false],
    ["fr", 0.19, -0.28, false],
    ["bl", -0.25, 0.3, true],
    ["br", 0.25, 0.3, true],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, rear ? -0.3 : -0.39, z],
      rot: rear ? [-16, 0, 0] : [0, 0, 0],
      size: rear ? [0.28, 0.5, 0.38] : [0.13, 0.34, 0.15],
      material: rear ? "fur_dark" : "fur",
      joint: { pivot: [0, rear ? 0.25 : 0.17, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, rear ? -0.35 : -0.2, rear ? -0.13 : -0.04],
      size: rear ? [0.26, 0.1, 0.5] : [0.18, 0.09, 0.23],
      material: "paw",
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.35, 0.35],
    rot: [48, 0, 0],
    size: [0.34, 0.66, 0.36],
    material: "fur_dark",
    faces: {
      east: { texture: "tail_mottle" },
      west: { texture: "tail_mottle" },
    },
    joint: { pivot: [0, -0.31, 0], axis: [0, 0, 1] },
  }));
  part("tail_plume", box({
    parent: "tail",
    at: [0, 0.52, 0.11],
    rot: [5, 0, 0],
    size: [0.5, 0.72, 0.48],
    material: "fur_light",
    faces: {
      east: { texture: "tail_mottle" },
      west: { texture: "tail_mottle" },
    },
  }));
  part("tail_tip", box({
    parent: "tail_plume",
    at: [0, 0.48, 0.03],
    rot: [-12, 0, 0],
    size: [0.38, 0.48, 0.4],
    material: "fur_dark",
    faces: {
      east: { texture: "tail_mottle" },
      west: { texture: "tail_mottle" },
    },
  }));

  quadrupedWalk("bound", {
    label: "Woodland bound",
    role: "locomotion",
    fps: 20,
    duration: 0.72,
    cycleDistance: 0.82,
    gait: "bound",
    loop: true,
    samples: 21,
    contactParts: {
      frontLeft: "foot_fl",
      frontRight: "foot_fr",
      backLeft: "foot_bl",
      backRight: "foot_br",
    },
    body: "body",
    bodyBob: 0.045,
    bodyBobCenter: 0.046,
    bodyBobPhase: 0.5,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.48,
    swingDegrees: 25,
    tail: "tail",
    tailSwingDegrees: 10,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 13, overshoot: 0.7, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 13, overshoot: 0.7, lag: 0.12 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 14, overshoot: 0.85, lag: 0.17 }),
    ],
  });

  quadrupedWalk("flee", {
    label: "Cover escape",
    role: "locomotion",
    fps: 30,
    duration: 0.46,
    cycleDistance: 1.14,
    gait: "bound",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "foot_fl",
      frontRight: "foot_fr",
      backLeft: "foot_bl",
      backRight: "foot_br",
    },
    body: "body",
    bodyBob: 0.11,
    bodyBobCenter: 0.1,
    bodyBobPhase: 0.5,
    head: "head",
    headSwingDegrees: 4,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.38,
    swingDegrees: 39,
    tail: "tail",
    tailSwingDegrees: 16,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 20, overshoot: 0.85, lag: 0.08 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 20, overshoot: 0.85, lag: 0.08 }),
      followThrough("tail", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 19, overshoot: 0.9, lag: 0.12 }),
    ],
  });

  clip("idle", {
    label: "Woodland watch",
    role: "idle",
    fps: 20,
    loop: true,
    keys: [
      ["body", 0, { at: [0, 0, 0] }],
      ["body", 1.4, { at: [0, 0.012, 0] }],
      ["body", 2.8, { at: [0, 0, 0] }],
      ["head", 0, { rot: [0, -5, 0] }],
      ["head", 1.4, { rot: [-4, 6, 0] }],
      ["head", 2.8, { rot: [0, -5, 0] }],
      ["ear_l", 0, { rot: [0, 0, 0] }],
      ["ear_l", 0.65, { rot: [-12, 0, 7] }],
      ["ear_l", 1.3, { rot: [0, 0, 0] }],
      ["ear_l", 2.8, { rot: [0, 0, 0] }],
      ["ear_r", 0, { rot: [0, 0, 0] }],
      ["ear_r", 1.7, { rot: [-10, 0, -6] }],
      ["ear_r", 2.3, { rot: [0, 0, 0] }],
      ["ear_r", 2.8, { rot: [0, 0, 0] }],
      ["tail", 0, { rot: [0, 0, -4] }],
      ["tail", 1.4, { rot: [0, 0, 5] }],
      ["tail", 2.8, { rot: [0, 0, -4] }],
    ],
  });

  clip("forage", {
    label: "Mast forage",
    role: "idle",
    fps: 20,
    loop: true,
    keys: [
      ["body", 0, { at: [0, -0.025, 0.02], rot: [8, 0, 0] }],
      ["body", 0.7, { at: [0, -0.04, -0.015], rot: [11, 0, 0] }],
      ["body", 1.4, { at: [0, -0.025, 0.02], rot: [8, 0, 0] }],
      ["head", 0, { rot: [30, -4, 0] }],
      ["head", 0.35, { rot: [38, 4, 0] }],
      ["head", 0.7, { rot: [31, 0, 0] }],
      ["head", 1.05, { rot: [37, -4, 0] }],
      ["head", 1.4, { rot: [30, -4, 0] }],
      ["leg_fl", 0, { rot: [-18, 0, 0] }],
      ["leg_fl", 0.7, { rot: [-25, 0, 0] }],
      ["leg_fl", 1.4, { rot: [-18, 0, 0] }],
      ["leg_fr", 0, { rot: [-25, 0, 0] }],
      ["leg_fr", 0.7, { rot: [-18, 0, 0] }],
      ["leg_fr", 1.4, { rot: [-25, 0, 0] }],
      ["tail", 0, { rot: [0, 0, -6] }],
      ["tail", 0.7, { rot: [0, 0, 6] }],
      ["tail", 1.4, { rot: [0, 0, -6] }],
    ],
  });

  clip("carry", {
    label: "Carry mast",
    role: "idle",
    fps: 20,
    loop: true,
    keys: [
      ["body", 0, { at: [0, 0.025, 0], rot: [-4, 0, 0] }],
      ["body", 1, { at: [0, 0.035, 0], rot: [-3, 0, 0] }],
      ["body", 2, { at: [0, 0.025, 0], rot: [-4, 0, 0] }],
      ["head", 0, { rot: [-10, 0, 0] }],
      ["head", 1, { rot: [-7, 3, 0] }],
      ["head", 2, { rot: [-10, 0, 0] }],
      ["leg_fl", 0, { rot: [-62, 0, -8] }],
      ["leg_fl", 2, { rot: [-62, 0, -8] }],
      ["leg_fr", 0, { rot: [-62, 0, 8] }],
      ["leg_fr", 2, { rot: [-62, 0, 8] }],
      ["tail", 0, { rot: [-5, 0, 0] }],
      ["tail", 1, { rot: [-1, 0, 3] }],
      ["tail", 2, { rot: [-5, 0, 0] }],
    ],
  });

  clip("cache_deposit", {
    label: "Cache deposit",
    role: "action",
    nextClip: "idle",
    fps: 24,
    loop: false,
    keys: [
      ["body", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["body", 0.35, { at: [0, -0.035, -0.035], rot: [13, 0, 0] }],
      ["body", 0.75, { at: [0, -0.045, -0.05], rot: [17, 0, 0] }],
      ["body", 1.1, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.35, { rot: [29, 0, 0] }],
      ["head", 0.75, { rot: [40, 0, 0] }],
      ["head", 1.1, { rot: [0, 0, 0] }],
      ["leg_fl", 0, { rot: [0, 0, 0] }],
      ["leg_fl", 0.35, { rot: [-38, 0, 0] }],
      ["leg_fl", 0.55, { rot: [34, 0, 0] }],
      ["leg_fl", 0.75, { rot: [-38, 0, 0] }],
      ["leg_fl", 1.1, { rot: [0, 0, 0] }],
      ["leg_fr", 0, { rot: [0, 0, 0] }],
      ["leg_fr", 0.35, { rot: [34, 0, 0] }],
      ["leg_fr", 0.55, { rot: [-38, 0, 0] }],
      ["leg_fr", 0.75, { rot: [34, 0, 0] }],
      ["leg_fr", 1.1, { rot: [0, 0, 0] }],
    ],
  });

  clip("cache_retrieve", {
    label: "Cache retrieve",
    role: "action",
    nextClip: "carry",
    fps: 24,
    loop: false,
    keys: [
      ["body", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["body", 0.4, { at: [0, -0.04, -0.04], rot: [16, 0, 0] }],
      ["body", 0.8, { at: [0, -0.025, 0], rot: [8, 0, 0] }],
      ["body", 1.2, { at: [0, 0.025, 0], rot: [-4, 0, 0] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.4, { rot: [39, 0, 0] }],
      ["head", 0.8, { rot: [18, 0, 0] }],
      ["head", 1.2, { rot: [-10, 0, 0] }],
      ["leg_fl", 0, { rot: [0, 0, 0] }],
      ["leg_fl", 0.4, { rot: [-36, 0, 0] }],
      ["leg_fl", 0.8, { rot: [-18, 0, 0] }],
      ["leg_fl", 1.2, { rot: [-62, 0, -8] }],
      ["leg_fr", 0, { rot: [0, 0, 0] }],
      ["leg_fr", 0.4, { rot: [31, 0, 0] }],
      ["leg_fr", 0.8, { rot: [-18, 0, 0] }],
      ["leg_fr", 1.2, { rot: [-62, 0, 8] }],
    ],
  });

  clip("climb", {
    label: "Trunk climb",
    role: "locomotion",
    fps: 24,
    loop: true,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.56,
      direction: [0, 1, 0],
      units: "figure",
    },
    keys: [
      ["body", 0, { at: [0, 0.5, 0], rot: [72, 0, 0] }],
      ["body", 0.4, { at: [0, 0.54, 0], rot: [74, 0, 0] }],
      ["body", 0.8, { at: [0, 0.5, 0], rot: [72, 0, 0] }],
      ["head", 0, { rot: [-9, 0, 0] }],
      ["head", 0.4, { rot: [-3, 0, 0] }],
      ["head", 0.8, { rot: [-9, 0, 0] }],
      ["leg_fl", 0, { rot: [-42, 0, 0] }],
      ["leg_fl", 0.4, { rot: [34, 0, 0] }],
      ["leg_fl", 0.8, { rot: [-42, 0, 0] }],
      ["leg_fr", 0, { rot: [34, 0, 0] }],
      ["leg_fr", 0.4, { rot: [-42, 0, 0] }],
      ["leg_fr", 0.8, { rot: [34, 0, 0] }],
      ["leg_bl", 0, { rot: [38, 0, 0] }],
      ["leg_bl", 0.4, { rot: [-30, 0, 0] }],
      ["leg_bl", 0.8, { rot: [38, 0, 0] }],
      ["leg_br", 0, { rot: [-30, 0, 0] }],
      ["leg_br", 0.4, { rot: [38, 0, 0] }],
      ["leg_br", 0.8, { rot: [-30, 0, 0] }],
      ["tail", 0, { rot: [48, 0, -4] }],
      ["tail", 0.4, { rot: [54, 0, 4] }],
      ["tail", 0.8, { rot: [48, 0, -4] }],
    ],
  });

  const appendRefugePose = (keys: Parameters<typeof clip>[1]["keys"], time: number) => {
    keys.push(
      ["body", time, { at: [0, -0.12, -0.06], rot: [10, 0, 0] }],
      ["head", time, { rot: [-8, 0, 0] }],
      ["leg_fl", time, { rot: [-48, 0, 0] }],
      ["leg_fr", time, { rot: [-48, 0, 0] }],
      ["leg_bl", time, { rot: [38, 0, 0] }],
      ["leg_br", time, { rot: [38, 0, 0] }],
      ["tail", time, { rot: [-31, 0, 20] }],
    );
  };
  const appendStandingPose = (keys: Parameters<typeof clip>[1]["keys"], time: number) => {
    for (const name of ["body", "head", "leg_fl", "leg_fr", "leg_bl", "leg_br", "tail"]) {
      keys.push([name, time, name === "body" ? { at: [0, 0, 0], rot: [0, 0, 0] } : { rot: [0, 0, 0] }]);
    }
  };

  const refugeEnterKeys: Parameters<typeof clip>[1]["keys"] = [];
  appendStandingPose(refugeEnterKeys, 0);
  refugeEnterKeys.push(
    ["body", 0.45, { at: [0, -0.05, -0.03], rot: [6, 0, 0] }],
    ["leg_fl", 0.45, { rot: [-24, 0, 0] }],
    ["leg_fr", 0.45, { rot: [-24, 0, 0] }],
    ["leg_bl", 0.45, { rot: [18, 0, 0] }],
    ["leg_br", 0.45, { rot: [18, 0, 0] }],
  );
  appendRefugePose(refugeEnterKeys, 0.9);
  clip("refuge_enter", {
    label: "Enter tree refuge",
    role: "action",
    nextClip: "refuge_idle",
    fps: 24,
    loop: false,
    keys: refugeEnterKeys,
  });

  const refugeIdleKeys: Parameters<typeof clip>[1]["keys"] = [];
  appendRefugePose(refugeIdleKeys, 0);
  appendRefugePose(refugeIdleKeys, 2.2);
  refugeIdleKeys.push(
    ["head", 1.1, { rot: [-5, 4, 0] }],
    ["ear_l", 0, { rot: [-8, 0, 5] }],
    ["ear_l", 1.1, { rot: [-14, 0, 8] }],
    ["ear_l", 2.2, { rot: [-8, 0, 5] }],
    ["ear_r", 0, { rot: [-10, 0, -6] }],
    ["ear_r", 2.2, { rot: [-10, 0, -6] }],
  );
  clip("refuge_idle", {
    label: "Tree refuge rest",
    role: "idle",
    fps: 20,
    loop: true,
    keys: refugeIdleKeys,
  });

  const refugeExitKeys: Parameters<typeof clip>[1]["keys"] = [];
  appendRefugePose(refugeExitKeys, 0);
  refugeExitKeys.push(
    ["body", 0.45, { at: [0, -0.05, -0.03], rot: [6, 0, 0] }],
    ["leg_fl", 0.45, { rot: [-24, 0, 0] }],
    ["leg_fr", 0.45, { rot: [-24, 0, 0] }],
    ["leg_bl", 0.45, { rot: [18, 0, 0] }],
    ["leg_br", 0.45, { rot: [18, 0, 0] }],
  );
  appendStandingPose(refugeExitKeys, 0.9);
  clip("refuge_exit", {
    label: "Leave tree refuge",
    role: "action",
    nextClip: "idle",
    fps: 24,
    loop: false,
    keys: refugeExitKeys,
  });

  clip("alarm", {
    label: "Tail-flag alarm",
    role: "action",
    nextClip: "flee",
    fps: 24,
    loop: false,
    keys: [
      ["body", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["body", 0.18, { at: [0, 0.07, 0], rot: [-6, 0, 0] }],
      ["body", 0.65, { at: [0, 0.025, 0], rot: [-3, 0, 0] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.18, { rot: [-15, -7, 0] }],
      ["head", 0.65, { rot: [-12, 8, 0] }],
      ["ear_l", 0, { rot: [0, 0, 0] }],
      ["ear_l", 0.18, { rot: [-16, 0, 9] }],
      ["ear_l", 0.65, { rot: [-9, 0, 4] }],
      ["ear_r", 0, { rot: [0, 0, 0] }],
      ["ear_r", 0.18, { rot: [-9, 0, -4] }],
      ["ear_r", 0.65, { rot: [-16, 0, -9] }],
      ["tail", 0, { rot: [0, 0, 0] }],
      ["tail", 0.18, { rot: [-34, 0, -12] }],
      ["tail", 0.42, { rot: [-24, 0, 13] }],
      ["tail", 0.65, { rot: [-31, 0, -8] }],
    ],
  });

  defaultClip("idle");
});
