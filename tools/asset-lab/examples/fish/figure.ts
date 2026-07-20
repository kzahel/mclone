import { figure } from "../../src/dsl";

// A box-only tropical fish with a compact blue body, yellow fins, scale bands,
// and a two-stage vertical tail. Fine markings stay in side and face textures
// while the sparse fin slabs carry the swimming silhouette.
export default figure("fish", ({
  mat,
  asciiTexture,
  part,
  box,
  swim,
}) => {
  mat("blue", "#2878b8");
  mat("blue_light", "#48a7cf");
  mat("blue_dark", "#173d72");
  mat("yellow", "#efc43a");
  mat("yellow_dark", "#b88520");
  mat("belly", "#b8dedf");

  asciiTexture("face", {
    palette: { ".": "#48a7cf", "e": "#101820", "m": "#173d72", "y": "#efc43a" },
    pixels: [
      "........",
      ".e....e.",
      ".e....e.",
      "........",
      "..mmmm..",
      "...mm...",
      "..yyyy..",
      "........",
    ],
  });
  asciiTexture("scale_bands", {
    palette: { ".": "#2878b8", "l": "#48a7cf", "d": "#173d72", "y": "#efc43a" },
    pixels: [
      "dd..ll..dd..",
      "d..l..l..d..",
      "..l.yy.l..d.",
      ".l.y..y.l..d",
      "..l.yy.l..d.",
      "d..l..l..d..",
      "dd..ll..dd..",
      "dddddddddddd",
    ],
  });
  asciiTexture("tail_pattern", {
    palette: { "y": "#efc43a", "d": "#b88520", "b": "#173d72" },
    pixels: [
      "yyyyyyyy",
      "yddddddy",
      "ydyyyyyy",
      "ydyyydyy",
      "ydyyyyyy",
      "yddddddy",
      "yyyyyyyy",
      "bbbbbbbb",
    ],
  });

  part("body", box({
    at: [0, 0.78, 0.04],
    size: [0.52, 0.62, 0.94],
    material: "blue",
    faces: {
      east: { texture: "scale_bands" },
      west: { texture: "scale_bands" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.34, -0.02],
    size: [0.42, 0.12, 0.68],
    material: "belly",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.01, -0.59],
    size: [0.5, 0.54, 0.34],
    material: "blue_light",
    faces: { north: { texture: "face" } },
  }));
  part("mouth", box({
    parent: "head",
    at: [0, -0.12, -0.21],
    size: [0.22, 0.1, 0.12],
    material: "yellow_dark",
  }));

  part("dorsal_fin", box({
    parent: "body",
    at: [0, 0.43, 0.02],
    rot: [-8, 0, 0],
    size: [0.08, 0.3, 0.48],
    material: "yellow",
  }));
  part("ventral_fin", box({
    parent: "body",
    at: [0, -0.4, 0.06],
    rot: [10, 0, 0],
    size: [0.07, 0.22, 0.34],
    material: "yellow_dark",
  }));
  part("fin_l", box({
    parent: "body",
    at: [-0.34, -0.04, -0.14],
    rot: [0, 8, -8],
    size: [0.34, 0.06, 0.32],
    material: "yellow",
    joint: { pivot: [0.17, 0, -0.1], axis: [0, 0, 1] },
  }));
  part("fin_r", box({
    parent: "body",
    at: [0.34, -0.04, -0.14],
    rot: [0, -8, 8],
    size: [0.34, 0.06, 0.32],
    material: "yellow",
    joint: { pivot: [-0.17, 0, -0.1], axis: [0, 0, 1] },
  }));

  part("tail", box({
    parent: "body",
    at: [0, 0, 0.6],
    size: [0.16, 0.34, 0.34],
    material: "blue_dark",
    joint: { pivot: [0, 0, -0.17], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0, 0.3],
    size: [0.08, 0.74, 0.42],
    material: "yellow",
    faces: {
      east: { texture: "tail_pattern" },
      west: { texture: "tail_pattern" },
    },
    joint: { pivot: [0, 0, -0.21], axis: [0, 1, 0] },
  }));

  swim("swim", {
    fps: 18,
    duration: 0.9,
    cycleDistance: 1.05,
    loop: true,
    samples: 17,
    body: "body",
    bodyBob: 0.018,
    bodySwayDegrees: 3.5,
    finSwingDegrees: 10,
    leftFin: "fin_l",
    rightFin: "fin_r",
    tail: "tail",
    tailSwingDegrees: 19,
    tailTip: "tail_tip",
    tailTipPhase: 0.1,
    tailTipSwingDegrees: 27,
  });
});
