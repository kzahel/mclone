import { figure } from "../../src/dsl";

// A box-only great white shark with a long two-mass body, pale belly, tall
// dorsal fin, swept pectorals, gill and mouth textures, and a vertical caudal
// fin driven through the shared two-stage swim tail.
export default figure("shark", ({
  mat,
  asciiTexture,
  part,
  box,
  swim,
}) => {
  mat("skin", "#596d77");
  mat("skin_light", "#71858d");
  mat("skin_dark", "#324650");
  mat("belly", "#d3d4cb");
  mat("mouth", "#211c1c");
  mat("tooth", "#f2eee2");

  asciiTexture("mouth_face", {
    palette: { "m": "#211c1c", "t": "#f2eee2" },
    pixels: [
      "mmmmmmmm",
      "mttttttm",
      "mttttttm",
      "mmmmmmmm",
    ],
  });
  asciiTexture("gills", {
    palette: { ".": "#596d77", "g": "#253740", "l": "#71858d" },
    pixels: [
      "llllllll",
      "........",
      ".g.g.g..",
      ".g.g.g..",
      ".g.g.g..",
      "........",
      "........",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#596d77", "d": "#324650", "l": "#71858d", "b": "#d3d4cb" },
    pixels: [
      "dddddddddddddddd",
      "dlllllllllllllld",
      "l..............l",
      "................",
      "..bbbbbbbbbbbb..",
      ".bbbbbbbbbbbbbb.",
      "bbbbbbbbbbbbbbbb",
    ],
  });
  asciiTexture("tail_pattern", {
    palette: { ".": "#596d77", "d": "#324650", "l": "#71858d" },
    pixels: [
      "dddddddd",
      "dll....d",
      "dl.....d",
      "d......d",
      "d......d",
      "dl.....d",
      "dll....d",
      "dddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.9, 0.08],
    size: [0.82, 0.64, 1.58],
    material: "skin",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.36, -0.06],
    size: [0.68, 0.13, 1.18],
    material: "belly",
  }));
  part("shoulder", box({
    parent: "body",
    at: [0, 0.04, -0.72],
    size: [0.86, 0.6, 0.44],
    material: "skin_light",
    faces: {
      east: { texture: "gills" },
      west: { texture: "gills" },
    },
  }));
  part("head", box({
    parent: "shoulder",
    at: [0, -0.02, -0.42],
    size: [0.78, 0.54, 0.5],
    material: "skin_light",
  }));
  part("snout", box({
    parent: "head",
    at: [0, 0.01, -0.34],
    size: [0.68, 0.4, 0.22],
    material: "skin_light",
  }));
  part("jaw", box({
    parent: "head",
    at: [0, -0.24, -0.23],
    size: [0.58, 0.12, 0.36],
    material: "mouth",
    faces: { north: { texture: "mouth_face" } },
  }));
  part("eye_l", box({
    parent: "head",
    at: [-0.405, 0.1, -0.08],
    size: [0.035, 0.1, 0.13],
    material: "mouth",
  }));
  part("eye_r", box({
    parent: "head",
    at: [0.405, 0.1, -0.08],
    size: [0.035, 0.1, 0.13],
    material: "mouth",
  }));

  part("dorsal_fin", box({
    parent: "body",
    at: [0, 0.5, 0.08],
    rot: [-10, 0, 0],
    size: [0.1, 0.58, 0.62],
    material: "skin_dark",
  }));
  part("fin_l", box({
    parent: "body",
    at: [-0.58, -0.12, -0.34],
    rot: [0, 16, -10],
    size: [0.7, 0.09, 0.44],
    material: "skin_dark",
    joint: { pivot: [0.35, 0, -0.12], axis: [0, 0, 1] },
  }));
  part("fin_r", box({
    parent: "body",
    at: [0.58, -0.12, -0.34],
    rot: [0, -16, 10],
    size: [0.7, 0.09, 0.44],
    material: "skin_dark",
    joint: { pivot: [-0.35, 0, -0.12], axis: [0, 0, 1] },
  }));
  part("rear_dorsal", box({
    parent: "body",
    at: [0, 0.38, 0.65],
    rot: [-12, 0, 0],
    size: [0.07, 0.24, 0.28],
    material: "skin_dark",
  }));

  part("tail", box({
    parent: "body",
    at: [0, 0, 1.02],
    size: [0.3, 0.3, 0.62],
    material: "skin",
    joint: { pivot: [0, 0, -0.31], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0, 0.46],
    size: [0.14, 0.32, 0.34],
    material: "skin_dark",
    joint: { pivot: [0, 0, -0.17], axis: [0, 1, 0] },
  }));
  part("tail_upper", box({
    parent: "tail_tip",
    at: [0, 0.38, 0.14],
    rot: [-8, 0, 0],
    size: [0.09, 0.64, 0.44],
    material: "skin_dark",
    faces: {
      east: { texture: "tail_pattern" },
      west: { texture: "tail_pattern" },
    },
  }));
  part("tail_lower", box({
    parent: "tail_tip",
    at: [0, -0.33, 0.12],
    rot: [8, 0, 0],
    size: [0.09, 0.5, 0.38],
    material: "skin_dark",
    faces: {
      east: { texture: "tail_pattern" },
      west: { texture: "tail_pattern" },
    },
  }));

  swim("swim", {
    fps: 18,
    duration: 1.18,
    cycleDistance: 1.7,
    loop: true,
    samples: 17,
    body: "body",
    bodyBob: 0.012,
    bodySwayDegrees: 2.8,
    finSwingDegrees: 4,
    leftFin: "fin_l",
    rightFin: "fin_r",
    tail: "tail",
    tailSwingDegrees: 15,
    tailTip: "tail_tip",
    tailTipPhase: 0.11,
    tailTipSwingDegrees: 22,
  });
});
