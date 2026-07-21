import { figure } from "../../src/dsl";

// A box-only adult binturong with a shaggy charcoal coat, pale whiskers,
// plantigrade feet, and a five-stage prehensile tail with its own curl action.
export default figure("binturong", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  part,
  quadrupedWalk,
  swing,
}) => {
  mat("fur", "#343635");
  mat("fur_light", "#50534f");
  mat("fur_dark", "#202221");
  mat("cream", "#c9bda4");
  mat("muzzle", "#8d806d");
  mat("nose", "#171918");
  mat("eye", "#b98b43");
  mat("paw", "#282725");

  asciiTexture("coat", {
    palette: { ".": "#343635", "l": "#50534f", "d": "#202221", "c": "#8d806d" },
    pixels: [
      "dddddddddddd",
      "d.l..l..l..d",
      "..ll..ll....",
      ".l..l...l.l.",
      "....cccc....",
      "dddddddddddd",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#343635", "l": "#c9bda4", "e": "#b98b43", "d": "#202221" },
    pixels: [
      "dd....dd",
      "dle..eld",
      ".ll..ll.",
      "..llll..",
      "........",
      "dddddddd",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#8d806d", "n": "#171918", "c": "#c9bda4" },
    pixels: ["..cccc..", ".cccccc.", "..nnnn..", "...nn...", "........"],
  });

  part("body", box({
    at: [0, 0.76, 0.05],
    size: [0.76, 0.54, 1.18],
    material: "fur",
    faces: { east: { texture: "coat" }, west: { texture: "coat" } },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.3, 0.02],
    size: [0.56, 0.12, 0.82],
    material: "fur_dark",
  }));
  part("shoulder_ruff", box({
    parent: "body",
    at: [0, 0.04, -0.55],
    size: [0.82, 0.56, 0.2],
    material: "fur_light",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.02, -0.72],
    size: [0.58, 0.48, 0.5],
    material: "fur",
    faces: { north: { texture: "face" } },
    joint: { pivot: [0, 0, 0.21], axis: [1, 0, 0] },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.11, -0.34],
    size: [0.34, 0.22, 0.22],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.21, 0.28, 0.03],
      rot: [0, 0, sign * 10],
      size: [0.17, 0.22, 0.12],
      material: "fur_dark",
      joint: { pivot: [0, -0.09, 0], axis: [1, 0, 0] },
    }));
    part(`whisker_upper_${side}`, box({
      parent: "muzzle",
      at: [sign * 0.25, 0.02, -0.1],
      rot: [0, 0, sign * -8],
      size: [0.3, 0.025, 0.025],
      material: "cream",
    }));
    part(`whisker_lower_${side}`, box({
      parent: "muzzle",
      at: [sign * 0.24, -0.06, -0.08],
      rot: [0, 0, sign * 10],
      size: [0.28, 0.025, 0.025],
      material: "cream",
    }));
  }

  for (const [suffix, x, z] of [
    ["fl", -0.24, -0.34],
    ["fr", 0.24, -0.34],
    ["bl", -0.25, 0.38],
    ["br", 0.25, 0.38],
  ] as const) {
    part(`leg_${suffix}`, box({
      parent: "body",
      at: [x, -0.4, z],
      size: [0.16, 0.35, 0.18],
      material: "fur_dark",
      joint: { pivot: [0, 0.16, 0], axis: [1, 0, 0] },
    }));
    part(`paw_${suffix}`, box({
      parent: `leg_${suffix}`,
      at: [0, -0.22, -0.06],
      size: [0.22, 0.14, 0.28],
      material: "paw",
    }));
  }

  part("tail_1", box({
    parent: "body",
    at: [0, 0.07, 0.68],
    rot: [-8, 0, 0],
    size: [0.38, 0.34, 0.5],
    material: "fur_dark",
    joint: { pivot: [0, 0, -0.21], axis: [0, 1, 0] },
  }));
  for (const [index, width, length, pitch, material] of [
    [2, 0.34, 0.45, -8, "fur_light"],
    [3, 0.3, 0.42, -6, "fur"],
    [4, 0.25, 0.36, -4, "fur_dark"],
    [5, 0.19, 0.3, 2, "fur_light"],
  ] as const) {
    part(`tail_${index}`, box({
      parent: `tail_${index - 1}`,
      at: [0, 0, index === 2 ? 0.43 : index === 3 ? 0.38 : index === 4 ? 0.34 : 0.29],
      rot: [pitch, 0, 0],
      size: [width, width * 0.86, length],
      material,
      joint: { pivot: [0, 0, -length * 0.43], axis: [0, 1, 0] },
    }));
  }

  quadrupedWalk("prowl", {
    label: "Canopy prowl",
    fps: 18,
    duration: 1.16,
    cycleDistance: 0.58,
    gait: "walk",
    loop: true,
    samples: 21,
    contactParts: {
      frontLeft: "paw_fl",
      frontRight: "paw_fr",
      backLeft: "paw_bl",
      backRight: "paw_br",
    },
    body: "body",
    bodyBob: 0.012,
    bodyBobCenter: 0.014,
    head: "head",
    headSwingDegrees: 2.5,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.7,
    swingDegrees: 15,
    tail: "tail_1",
    tailSwingDegrees: 8,
    tracks: [
      followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.12 }),
      followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 7, overshoot: 0.55, lag: 0.12 }),
      followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 10, overshoot: 0.75, lag: 0.14 }),
      followThrough("tail_3", { source: "tail_2", sourceChannel: "rot", sourceAxis: "y", axis: "y", degrees: 12, overshoot: 0.78, lag: 0.17 }),
    ],
  });
  clip("tail_curl", {
    label: "Prehensile tail curl",
    role: "action",
    nextClip: "prowl",
    fps: 30,
    loop: false,
    keys: [
      ["tail_1", 0, { rot: [0, 0, 0] }],
      ["tail_1", 0.42, { rot: [-10, 18, 0] }],
      ["tail_1", 0.72, { rot: [-12, 24, 0] }],
      ["tail_1", 1.04, { rot: [0, 0, 0] }],
      ["tail_2", 0, { rot: [0, 0, 0] }],
      ["tail_2", 0.42, { rot: [-18, 30, 0] }],
      ["tail_2", 0.72, { rot: [-22, 42, 0] }],
      ["tail_2", 1.04, { rot: [0, 0, 0] }],
      ["tail_3", 0, { rot: [0, 0, 0] }],
      ["tail_3", 0.42, { rot: [-22, 38, 0] }],
      ["tail_3", 0.72, { rot: [-26, 54, 0] }],
      ["tail_3", 1.04, { rot: [0, 0, 0] }],
      ["tail_4", 0, { rot: [0, 0, 0] }],
      ["tail_4", 0.42, { rot: [-24, 46, 0] }],
      ["tail_4", 0.72, { rot: [-30, 66, 0] }],
      ["tail_4", 1.04, { rot: [0, 0, 0] }],
      ["tail_5", 0, { rot: [0, 0, 0] }],
      ["tail_5", 0.42, { rot: [-20, 54, 0] }],
      ["tail_5", 0.72, { rot: [-24, 78, 0] }],
      ["tail_5", 1.04, { rot: [0, 0, 0] }],
      ["head", 0, { rot: [0, 0, 0] }],
      ["head", 0.5, { rot: [5, -5, 0] }],
      ["head", 1.04, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("prowl");
});
