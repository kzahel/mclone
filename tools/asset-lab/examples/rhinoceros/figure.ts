import { figure } from "../../src/dsl";

// A box-only rhinoceros built around a deep shoulder mass, low head, plated
// hide, and a stepped two-horn profile. Its slow walk keeps the broad feet
// planted and lets the head carry only a restrained counter-sway.
export default figure("rhinoceros", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  followThrough,
}) => {
  mat("hide", "#777873");
  mat("hide_light", "#92938c");
  mat("hide_shadow", "#5f615d");
  mat("plate", "#6b6d68");
  mat("horn", "#d9cfad");
  mat("horn_tip", "#9e967c");
  mat("ear_inner", "#796b6b");
  mat("eye", "#151514");
  mat("hoof", "#4f504d");

  asciiTexture("shoulder_folds", {
    palette: { ".": "#777873", "d": "#5f615d", "l": "#92938c" },
    pixels: [
      "..dd........",
      ".d..d...lll.",
      "d....d......",
      ".d..d.......",
      "..dd....dd..",
      "......dd..d.",
      ".........d..",
      "............",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#92938c", "n": "#343532" },
    pixels: [
      "........",
      ".nn..nn.",
      ".nn..nn.",
      "........",
      "........",
      "........",
    ],
  });

  part("body", box({
    at: [0, 1.12, 0.1],
    size: [1.18, 0.82, 1.66],
    material: "hide",
    faces: {
      east: { texture: "shoulder_folds" },
      west: { texture: "shoulder_folds" },
    },
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.12, -0.54],
    size: [1.28, 0.76, 0.58],
    material: "plate",
  }));
  part("rump", box({
    parent: "body",
    at: [0, 0.06, 0.59],
    size: [1.12, 0.72, 0.54],
    material: "hide_light",
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.44, 0.08],
    size: [0.96, 0.12, 1.22],
    material: "hide_shadow",
  }));

  part("neck", box({
    parent: "body",
    at: [0, 0.02, -0.94],
    rot: [-8, 0, 0],
    size: [0.9, 0.66, 0.52],
    material: "plate",
    joint: { pivot: [0, 0, 0.23], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, -0.12, -0.48],
    rot: [7, 0, 0],
    size: [0.72, 0.56, 0.7],
    material: "hide_light",
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.46],
    size: [0.64, 0.38, 0.34],
    material: "hide_light",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.23], ["r", 0.23]] as const) {
    part(`eye_${side}`, box({
      parent: "head",
      at: [x, 0.08, -0.355],
      size: [0.07, 0.07, 0.03],
      material: "eye",
    }));
    part(`ear_${side}`, box({
      parent: "head",
      at: [x * 1.35, 0.34, 0.12],
      rot: [0, 0, side === "l" ? -15 : 15],
      size: [0.2, 0.27, 0.11],
      material: "ear_inner",
      joint: { pivot: [side === "l" ? 0.07 : -0.07, -0.08, 0], axis: [1, 0, 0] },
    }));
  }

  // Each horn remains visibly cuboid: a broad base and a smaller forward tip.
  part("horn_front", box({
    parent: "head",
    at: [0, 0.39, -0.36],
    rot: [-24, 0, 0],
    size: [0.18, 0.5, 0.18],
    material: "horn",
  }));
  part("horn_front_tip", box({
    parent: "horn_front",
    at: [0, 0.3, -0.07],
    size: [0.11, 0.22, 0.11],
    material: "horn_tip",
  }));
  part("horn_rear", box({
    parent: "head",
    at: [0, 0.35, -0.02],
    rot: [-12, 0, 0],
    size: [0.15, 0.28, 0.15],
    material: "horn",
  }));
  part("horn_rear_tip", box({
    parent: "horn_rear",
    at: [0, 0.18, -0.025],
    size: [0.09, 0.13, 0.09],
    material: "horn_tip",
  }));

  for (const [suffix, x, z] of [
    ["fl", -0.42, -0.53],
    ["fr", 0.42, -0.53],
    ["bl", -0.4, 0.53],
    ["br", 0.4, 0.53],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.65, z],
      size: [0.28, 0.55, 0.3],
      material: suffix[0] === "f" ? "plate" : "hide",
      joint: { pivot: [0, 0.275, 0], axis: [1, 0, 0] },
    }));
    part(`foot_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.34, -0.055],
      size: [0.38, 0.16, 0.44],
      material: "hoof",
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, -0.08, 0.91],
    rot: [-8, 0, 0],
    size: [0.08, 0.5, 0.08],
    material: "hide_shadow",
    joint: { pivot: [0, 0.25, 0], axis: [0, 0, 1] },
  }));
  part("tail_tuft", box({
    parent: "tail",
    at: [0, -0.32, 0.02],
    size: [0.18, 0.18, 0.16],
    material: "hide_shadow",
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.42,
    cycleDistance: 0.78,
    gait: "walk",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "foot_fl",
      frontRight: "foot_fr",
      backLeft: "foot_bl",
      backRight: "foot_br",
    },
    body: "body",
    bodyBob: 0.016,
    bodyBobCenter: 0.018,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.72,
    swingDegrees: 13,
    tail: "tail",
    tailSwingDegrees: 7,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.35, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.35, lag: 0.12 }),
    ],
  });
});
