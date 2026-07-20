import { figure } from "../../src/dsl";

// A box-only bottlenose dolphin with a stepped forehead and rostrum, pale
// belly, swept pectoral fins, dorsal fin, and horizontal flukes. It uses the
// swim macro on the x axis for mammalian up/down tail propulsion.
export default figure("dolphin", ({
  mat,
  asciiTexture,
  part,
  box,
  swim,
}) => {
  mat("skin", "#668ca1");
  mat("skin_light", "#82a8b8");
  mat("skin_dark", "#3f667c");
  mat("belly", "#c7d8d9");
  mat("eye", "#14212a");

  asciiTexture("face", {
    palette: { ".": "#82a8b8", "e": "#14212a", "l": "#c7d8d9" },
    pixels: [
      "........",
      ".e....e.",
      "........",
      "..llll..",
      ".llllll.",
      "..llll..",
      "........",
      "........",
    ],
  });
  asciiTexture("body_side", {
    palette: { ".": "#668ca1", "d": "#3f667c", "l": "#c7d8d9" },
    pixels: [
      "dddddddddddddd",
      "d............d",
      "..............",
      "..............",
      "..llllllllll..",
      ".llllllllllll.",
      "llllllllllllll",
    ],
  });
  asciiTexture("fluke", {
    palette: { ".": "#668ca1", "d": "#3f667c", "l": "#82a8b8" },
    pixels: [
      "dddddddddddd",
      "dlllllllllld",
      "d..........d",
      "dd........dd",
      "dddd....dddd",
      "dddddddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.88, 0.08],
    size: [0.68, 0.58, 1.46],
    material: "skin",
    faces: {
      east: { texture: "body_side" },
      west: { texture: "body_side" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.33, -0.05],
    size: [0.52, 0.12, 1.08],
    material: "belly",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.05, -0.88],
    size: [0.64, 0.54, 0.54],
    material: "skin_light",
    faces: { north: { texture: "face" } },
  }));
  part("melon", box({
    parent: "head",
    at: [0, 0.2, -0.27],
    size: [0.5, 0.24, 0.3],
    material: "skin_light",
  }));
  part("rostrum", box({
    parent: "head",
    at: [0, -0.12, -0.46],
    size: [0.28, 0.18, 0.52],
    material: "belly",
  }));
  part("nose", box({
    parent: "rostrum",
    at: [0, 0, -0.29],
    size: [0.22, 0.13, 0.1],
    material: "skin_dark",
  }));
  part("eye_l", box({
    parent: "head",
    at: [-0.325, 0.1, -0.13],
    size: [0.035, 0.09, 0.11],
    material: "eye",
  }));
  part("eye_r", box({
    parent: "head",
    at: [0.325, 0.1, -0.13],
    size: [0.035, 0.09, 0.11],
    material: "eye",
  }));

  part("dorsal_fin", box({
    parent: "body",
    at: [0, 0.43, 0.14],
    rot: [-12, 0, 0],
    size: [0.1, 0.42, 0.52],
    material: "skin_dark",
  }));
  part("fin_l", box({
    parent: "body",
    at: [-0.47, -0.12, -0.28],
    rot: [0, 14, -10],
    size: [0.54, 0.08, 0.38],
    material: "skin_dark",
    joint: { pivot: [0.27, 0, -0.1], axis: [0, 0, 1] },
  }));
  part("fin_r", box({
    parent: "body",
    at: [0.47, -0.12, -0.28],
    rot: [0, -14, 10],
    size: [0.54, 0.08, 0.38],
    material: "skin_dark",
    joint: { pivot: [-0.27, 0, -0.1], axis: [0, 0, 1] },
  }));

  part("tail", box({
    parent: "body",
    at: [0, 0, 0.94],
    size: [0.3, 0.28, 0.58],
    material: "skin",
    joint: { pivot: [0, 0, -0.29], axis: [1, 0, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0, 0.48],
    size: [1.08, 0.08, 0.5],
    material: "skin_dark",
    faces: {
      up: { texture: "fluke" },
      down: { texture: "fluke" },
    },
    joint: { pivot: [0, 0, -0.25], axis: [1, 0, 0] },
  }));

  swim("swim", {
    fps: 18,
    duration: 1.08,
    cycleDistance: 1.8,
    loop: true,
    samples: 17,
    body: "body",
    bodyBob: 0.035,
    bodyBobPhase: 0,
    bodySwayDegrees: 2.5,
    finSwingDegrees: 5,
    leftFin: "fin_l",
    rightFin: "fin_r",
    tail: "tail",
    tailAxis: "x",
    tailSwingDegrees: 14,
    tailTip: "tail_tip",
    tailTipPhase: 0.1,
    tailTipSwingDegrees: 21,
  });
});
