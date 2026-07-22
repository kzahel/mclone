import { figure } from "../../src/dsl";

// A horse-and-fish hippocampus with equine forequarters, webbed forelegs,
// lateral fins, and a long articulated fish tail instead of hindquarters.
export default figure("hippocampus", ({
  asciiTexture,
  bob,
  box,
  clip,
  defaultClip,
  mat,
  metadata,
  part,
  swim,
  swing,
}) => {
  metadata({
    bodyPlans: ["quadruped", "swimmer"],
    disposition: "neutral",
    groups: ["animal", "fantasy"],
    habitats: ["water"],
    scale: "large",
    themes: ["equine", "fish", "hippocampus", "hybrid", "marine", "mythic"],
  });

  mat("coat", "#4b8792");
  mat("coat_light", "#78b7b1");
  mat("coat_dark", "#285866");
  mat("belly", "#b9d9c7");
  mat("fin", "#d6a85b");
  mat("fin_dark", "#9a6d3f");
  mat("mane", "#244653");
  mat("eye", "#e6c75c");

  asciiTexture("horse_face", {
    palette: { ".": "#4b8792", "e": "#e6c75c", "p": "#172126", "l": "#78b7b1" },
    pixels: [
      "ll......ll",
      "l.ep..pe.l",
      "..ep..pe..",
      "..........",
      "....ll....",
      "...llll...",
      "..........",
      "..........",
    ],
  });
  asciiTexture("scale_flank", {
    palette: { ".": "#4b8792", "l": "#78b7b1", "d": "#285866", "b": "#b9d9c7" },
    pixels: [
      "dddddddddddd",
      "dll..ll..lld",
      "d..ll..ll..d",
      "..ll..ll....",
      "....ll..ll..",
      "..bbbbbbbb..",
      "bbbbbbbbbbbb",
    ],
  });
  asciiTexture("fin_rays", {
    palette: { ".": "#d6a85b", "l": "#f0cb79", "d": "#9a6d3f" },
    pixels: [
      "dddddddd",
      "dll.llld",
      "dl.ll.ld",
      "dll.llld",
      "dl.ll.ld",
      "dddddddd",
    ],
  });
  asciiTexture("tail_bands", {
    palette: { ".": "#4b8792", "l": "#78b7b1", "d": "#285866" },
    pixels: ["llllllll", "........", "dddddddd", "........", "llllllll", "dddddddd"],
  });

  part("forebody", box({
    at: [0, 1.15, -0.16],
    size: [0.88, 0.7, 1.22],
    material: "coat",
    faces: {
      east: { texture: "scale_flank" },
      west: { texture: "scale_flank" },
    },
  }));
  part("chest", box({
    parent: "forebody",
    at: [0, 0, -0.5],
    size: [0.92, 0.72, 0.46],
    material: "coat_light",
  }));
  part("belly", box({
    parent: "forebody",
    at: [0, -0.37, 0.04],
    size: [0.68, 0.12, 0.86],
    material: "belly",
  }));
  part("neck", box({
    parent: "forebody",
    at: [0, 0.48, -0.48],
    rot: [-36, 0, 0],
    size: [0.46, 0.86, 0.48],
    material: "coat",
    joint: { pivot: [0, -0.39, 0.1], axis: [1, 0, 0] },
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.51, -0.18],
    rot: [28, 0, 0],
    size: [0.42, 0.44, 0.58],
    material: "coat",
    faces: { north: { texture: "horse_face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.12, -0.44],
    size: [0.32, 0.26, 0.36],
    material: "coat_light",
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({
      parent: "head",
      at: [sign * 0.14, 0.3, 0.1],
      rot: [-5, 0, sign * 10],
      size: [0.12, 0.26, 0.1],
      material: "fin",
      joint: { pivot: [0, -0.11, 0], axis: [1, 0, 0] },
    }));
  }
  for (const [index, y, size] of [
    [1, 0.3, [0.08, 0.22, 0.18]],
    [2, 0.08, [0.1, 0.28, 0.22]],
    [3, -0.14, [0.12, 0.3, 0.24]],
    [4, -0.35, [0.09, 0.27, 0.22]],
  ] as const) {
    part(`mane_fin_${index}`, box({
      parent: "neck",
      at: [0, y, 0.31],
      rot: [8, 0, 0],
      size,
      material: index % 2 === 0 ? "fin" : "mane",
      faces: { east: { texture: "fin_rays" }, west: { texture: "fin_rays" } },
    }));
  }

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`foreleg_${side}`, box({
      parent: "chest",
      at: [sign * 0.29, -0.47, -0.03],
      rot: [24, 0, sign * 5],
      size: [0.19, 0.64, 0.21],
      material: "coat_light",
      joint: { pivot: [0, 0.3, 0], axis: [1, 0, 0] },
    }));
    part(`webbed_hoof_${side}`, box({
      parent: `foreleg_${side}`,
      at: [sign * 0.03, -0.4, -0.06],
      rot: [0, sign * -8, sign * 8],
      size: [0.34, 0.1, 0.42],
      material: "fin",
      faces: { up: { texture: "fin_rays" }, down: { texture: "fin_rays" } },
    }));
    part(`pectoral_fin_${side}`, box({
      parent: "forebody",
      at: [sign * 0.58, -0.05, -0.14],
      rot: [0, sign * -12, sign * 10],
      size: [0.62, 0.08, 0.46],
      material: "fin_dark",
      faces: { up: { texture: "fin_rays" }, down: { texture: "fin_rays" } },
      joint: { pivot: [sign * -0.28, 0, -0.12], axis: [0, 0, 1] },
    }));
  }

  part("dorsal_fin", box({
    parent: "forebody",
    at: [0, 0.48, 0.32],
    rot: [-10, 0, 0],
    size: [0.1, 0.5, 0.5],
    material: "fin_dark",
    faces: { east: { texture: "fin_rays" }, west: { texture: "fin_rays" } },
  }));
  part("tail_base", box({
    parent: "forebody",
    at: [0, -0.04, 0.78],
    size: [0.62, 0.52, 0.66],
    material: "coat",
    faces: { east: { texture: "tail_bands" }, west: { texture: "tail_bands" } },
    joint: { pivot: [0, 0, -0.29], axis: [0, 1, 0] },
  }));
  part("tail_1", box({
    parent: "tail_base",
    at: [0, -0.03, 0.54],
    size: [0.5, 0.44, 0.56],
    material: "coat_light",
    faces: { east: { texture: "tail_bands" }, west: { texture: "tail_bands" } },
    joint: { pivot: [0, 0, -0.25], axis: [0, 1, 0] },
  }));
  part("tail_2", box({
    parent: "tail_1",
    at: [0, -0.02, 0.46],
    size: [0.39, 0.35, 0.48],
    material: "coat_dark",
    faces: { east: { texture: "tail_bands" }, west: { texture: "tail_bands" } },
    joint: { pivot: [0, 0, -0.21], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail_2",
    at: [0, 0, 0.4],
    size: [0.26, 0.27, 0.42],
    material: "coat_light",
    joint: { pivot: [0, 0, -0.18], axis: [0, 1, 0] },
  }));
  part("caudal_upper", box({
    parent: "tail_tip",
    at: [0, 0.33, 0.24],
    rot: [-9, 0, 0],
    size: [0.1, 0.58, 0.44],
    material: "fin",
    faces: { east: { texture: "fin_rays" }, west: { texture: "fin_rays" } },
  }));
  part("caudal_lower", box({
    parent: "tail_tip",
    at: [0, -0.3, 0.23],
    rot: [9, 0, 0],
    size: [0.1, 0.5, 0.4],
    material: "fin",
    faces: { east: { texture: "fin_rays" }, west: { texture: "fin_rays" } },
  }));

  swim("tidal_swim", {
    label: "Tidal swim",
    fps: 24,
    duration: 1.28,
    cycleDistance: 1.16,
    loop: true,
    samples: 31,
    body: "forebody",
    bodyBob: 0.025,
    bodySwayDegrees: 2.3,
    finSwingDegrees: 10,
    leftFin: "pectoral_fin_l",
    rightFin: "pectoral_fin_r",
    tail: "tail_base",
    tailSwingDegrees: 8,
    tailTip: "tail_1",
    tailTipPhase: 0.11,
    tailTipSwingDegrees: 13,
    tracks: [
      swing("tail_2", { axis: "y", degrees: 18, phase: 0.22 }),
      swing("tail_tip", { axis: "y", degrees: 24, phase: 0.33 }),
      swing("foreleg_l", { axis: "x", degrees: 9, phase: 0.5 }),
      swing("foreleg_r", { axis: "x", degrees: 9, phase: 0 }),
      swing("mane_fin_2", { axis: "y", degrees: 5, phase: 0.18 }),
      swing("mane_fin_3", { axis: "y", degrees: 7, phase: 0.28 }),
    ],
  });
  clip("cresting_breach", {
    label: "Cresting breach",
    role: "action",
    nextClip: "tidal_swim",
    fps: 30,
    loop: false,
    keys: [
      ["forebody", 0, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["forebody", 0.38, { at: [0, 0.18, 0], rot: [-9, 0, 0] }],
      ["forebody", 0.82, { at: [0, 0.34, 0], rot: [-15, 0, 0] }],
      ["forebody", 1.48, { at: [0, 0, 0], rot: [0, 0, 0] }],
      ["neck", 0, { rot: [0, 0, 0] }],
      ["neck", 0.38, { rot: [-7, 0, 0] }],
      ["neck", 0.82, { rot: [-12, 0, 0] }],
      ["neck", 1.48, { rot: [0, 0, 0] }],
      ["pectoral_fin_l", 0, { rot: [0, 0, 0] }],
      ["pectoral_fin_l", 0.38, { rot: [0, -6, -26] }],
      ["pectoral_fin_l", 0.82, { rot: [0, -10, -42] }],
      ["pectoral_fin_l", 1.48, { rot: [0, 0, 0] }],
      ["pectoral_fin_r", 0, { rot: [0, 0, 0] }],
      ["pectoral_fin_r", 0.38, { rot: [0, 6, 26] }],
      ["pectoral_fin_r", 0.82, { rot: [0, 10, 42] }],
      ["pectoral_fin_r", 1.48, { rot: [0, 0, 0] }],
      ["foreleg_l", 0, { rot: [0, 0, 0] }],
      ["foreleg_l", 0.38, { rot: [-22, 0, -6] }],
      ["foreleg_l", 0.82, { rot: [-36, 0, -10] }],
      ["foreleg_l", 1.48, { rot: [0, 0, 0] }],
      ["foreleg_r", 0, { rot: [0, 0, 0] }],
      ["foreleg_r", 0.38, { rot: [-22, 0, 6] }],
      ["foreleg_r", 0.82, { rot: [-36, 0, 10] }],
      ["foreleg_r", 1.48, { rot: [0, 0, 0] }],
      ["tail_base", 0, { rot: [0, 0, 0] }],
      ["tail_base", 0.38, { rot: [0, -10, 0] }],
      ["tail_base", 0.82, { rot: [0, 15, 0] }],
      ["tail_base", 1.48, { rot: [0, 0, 0] }],
      ["tail_1", 0, { rot: [0, 0, 0] }],
      ["tail_1", 0.38, { rot: [0, -18, 0] }],
      ["tail_1", 0.82, { rot: [0, 27, 0] }],
      ["tail_1", 1.48, { rot: [0, 0, 0] }],
      ["tail_2", 0, { rot: [0, 0, 0] }],
      ["tail_2", 0.38, { rot: [0, -27, 0] }],
      ["tail_2", 0.82, { rot: [0, 38, 0] }],
      ["tail_2", 1.48, { rot: [0, 0, 0] }],
      ["tail_tip", 0, { rot: [0, 0, 0] }],
      ["tail_tip", 0.38, { rot: [0, -36, 0] }],
      ["tail_tip", 0.82, { rot: [0, 50, 0] }],
      ["tail_tip", 1.48, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("tidal_swim");
});
