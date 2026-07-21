import { figure } from "../../src/dsl";

// A box-only spotted pufferfish with a compact round body, puckered mouth,
// short fins, and small block spines that become prominent when it inflates.
export default figure("pufferfish", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  mat,
  part,
  swim,
}) => {
  mat("olive", "#9e9b43");
  mat("olive_light", "#c5bd61");
  mat("olive_dark", "#69662e");
  mat("belly", "#e1dba2");
  mat("fin", "#827f3b");
  mat("mouth", "#615642");
  mat("eye", "#171914");
  mat("spine", "#d9cf86");

  asciiTexture("spot_scales", {
    palette: { ".": "#9e9b43", "l": "#c5bd61", "d": "#69662e", "s": "#4e5129" },
    pixels: [
      "ll........ll",
      "l..ss..ss..l",
      "s..ss..ss..s",
      ".s...ss...s.",
      "...ss..ss...",
      "..s.....s...",
      "l..ss..ss..l",
      "ll........ll",
    ],
  });
  asciiTexture("face", {
    palette: { ".": "#c5bd61", "e": "#171914", "s": "#69662e", "b": "#e1dba2" },
    pixels: [
      "ssssssss",
      "s.e..e.s",
      "s.e..e.s",
      "s......s",
      "s..bb..s",
      "bbbbbbbb",
    ],
  });
  asciiTexture("tail_spots", {
    palette: { ".": "#827f3b", "d": "#69662e", "l": "#c5bd61" },
    pixels: ["llllllll", "l..dd..l", "l.d..d.l", "l..dd..l", "l.d..d.l", "llllllll"],
  });

  part("body", box({
    at: [0, 0.78, 0],
    size: [0.78, 0.76, 0.84],
    material: "olive",
    faces: {
      east: { texture: "spot_scales" },
      west: { texture: "spot_scales" },
      up: { texture: "spot_scales" },
    },
  }));
  part("belly", box({
    parent: "body",
    at: [0, -0.36, -0.05],
    size: [0.64, 0.13, 0.6],
    material: "belly",
  }));
  part("head", box({
    parent: "body",
    at: [0, 0.01, -0.5],
    size: [0.7, 0.62, 0.3],
    material: "olive_light",
    faces: { north: { texture: "face" } },
  }));
  part("mouth", box({
    parent: "head",
    at: [0, -0.13, -0.2],
    size: [0.22, 0.16, 0.15],
    material: "mouth",
  }));
  part("mouth_tip", box({
    parent: "mouth",
    at: [0, 0, -0.1],
    size: [0.14, 0.12, 0.1],
    material: "belly",
  }));

  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`fin_${side}`, box({
      parent: "body",
      at: [sign * 0.48, -0.04, -0.12],
      rot: [0, sign * -8, sign * 11],
      size: [0.32, 0.07, 0.3],
      material: "fin",
      joint: { pivot: [sign * -0.14, 0, -0.07], axis: [0, 0, 1] },
    }));
  }
  part("dorsal_fin", box({
    parent: "body",
    at: [0, 0.45, 0.06],
    rot: [-8, 0, 0],
    size: [0.08, 0.24, 0.32],
    material: "fin",
  }));
  part("tail", box({
    parent: "body",
    at: [0, 0, 0.5],
    size: [0.16, 0.3, 0.26],
    material: "olive_dark",
    joint: { pivot: [0, 0, -0.12], axis: [0, 1, 0] },
  }));
  part("tail_tip", box({
    parent: "tail",
    at: [0, 0, 0.25],
    size: [0.08, 0.58, 0.38],
    material: "fin",
    faces: { east: { texture: "tail_spots" }, west: { texture: "tail_spots" } },
    joint: { pivot: [0, 0, -0.18], axis: [0, 1, 0] },
  }));

  for (const [name, at, size, rot] of [
    ["spine_top_front", [0, 0.47, -0.23], [0.1, 0.26, 0.1], [-7, 0, 0]],
    ["spine_top_rear", [0, 0.47, 0.25], [0.1, 0.26, 0.1], [7, 0, 0]],
    ["spine_bottom_front", [0, -0.45, -0.22], [0.1, 0.24, 0.1], [7, 0, 0]],
    ["spine_bottom_rear", [0, -0.45, 0.24], [0.1, 0.24, 0.1], [-7, 0, 0]],
    ["spine_left_front", [-0.48, 0.13, -0.2], [0.24, 0.1, 0.1], [0, 0, 8]],
    ["spine_left_rear", [-0.48, 0.06, 0.24], [0.24, 0.1, 0.1], [0, 0, 8]],
    ["spine_right_front", [0.48, 0.13, -0.2], [0.24, 0.1, 0.1], [0, 0, -8]],
    ["spine_right_rear", [0.48, 0.06, 0.24], [0.24, 0.1, 0.1], [0, 0, -8]],
  ] as const) {
    part(name, box({ parent: "body", at, size, rot, material: "spine" }));
  }

  swim("swim", {
    label: "Swim",
    fps: 20,
    duration: 0.9,
    cycleDistance: 0.9,
    loop: true,
    samples: 19,
    body: "body",
    bodyBob: 0.018,
    bodySwayDegrees: 3,
    finSwingDegrees: 12,
    leftFin: "fin_l",
    rightFin: "fin_r",
    tail: "tail",
    tailSwingDegrees: 18,
    tailTip: "tail_tip",
    tailTipPhase: 0.1,
    tailTipSwingDegrees: 26,
  });
  clip("inflate", {
    label: "Inflate",
    role: "action",
    fps: 24,
    loop: false,
    keys: [
      ["body", 0, { scale: [1, 1, 1] }],
      ["body", 0.12, { scale: [0.96, 0.96, 1.04] }],
      ["body", 0.32, { scale: [1.22, 1.18, 1.12] }],
      ["body", 0.55, { scale: [1.48, 1.42, 1.22] }],
      ["body", 0.72, { scale: [1.42, 1.37, 1.2] }],
      ["body", 0.86, { scale: [1.45, 1.4, 1.21] }],
    ],
  });
  clip("deflate", {
    label: "Deflate",
    role: "action",
    nextClip: "swim",
    fps: 24,
    loop: false,
    keys: [
      ["body", 0, { scale: [1.45, 1.4, 1.21] }],
      ["body", 0.14, { scale: [1.48, 1.42, 1.22] }],
      ["body", 0.42, { scale: [1.14, 1.12, 1.08] }],
      ["body", 0.62, { scale: [0.96, 0.96, 1.04] }],
      ["body", 0.78, { scale: [1, 1, 1] }],
    ],
  });
  defaultClip("swim");
});
