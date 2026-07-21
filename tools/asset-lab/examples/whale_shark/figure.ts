import { figure } from "../../src/dsl";

// A box-only whale shark with broad spotted head, ridged back, pale belly,
// sweeping fins, crescent tail, slow cruise, and filter-feeding action.
export default figure("whale_shark", ({ asciiTexture, box, clip, defaultClip, mat, part, swim }) => {
  mat("blue", "#4b6771"); mat("blue_light", "#718b91"); mat("blue_dark", "#2e4a55"); mat("belly", "#d1d3c7"); mat("mouth", "#44353a"); mat("white", "#dbe0d2");
  asciiTexture("spots", { palette: { ".": "#4b6771", "l": "#718b91", "w": "#dbe0d2", "d": "#2e4a55" }, pixels: ["d..w..w..w.d", ".w..w..w..w.", "..ww..ww..ww", "w..w..w..w..", "dddddddddddd"] });
  asciiTexture("face", { palette: { ".": "#718b91", "w": "#dbe0d2", "d": "#2e4a55" }, pixels: ["d.w....w.d", "w...ww...w", "..........", "..ww..ww..", "dddddddddd"] });
  part("body", box({ at: [0, 1, 0.08], size: [1.02, 0.74, 1.9], material: "blue", faces: { east: { texture: "spots" }, west: { texture: "spots" }, up: { texture: "spots" } } }));
  part("belly", box({ parent: "body", at: [0, -0.42, -0.06], size: [0.84, 0.14, 1.42], material: "belly" }));
  part("shoulder", box({ parent: "body", at: [0, 0.02, -0.88], size: [1.08, 0.66, 0.46], material: "blue_light" }));
  part("head", box({ parent: "shoulder", at: [0, -0.02, -0.44], size: [1.16, 0.56, 0.5], material: "blue_light", faces: { north: { texture: "face" } } }));
  part("mouth", box({ parent: "head", at: [0, -0.17, -0.34], size: [0.82, 0.18, 0.2], material: "mouth", joint: { pivot: [0, 0.07, 0.08], axis: [1, 0, 0] } }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_${side}`, box({ parent: "head", at: [sign * 0.58, 0.08, -0.08], size: [0.04, 0.1, 0.13], material: "blue_dark" }));
    part(`fin_${side}`, box({ parent: "body", at: [sign * 0.72, -0.12, -0.38], rot: [0, sign * -15, sign * 9], size: [0.9, 0.1, 0.52], material: "blue_dark", joint: { pivot: [sign * -0.42, 0, -0.14], axis: [0, 0, 1] } }));
  }
  part("ridge_1", box({ parent: "body", at: [-0.27, 0.45, 0.1], size: [0.08, 0.16, 1.2], material: "blue_light" }));
  part("ridge_2", box({ parent: "body", at: [0.27, 0.45, 0.1], size: [0.08, 0.16, 1.2], material: "blue_light" }));
  part("dorsal", box({ parent: "body", at: [0, 0.58, 0.2], rot: [-10, 0, 0], size: [0.12, 0.62, 0.66], material: "blue_dark" }));
  part("tail", box({ parent: "body", at: [0, 0, 1.2], size: [0.36, 0.34, 0.7], material: "blue", joint: { pivot: [0, 0, -0.33], axis: [0, 1, 0] } }));
  part("tail_tip", box({ parent: "tail", at: [0, 0, 0.54], size: [0.16, 0.3, 0.38], material: "blue_dark", joint: { pivot: [0, 0, -0.18], axis: [0, 1, 0] } }));
  part("tail_upper", box({ parent: "tail_tip", at: [0, 0.48, 0.14], rot: [-8, 0, 0], size: [0.1, 0.82, 0.5], material: "blue_dark" }));
  part("tail_lower", box({ parent: "tail_tip", at: [0, -0.4, 0.12], rot: [8, 0, 0], size: [0.1, 0.66, 0.45], material: "blue_dark" }));
  swim("plankton_cruise", { label: "Plankton cruise", fps: 18, duration: 1.42, cycleDistance: 1.35, loop: true, samples: 23, body: "body", bodyBob: 0.014, bodySwayDegrees: 2, finSwingDegrees: 4, leftFin: "fin_l", rightFin: "fin_r", tail: "tail", tailSwingDegrees: 12, tailTip: "tail_tip", tailTipPhase: 0.12, tailTipSwingDegrees: 18 });
  clip("filter_feed", { label: "Filter feed", role: "action", nextClip: "plankton_cruise", fps: 30, loop: false, keys: [
    ["mouth", 0, { rot: [0, 0, 0], scale: [1, 1, 1] }], ["mouth", 0.28, { rot: [-18, 0, 0], scale: [1.08, 1.3, 1.12] }], ["mouth", 0.7, { rot: [-24, 0, 0], scale: [1.14, 1.5, 1.18] }], ["mouth", 1.18, { rot: [0, 0, 0], scale: [1, 1, 1] }],
    ["fin_l", 0, { rot: [0, 0, 0] }], ["fin_l", 0.7, { rot: [0, 0, -8] }], ["fin_l", 1.18, { rot: [0, 0, 0] }], ["fin_r", 0, { rot: [0, 0, 0] }], ["fin_r", 0.7, { rot: [0, 0, 8] }], ["fin_r", 1.18, { rot: [0, 0, 0] }],
  ] });
  defaultClip("plankton_cruise");
});
