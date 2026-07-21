import { figure } from "../../src/dsl";

// A box-only adult orca with a long black body, bright eye and saddle patches,
// white belly, blunt head, tall dorsal fin, broad pectorals, and horizontal
// flukes driven by vertical mammalian tail propulsion.
export default figure("orca", ({
  mat,
  asciiTexture,
  part,
  box,
  swim,
}) => {
  mat("black", "#171a1b");
  mat("black_light", "#2a3032");
  mat("white", "#f0eee5");
  mat("white_shadow", "#c9ced0");
  mat("gray", "#7f8b8d");

  asciiTexture("face", {
    palette: { "b": "#171a1b", "w": "#f0eee5", "g": "#7f8b8d" },
    pixels: [
      "bbbbbbbbbb",
      "bwwbbbbwwb",
      "bwwbbbbwwb",
      "bbbbbbbbbb",
      "bbggggggbb",
      "bbbggggbbb",
    ],
  });
  asciiTexture("body_side", {
    palette: { "b": "#171a1b", "l": "#2a3032", "w": "#f0eee5", "g": "#7f8b8d" },
    pixels: [
      "bbbbbbbbbbbbbbbbbb",
      "bbblllllllllllbbbb",
      "bbbbbbbbbbbbbbbbbb",
      "bbwwbbbbbbbbgggbbb",
      "bbbwwwbbbbbbggbbbb",
      "bbwwwwbbbbbbbbbbbb",
      "wwwwwwwwwwwwwwwwww",
      "wwwwwwwwwwwwwwwwww",
    ],
  });
  asciiTexture("pectoral", {
    palette: { "b": "#171a1b", "l": "#2a3032", "w": "#f0eee5" },
    pixels: [
      "bbbbbbbbbbbb",
      "bllllllllllb",
      "bllllllllllb",
      "bbllllllllbb",
      "bbbwwwwwwbbb",
      "bbbbbbbbbbbb",
    ],
  });
  asciiTexture("fluke", {
    palette: { "b": "#171a1b", "l": "#2a3032", "w": "#f0eee5" },
    pixels: [
      "bbbbbbbbbbbb",
      "bllllllllllb",
      "bbllllllllbb",
      "bbbllllllbbb",
      "bbbbwwwwbbbb",
      "bbbbbbbbbbbb",
    ],
  });

  part("body", box({
    at: [0, 0.98, 0.08],
    size: [0.96, 0.78, 1.92],
    material: "black",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.44, -0.08],
    size: [1, 0.16, 1.54],
    material: "white",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.03, -0.83],
    size: [1.02, 0.74, 0.52],
    material: "black_light",
  }));
  part("head", box({
    parent: "shoulders",
    at: [0, -0.02, -0.46],
    size: [0.94, 0.68, 0.52],
    material: "black",
    faces: { north: { texture: "face" } },
  }));
  part("snout", box({
    parent: "head",
    at: [0, -0.08, -0.34],
    size: [0.76, 0.45, 0.22],
    material: "black_light",
  }));
  part("chin", box({
    parent: "head",
    at: [0, -0.34, -0.13],
    size: [0.62, 0.12, 0.4],
    material: "white",
  }));
  for (const [side, x] of [["l", -0.48], ["r", 0.48]] as const) {
    part(`eye_patch_${side}`, box({
      parent: "head",
      at: [x, 0.1, -0.11],
      size: [0.04, 0.18, 0.27],
      material: "white",
    }));
    part(`saddle_patch_${side}`, box({
      parent: "body",
      at: [side === "l" ? -0.5 : 0.5, 0.13, 0.49],
      size: [0.04, 0.24, 0.34],
      material: "gray",
    }));
  }
  part("dorsal_fin", box({
    parent: "body",
    at: [0, 0.68, 0.1],
    rot: [-8, 0, 0],
    size: [0.14, 0.92, 0.68],
    material: "black",
  }));
  for (const [side, x, yaw, roll] of [
    ["l", -0.63, -24, 0],
    ["r", 0.63, 24, 0],
  ] as const) {
    part(`fin_${side}`, box({
      parent: "body",
      at: [x, -0.08, -0.24],
      rot: [0, yaw, roll],
      size: [0.78, 0.08, 0.44],
      material: "black",
      faces: {
        up: { texture: "pectoral" },
        down: { texture: "pectoral" },
      },
      joint: { pivot: [side === "l" ? 0.38 : -0.38, 0, -0.1], axis: [0, 0, 1] },
    }));
  }
  part("tail", box({
    parent: "body",
    at: [0, 0, 1.22],
    size: [0.38, 0.34, 0.72],
    material: "black",
    joint: { pivot: [0, 0, -0.34], axis: [1, 0, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0, 0.58],
    size: [1.34, 0.1, 0.58],
    material: "black_light",
    faces: {
      up: { texture: "fluke" },
      down: { texture: "fluke" },
    },
    joint: { pivot: [0, 0, -0.27], axis: [1, 0, 0] },
  }));

  swim("cruise", {
    fps: 18,
    duration: 1.32,
    cycleDistance: 2.1,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.032,
    bodySwayDegrees: 1.6,
    finAxis: "z",
    finPhase: 0.18,
    finSwingDegrees: 4,
    leftFin: "fin_l",
    rightFin: "fin_r",
    tail: "tail",
    tailAxis: "x",
    tailSwingDegrees: 13,
    tailTip: "tail_tip",
    tailTipPhase: 0.11,
    tailTipSwingDegrees: 20,
  });
});
