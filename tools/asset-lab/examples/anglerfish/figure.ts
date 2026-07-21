import { figure } from "../../src/dsl";

// A box-only deep-sea anglerfish with a massive toothed head, tiny eyes,
// glowing two-stage lure, compact fins, and a separate abrupt jaw snap.
export default figure("anglerfish", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  mat,
  part,
  swing,
  swim,
}) => {
  mat("navy", "#263844");
  mat("navy_light", "#38515d");
  mat("navy_dark", "#17252d");
  mat("mottle", "#4b6166");
  mat("mouth", "#4f292d");
  mat("gum", "#8a4a4d");
  mat("tooth", "#e6dfbe");
  mat("eye", "#d5b84c");
  mat("pupil", "#101412");
  mat("lure", { color: "#72dfd5", roughness: 0.2 });
  mat("lure_core", { color: "#d0fff0", roughness: 0.1 });

  asciiTexture("body_mottle", {
    palette: { ".": "#263844", "l": "#38515d", "d": "#17252d", "m": "#4b6166" },
    pixels: [
      "dddddddddddd",
      "dll......lld",
      "d..mm..mm..d",
      "d.m..mm..m.d",
      "d...llll...d",
      "d.mm....mm.d",
      "dll......lld",
      "dddddddddddd",
    ],
  });
  asciiTexture("jaw_side", {
    palette: { ".": "#263844", "d": "#17252d", "g": "#8a4a4d", "t": "#e6dfbe" },
    pixels: ["dddddddddd", "d........d", "gggggggggg", "gttttttttg", "gggggggggg"],
  });
  asciiTexture("tail_rays", {
    palette: { ".": "#38515d", "d": "#17252d", "l": "#4b6166" },
    pixels: ["llllllll", "l..dd..l", "l.d..d.l", "l..dd..l", "dddddddd"],
  });

  part("body", box({
    at: [0, 0.9, 0.1],
    size: [0.9, 0.72, 1.12],
    material: "navy",
    faces: { east: { texture: "body_mottle" }, west: { texture: "body_mottle" } },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.38, -0.04],
    size: [0.72, 0.12, 0.78],
    material: "navy_light",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.02, -0.69],
    size: [1.04, 0.78, 0.5],
    material: "navy_dark",
    faces: { east: { texture: "body_mottle" }, west: { texture: "body_mottle" } },
  }));
  part("upper_snout", box({
    parent: "head",
    at: [0, 0.13, -0.31],
    size: [0.9, 0.34, 0.28],
    material: "navy",
    faces: { down: { material: "mouth" } },
    joint: { pivot: [0, 0, 0.12], axis: [1, 0, 0] },
  }));
  part("lower_jaw", box({
    parent: "head",
    at: [0, -0.24, -0.3],
    size: [0.84, 0.22, 0.46],
    material: "navy_light",
    faces: {
      up: { material: "mouth" },
      east: { texture: "jaw_side" },
      west: { texture: "jaw_side" },
    },
    joint: { pivot: [0, 0, 0.19], axis: [1, 0, 0] },
  }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_mound_${side}`, box({
      parent: "head",
      at: [sign * 0.46, 0.22, -0.18],
      size: [0.18, 0.18, 0.18],
      material: "mottle",
    }));
    part(`eye_${side}`, box({
      parent: `eye_mound_${side}`,
      at: [0, 0, -0.11],
      size: [0.1, 0.1, 0.06],
      material: "eye",
      faces: { north: { material: "pupil" } },
    }));
  }
  for (const [index, x] of [[1, -0.27], [2, -0.09], [3, 0.09], [4, 0.27]] as const) {
    part(`tooth_upper_${index}`, box({
      parent: "upper_snout",
      at: [x, -0.2, -0.05],
      rot: [0, 0, index % 2 === 0 ? -3 : 3],
      size: [0.08, 0.18, 0.09],
      material: "tooth",
    }));
    part(`tooth_lower_${index}`, box({
      parent: "lower_jaw",
      at: [x, 0.15, -0.04],
      rot: [0, 0, index % 2 === 0 ? 3 : -3],
      size: [0.075, 0.16, 0.085],
      material: "tooth",
    }));
  }

  part("lure_base", box({
    parent: "head",
    at: [0, 0.51, 0.04],
    rot: [-12, 0, 0],
    size: [0.085, 0.34, 0.085],
    material: "navy_light",
    joint: { pivot: [0, -0.15, 0], axis: [1, 0, 0] },
  }));
  part("lure_stem", box({
    parent: "lure_base",
    at: [0, 0.27, -0.08],
    rot: [-24, 0, 0],
    size: [0.065, 0.3, 0.065],
    material: "mottle",
    joint: { pivot: [0, -0.13, 0], axis: [1, 0, 0] },
  }));
  part("lure_bulb", box({
    parent: "lure_stem",
    at: [0, 0.21, -0.08],
    size: [0.22, 0.2, 0.2],
    material: "lure",
    faces: { north: { material: "lure_core" }, up: { material: "lure_core" } },
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`fin_${side}`, box({
      parent: "body",
      at: [sign * 0.57, -0.08, -0.12],
      rot: [0, sign * -8, sign * 10],
      size: [0.38, 0.07, 0.36],
      material: "navy_light",
      joint: { pivot: [sign * -0.17, 0, -0.1], axis: [0, 0, 1] },
    }));
  }
  part("dorsal_fin", box({
    parent: "body",
    at: [0, 0.45, 0.18],
    rot: [-8, 0, 0],
    size: [0.08, 0.28, 0.42],
    material: "navy_light",
  }));
  part("tail", box({
    parent: "body",
    at: [0, 0, 0.69],
    size: [0.22, 0.38, 0.38],
    material: "navy_dark",
    joint: { pivot: [0, 0, -0.18], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0, 0.33],
    size: [0.09, 0.72, 0.46],
    material: "navy_light",
    faces: { east: { texture: "tail_rays" }, west: { texture: "tail_rays" } },
    joint: { pivot: [0, 0, -0.21], axis: [0, 1, 0] },
  }));

  swim("hover", {
    label: "Deep hover",
    fps: 22,
    duration: 1.18,
    cycleDistance: 0.36,
    loop: true,
    samples: 27,
    body: "body",
    bodyBob: 0.026,
    bodySwayDegrees: 2.4,
    finSwingDegrees: 9,
    leftFin: "fin_l",
    rightFin: "fin_r",
    tail: "tail",
    tailSwingDegrees: 12,
    tailTip: "tail_tip",
    tailTipPhase: 0.11,
    tailTipSwingDegrees: 18,
    tracks: [
      swing("lure_base", { axis: "x", degrees: 7, phase: 0.08 }),
      swing("lure_stem", { axis: "x", degrees: 12, phase: 0.2 }),
      swing("lower_jaw", { axis: "x", degrees: 1.5, center: -1, frequency: 0.5 }),
    ],
  });
  clip("jaw_snap", {
    label: "Jaw snap",
    role: "action",
    nextClip: "hover",
    fps: 30,
    loop: false,
    keys: [
      ["lower_jaw", 0, { rot: [0, 0, 0] }],
      ["lower_jaw", 0.12, { rot: [-18, 0, 0] }],
      ["lower_jaw", 0.22, { rot: [-36, 0, 0] }],
      ["lower_jaw", 0.3, { rot: [-36, 0, 0] }],
      ["lower_jaw", 0.34, { rot: [3, 0, 0] }],
      ["lower_jaw", 0.42, { rot: [-1, 0, 0] }],
      ["lower_jaw", 0.58, { rot: [0, 0, 0] }],
      ["upper_snout", 0, { rot: [0, 0, 0] }],
      ["upper_snout", 0.22, { rot: [-3, 0, 0] }],
      ["upper_snout", 0.34, { rot: [2, 0, 0] }],
      ["upper_snout", 0.58, { rot: [0, 0, 0] }],
      ["lure_base", 0, { rot: [0, 0, 0] }],
      ["lure_base", 0.22, { rot: [-8, 0, 0] }],
      ["lure_base", 0.34, { rot: [12, 0, 0] }],
      ["lure_base", 0.58, { rot: [0, 0, 0] }],
    ],
  });
  defaultClip("hover");
});
