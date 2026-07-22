import { figure } from "../../src/dsl";

// A naturalistic jackrabbit whose only fantasy trait is a compact forked
// antler rack. Long ears, high haunches, oversized rear feet, and a short tail
// preserve the hare silhouette through its bound and listening action.
export default figure("jackalope", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  metadata,
  part,
  quadrupedWalk,
}) => {
  metadata({
    bodyPlans: ["quadruped"],
    disposition: "passive",
    groups: ["animal", "fantasy"],
    habitats: ["land"],
    scale: "small",
    themes: ["desert", "folklore", "hare", "horned", "jackalope"],
  });

  mat("fur", "#a87848");
  mat("fur_light", "#d6b681");
  mat("fur_dark", "#65452e");
  mat("cream", "#ead8b5");
  mat("ear_inner", "#c77f76");
  mat("antler", "#81674b");
  mat("eye", "#1f1915");

  asciiTexture("jackalope_face", {
    palette: {
      ".": "#a87848",
      "d": "#65452e",
      "e": "#1f1915",
      "c": "#ead8b5",
    },
    pixels: [
      "dd......dd",
      "d..ee.ee.d",
      "...ee.ee..",
      "....cc....",
      "...cccc...",
      "....dd....",
      "..........",
      "dd......dd",
    ],
  });
  asciiTexture("hare_flank", {
    palette: { ".": "#a87848", "l": "#d6b681", "d": "#65452e" },
    pixels: [
      "ddd.........",
      "d..llll.....",
      "....ll......",
      ".......ddd..",
      "..ll....d...",
      "dddddddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.72, 0.06],
    size: [0.68, 0.54, 0.9],
    material: "fur",
    faces: {
      east: { texture: "hare_flank" },
      west: { texture: "hare_flank" },
    },
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.12, 0.34],
    size: [0.72, 0.58, 0.48],
    material: "fur_dark",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.29, -0.02],
    size: [0.44, 0.12, 0.58],
    material: "cream",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.22, -0.58],
    size: [0.5, 0.44, 0.44],
    material: "fur",
    faces: { north: { texture: "jackalope_face" } },
    joint: { pivot: [0, -0.18, 0.16], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.28],
    size: [0.32, 0.18, 0.18],
    material: "cream",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.14, 0.48, 0.04],
      rot: [4, 0, sign * 8],
      size: [0.15, 0.66, 0.12],
      material: "fur",
      joint: { pivot: [0, -0.31, 0], axis: [1, 0, 0] },
    }));
    part(`ear_inner_${side}`, box({
      parent: `ear_${side}`,
      at: [0, 0, -0.072],
      size: [0.08, 0.52, 0.025],
      material: "ear_inner",
    }));

    part(`antler_root_${side}`, box({
      parent: "head",
      at: [sign * 0.13, 0.36, 0.01],
      rot: [-6, 0, sign * -13],
      size: [0.07, 0.34, 0.07],
      material: "antler",
      joint: { pivot: [0, -0.15, 0], axis: [0, 0, 1] },
    }));
    part(`antler_beam_${side}`, box({
      parent: `antler_root_${side}`,
      at: [sign * 0.025, 0.27, 0.01],
      rot: [-7, 0, sign * -11],
      size: [0.06, 0.27, 0.06],
      material: "antler",
    }));
    part(`antler_tine_${side}`, box({
      parent: `antler_root_${side}`,
      at: [sign * 0.045, 0.14, -0.015],
      rot: [-18, 0, sign * 30],
      size: [0.055, 0.2, 0.055],
      material: "antler",
    }));

    part(`leg_f${side}`, box({
      parent: "body",
      at: [sign * 0.17, -0.43, -0.27],
      size: [0.14, 0.36, 0.16],
      material: "fur_light",
      joint: { pivot: [0, 0.17, 0], axis: [1, 0, 0] },
    }));
    part(`paw_f${side}`, box({
      parent: `leg_f${side}`,
      at: [0, -0.225, -0.04],
      size: [0.18, 0.11, 0.26],
      material: "cream",
    }));
    part(`leg_b${side}`, box({
      parent: "body",
      at: [sign * 0.25, -0.3, 0.3],
      rot: [-17, 0, 0],
      size: [0.28, 0.48, 0.4],
      material: "fur_dark",
      joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] },
    }));
    part(`paw_b${side}`, box({
      parent: `leg_b${side}`,
      at: [0, -0.33, -0.14],
      size: [0.25, 0.11, 0.52],
      material: "cream",
    }));
  }

  part("tail", box({
    parent: "rump",
    at: [0, 0.04, 0.34],
    rot: [-12, 0, 0],
    size: [0.24, 0.24, 0.24],
    material: "cream",
    joint: { pivot: [0, 0, -0.1], axis: [1, 0, 0] },
  }));

  quadrupedWalk("bound", {
    label: "Jackrabbit bound",
    fps: 24,
    duration: 0.72,
    cycleDistance: 0.76,
    gait: "trot",
    loop: true,
    samples: 21,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.045,
    bodyBobCenter: 0.047,
    head: "head",
    headSwingDegrees: 4,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.58,
    swingDegrees: 25,
    tail: "tail",
    tailSwingDegrees: 8,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 14, overshoot: 0.78, lag: 0.14 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 14, overshoot: 0.78, lag: 0.14 }),
      followThrough("antler_root_l", { source: "head", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 2, overshoot: 0.3, lag: 0.1 }),
      followThrough("antler_root_r", { source: "head", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 2, overshoot: 0.3, lag: 0.1 }),
    ],
  });
  clip("listen", {
    label: "Listen",
    role: "action",
    nextClip: "bound",
    fps: 30,
    loop: false,
    keys: [
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.24, { rot: [-8, -12, 0] }],
      ["head", 0.62, { rot: [-11, 10, 0] }],
      ["head", 0.94, { rot: [-6, 0, 0] }],
      ["head", 1.24, { rot: [0, 0, 0] }],
      ["ear_l", 0, { rot: [0, 0, 0] }],
      ["ear_l", 0.24, { rot: [-12, 0, -8] }],
      ["ear_l", 0.62, { rot: [8, 0, 6] }],
      ["ear_l", 1.24, { rot: [0, 0, 0] }],
      ["ear_r", 0, { rot: [0, 0, 0] }],
      ["ear_r", 0.24, { rot: [7, 0, 7] }],
      ["ear_r", 0.62, { rot: [-13, 0, -6] }],
      ["ear_r", 1.24, { rot: [0, 0, 0] }],
      ["tail", 0, { rot: [0, 0, 0] }],
      ["tail", 0.24, { rot: [-12, 0, 0] }],
      ["tail", 0.62, { rot: [-5, 0, 0] }],
      ["tail", 1.24, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("bound");
});
