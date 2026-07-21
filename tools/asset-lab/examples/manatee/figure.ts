import { figure } from "../../src/dsl";

// A box-only West Indian manatee with a massive stepped gray body, blunt
// whiskered muzzle, tiny eyes, paired paddle flippers, and one broad horizontal
// spoon tail driven by a slow vertical swim cycle.
export default figure("manatee", ({
  mat,
  asciiTexture,
  part,
  box,
  swim,
}) => {
  mat("skin", "#777d78");
  mat("skin_light", "#939a94");
  mat("skin_dark", "#555b57");
  mat("belly", "#aeb0a5");
  mat("muzzle", "#89877d");
  mat("eye", "#181a18");

  asciiTexture("face", {
    palette: { ".": "#939a94", "e": "#181a18", "d": "#555b57", "l": "#aeb0a5" },
    pixels: [
      "dd......dd",
      "d.e....e.d",
      "..e....e..",
      "...llll...",
      "..llllll..",
      "..........",
    ],
  });
  asciiTexture("muzzle_face", {
    palette: { ".": "#89877d", "n": "#424440", "w": "#d0cec0" },
    pixels: [
      "w........w",
      ".w.nn..w..",
      "..nnnnnn..",
      ".w.nn..w..",
      "w........w",
    ],
  });
  asciiTexture("body_mottle", {
    palette: { ".": "#777d78", "l": "#939a94", "d": "#555b57", "b": "#aeb0a5" },
    pixels: [
      "dd..............dd",
      "d..ll...l..ll....d",
      "..l...ll......l...",
      ".....l....ll......",
      "...ll........ll...",
      "..bbbbbbbbbbbbbb..",
      "bbbbbbbbbbbbbbbbbb",
    ],
  });
  asciiTexture("spoon_tail", {
    palette: { ".": "#777d78", "l": "#939a94", "d": "#555b57" },
    pixels: [
      "dddddddddddddd",
      "dlllllllllllld",
      "dl..........ld",
      "d............d",
      "dd..........dd",
      "dddddddddddddd",
    ],
  });

  part("body", box({
    at: [0, 0.88, 0.12],
    size: [1.12, 0.84, 1.76],
    material: "skin",
    faces: {
      east: { texture: "body_mottle" },
      west: { texture: "body_mottle" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.48, -0.06],
    size: [0.92, 0.15, 1.36],
    material: "belly",
  }));
  part("shoulders", box({
    parent: "body",
    at: [0, 0.04, -0.74],
    size: [1.18, 0.78, 0.58],
    material: "skin_light",
  }));
  part("rump", box({
    parent: "body",
    at: [0, -0.03, 0.74],
    size: [0.94, 0.7, 0.54],
    material: "skin_dark",
  }));
  part("head", box({
    parent: "shoulders",
    at: [0, -0.02, -0.5],
    size: [0.92, 0.66, 0.52],
    material: "skin_light",
    faces: { north: { texture: "face" } },
  }));
  part("muzzle", box({
    parent: "head",
    at: [0, -0.15, -0.38],
    size: [0.68, 0.34, 0.3],
    material: "muzzle",
    faces: { north: { texture: "muzzle_face" } },
  }));
  part("lower_lip", box({
    parent: "muzzle",
    at: [0, -0.21, 0.02],
    size: [0.48, 0.12, 0.24],
    material: "skin_dark",
  }));
  for (const [side, x, yaw, roll] of [
    ["l", -0.67, 10, -7],
    ["r", 0.67, -10, 7],
  ] as const) {
    part(`flipper_${side}`, box({
      parent: "body",
      at: [x, -0.22, -0.42],
      rot: [0, yaw, roll],
      size: [0.68, 0.13, 0.38],
      material: "skin_dark",
      joint: { pivot: [side === "l" ? 0.31 : -0.31, 0, -0.07], axis: [0, 0, 1] },
    }));
  }
  part("tail", box({
    parent: "rump",
    at: [0, 0, 0.52],
    size: [0.48, 0.4, 0.62],
    material: "skin_dark",
    joint: { pivot: [0, 0, -0.28], axis: [1, 0, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0, 0.5],
    size: [1.3, 0.12, 0.62],
    material: "skin",
    faces: {
      up: { texture: "spoon_tail" },
      down: { texture: "spoon_tail" },
    },
    joint: { pivot: [0, 0, -0.29], axis: [1, 0, 0] },
  }));

  swim("glide", {
    fps: 18,
    duration: 1.86,
    cycleDistance: 0.86,
    loop: true,
    samples: 23,
    body: "body",
    bodyBob: 0.026,
    bodySwayDegrees: 1.1,
    finAxis: "z",
    finPhase: 0.22,
    finSwingDegrees: 9,
    leftFin: "flipper_l",
    rightFin: "flipper_r",
    tail: "tail",
    tailAxis: "x",
    tailSwingDegrees: 9,
    tailTip: "tail_tip",
    tailTipPhase: 0.13,
    tailTipSwingDegrees: 14,
  });
});
