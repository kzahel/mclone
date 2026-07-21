import { figure } from "../../src/dsl";

// A box-only adult male narwhal with mottled gray back, pale belly, long
// spiral tusk, compact flippers, horizontal flukes, swim, and tusk spar.
export default figure("narwhal", ({ asciiTexture, box, clip, defaultClip, mat, part, swim }) => {
  mat("gray", "#7d8583"); mat("gray_light", "#a8aba4"); mat("gray_dark", "#515a5b"); mat("white", "#dddcd1"); mat("tusk", "#e5dcc7"); mat("tusk_dark", "#a79c8a"); mat("eye", "#171918");
  asciiTexture("mottle", { palette: { ".": "#7d8583", "l": "#a8aba4", "d": "#515a5b", "w": "#dddcd1" }, pixels: ["d..l.d..l.d.", ".d..l.d..l.d", "..dd..ll..dd", "wwwwwwwwwwww"] });
  asciiTexture("spiral", { palette: { ".": "#e5dcc7", "d": "#a79c8a" }, pixels: ["dddd....", "....dddd", "dddd....", "....dddd"] });
  part("body", box({ at: [0, 0.96, 0.08], size: [0.86, 0.66, 1.5], material: "gray", faces: { east: { texture: "mottle" }, west: { texture: "mottle" } } }));
  part("belly", box({ parent: "body", at: [0, -0.38, -0.04], size: [0.68, 0.14, 1.08], material: "white" }));
  part("head", box({ parent: "body", at: [0, 0, -0.86], size: [0.78, 0.58, 0.48], material: "gray_light" }));
  part("snout", box({ parent: "head", at: [0, -0.03, -0.3], size: [0.62, 0.4, 0.18], material: "gray_light" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_${side}`, box({ parent: "head", at: [sign * 0.4, 0.1, -0.08], size: [0.04, 0.1, 0.12], material: "eye" }));
    part(`flipper_${side}`, box({ parent: "body", at: [sign * 0.55, -0.12, -0.28], rot: [0, sign * -16, sign * 12], size: [0.58, 0.08, 0.4], material: "gray_dark", joint: { pivot: [sign * -0.26, 0, -0.1], axis: [0, 0, 1] } }));
  }
  part("tusk_1", box({ parent: "snout", at: [0, 0.02, -0.38], size: [0.18, 0.18, 0.62], material: "tusk", faces: { east: { texture: "spiral" }, west: { texture: "spiral" } } }));
  part("tusk_2", box({ parent: "tusk_1", at: [0, 0, -0.52], size: [0.12, 0.12, 0.5], material: "tusk_dark", faces: { east: { texture: "spiral" }, west: { texture: "spiral" } } }));
  part("tusk_tip", box({ parent: "tusk_2", at: [0, 0, -0.38], size: [0.06, 0.06, 0.32], material: "tusk" }));
  part("tail", box({ parent: "body", at: [0, 0, 0.98], size: [0.34, 0.3, 0.52], material: "gray_dark", joint: { pivot: [0, 0, -0.24], axis: [1, 0, 0] } }));
  part("fluke_l", box({ parent: "tail", at: [-0.36, 0, 0.36], rot: [0, -12, 0], size: [0.62, 0.08, 0.38], material: "gray_dark" }));
  part("fluke_r", box({ parent: "tail", at: [0.36, 0, 0.36], rot: [0, 12, 0], size: [0.62, 0.08, 0.38], material: "gray_dark" }));
  swim("arctic_swim", { label: "Arctic swim", fps: 20, duration: 1.16, cycleDistance: 1.2, loop: true, samples: 23, body: "body", bodyBob: 0.014, bodySwayDegrees: 2, finSwingDegrees: 5, leftFin: "flipper_l", rightFin: "flipper_r", tail: "tail", tailAxis: "x", tailSwingDegrees: 12 });
  clip("tusk_spar", { label: "Tusk spar", role: "action", nextClip: "arctic_swim", fps: 30, loop: false, keys: [
    ["body", 0, { rot: [0, 0, 0] }], ["body", 0.25, { rot: [0, -22, 0] }], ["body", 0.42, { rot: [0, 30, 0] }], ["body", 0.62, { rot: [0, -18, 0] }], ["body", 0.9, { rot: [0, 0, 0] }],
    ["tail", 0, { rot: [0, 0, 0] }], ["tail", 0.25, { rot: [0, 12, 0] }], ["tail", 0.42, { rot: [0, -18, 0] }], ["tail", 0.9, { rot: [0, 0, 0] }],
  ] });
  defaultClip("arctic_swim");
});
