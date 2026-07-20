import { figure } from "../../src/dsl";

// A box-only adult North American beaver with a deep brown barrel, blunt
// muzzle, visible orange incisors, compact legs, and a broad crosshatched
// paddle tail. The tail follows the body with a restrained lateral sweep.
export default figure("beaver", ({
  mat,
  asciiTexture,
  part,
  box,
  quadrupedWalk,
  swing,
  followThrough,
}) => {
  mat("fur", "#65452f");
  mat("fur_light", "#8a6241");
  mat("fur_dark", "#35291f");
  mat("belly", "#a27d58");
  mat("muzzle", "#b28c67");
  mat("eye", "#171411");
  mat("nose", "#241c18");
  mat("incisor", "#d98a35");
  mat("tail", "#3b2b22");
  mat("paw", "#332820");

  asciiTexture("face", {
    palette: { ".": "#8a6241", "d": "#65452f", "e": "#171411", "l": "#b28c67" },
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
    palette: { ".": "#b28c67", "n": "#241c18", "l": "#d0ad83" },
    pixels: [
      "ll....ll",
      ".lnnnnl.",
      "..nnnn..",
      "........",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#65452f", "l": "#8a6241", "d": "#35291f", "b": "#a27d58" },
    pixels: [
      "dddddddddddd",
      "dlllllllllld",
      "l..........l",
      "............",
      "..bbbbbbbb..",
      ".bbbbbbbbbb.",
    ],
  });
  asciiTexture("tail_crosshatch", {
    palette: { ".": "#3b2b22", "l": "#60483a", "d": "#261d18" },
    pixels: [
      "d..l..d..l",
      ".d..l..d..",
      "..d..l..d.",
      "l..d..l..d",
      ".l..d..l..",
      "..l..d..l.",
    ],
  });

  part("body", box({
    at: [0, 0.72, 0.06],
    size: [0.86, 0.58, 1.22],
    material: "fur",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.34, -0.02],
    size: [0.66, 0.12, 0.88],
    material: "belly",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.04, -0.48],
    size: [0.88, 0.52, 0.38],
    material: "fur_light",
  }));
  part("head", box({
    parent: "shoulders",
    at: [0, 0.1, -0.39],
    size: [0.66, 0.56, 0.52],
    material: "fur_light",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.22], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.15, -0.36],
    size: [0.5, 0.24, 0.28],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  for (const [side, x] of [["l", -0.09], ["r", 0.09]] as const) {
    part(`incisor_${side}`, box({
      parent: "muzzle",
      at: [x, -0.17, -0.17],
      size: [0.13, 0.22, 0.07],
      material: "incisor",
    }));
  }
  for (const [side, x] of [["l", -0.27], ["r", 0.27]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [x, 0.3, 0.07],
      size: [0.16, 0.19, 0.12],
      material: "fur_dark",
      joint: { pivot: [0, -0.07, 0], axis: [1, 0, 0] },
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.3, -0.36],
    ["fr", 0.3, -0.36],
    ["bl", -0.3, 0.38],
    ["br", 0.3, 0.38],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.44, z],
      size: [0.18, 0.38, 0.2],
      material: "fur_dark",
      joint: { pivot: [0, 0.19, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.24, -0.055],
      size: [0.26, 0.1, 0.3],
      material: "paw",
    }));
  }

  part("tail_base", box({
    parent: "body",
    at: [0, -0.12, 0.78],
    size: [0.34, 0.18, 0.48],
    material: "tail",
    joint: { pivot: [0, 0, -0.22], axis: [0, 1, 0] },
  }));
  part("tail_paddle", box({
    parent: "tail_base",
    at: [0, -0.03, 0.52],
    size: [0.68, 0.12, 0.7],
    material: "tail",
    faces: { up: { texture: "tail_crosshatch" } },
    joint: { pivot: [0, 0, -0.32], axis: [0, 1, 0] },
  }));

  quadrupedWalk("walk", {
    fps: 18,
    duration: 1.18,
    cycleDistance: 0.62,
    gait: "walk",
    loop: true,
    samples: 19,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.014,
    bodyBobCenter: 0.016,
    head: "head",
    headSwingDegrees: 2.2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.7,
    swingDegrees: 15,
    tracks: [
      swing("tail_base", { axis: "y", degrees: 7, phase: 0.5 }),
      followThrough("tail_paddle", { source: "tail_base", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 10, overshoot: 0.4, lag: 0.12 }),
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.45, lag: 0.1 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.45, lag: 0.1 }),
    ],
  });
});
