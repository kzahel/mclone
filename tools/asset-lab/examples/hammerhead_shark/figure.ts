import { figure } from "../../src/dsl";

// A box-only scalloped hammerhead with broad cephalofoil, end-set eyes, gills,
// swept fins, vertical caudal tail, cruising swim, and head-sweep action.
export default figure("hammerhead_shark", ({ asciiTexture, box, clip, defaultClip, mat, part, swim }) => {
  mat("gray", "#60757c"); mat("gray_light", "#829399"); mat("gray_dark", "#344d55"); mat("belly", "#d0d3ca"); mat("black", "#172024"); mat("mouth", "#342426");
  asciiTexture("body", { palette: { ".": "#60757c", "l": "#829399", "d": "#344d55", "b": "#d0d3ca" }, pixels: ["dddddddddddd", "dll......lld", "l..........l", "............", "bbbbbbbbbbbb"] });
  asciiTexture("gills", { palette: { ".": "#60757c", "g": "#24383f", "l": "#829399" }, pixels: ["llllllll", "........", ".g.g.g..", ".g.g.g..", "........"] });
  part("body", box({ at: [0, 0.94, 0.08], size: [0.8, 0.62, 1.58], material: "gray", faces: { east: { texture: "body" }, west: { texture: "body" } } }));
  part("belly", box({ parent: "body", at: [0, -0.35, -0.05], size: [0.66, 0.13, 1.16], material: "belly" }));
  part("shoulder", box({ parent: "body", at: [0, 0.02, -0.76], size: [0.84, 0.56, 0.42], material: "gray_light", faces: { east: { texture: "gills" }, west: { texture: "gills" } } }));
  part("neck", box({ parent: "shoulder", at: [0, 0, -0.34], size: [0.64, 0.42, 0.32], material: "gray_light", joint: { pivot: [0, 0, 0.14], axis: [0, 1, 0] } }));
  part("hammer", box({ parent: "neck", at: [0, 0.03, -0.3], size: [1.5, 0.36, 0.38], material: "gray_light" }));
  part("snout", box({ parent: "hammer", at: [0, -0.02, -0.27], size: [0.64, 0.28, 0.22], material: "gray_light" }));
  part("mouth", box({ parent: "snout", at: [0, -0.13, -0.13], size: [0.48, 0.08, 0.16], material: "mouth" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`hammer_tip_${side}`, box({ parent: "hammer", at: [sign * 0.79, 0, 0.05], size: [0.18, 0.4, 0.42], material: "gray_dark" }));
    part(`eye_${side}`, box({ parent: `hammer_tip_${side}`, at: [sign * 0.1, 0.07, -0.1], size: [0.05, 0.12, 0.15], material: "black" }));
    part(`fin_${side}`, box({ parent: "body", at: [sign * 0.58, -0.12, -0.32], rot: [0, sign * -16, sign * 10], size: [0.72, 0.09, 0.46], material: "gray_dark", joint: { pivot: [sign * -0.34, 0, -0.12], axis: [0, 0, 1] } }));
  }
  part("dorsal", box({ parent: "body", at: [0, 0.5, 0.06], rot: [-10, 0, 0], size: [0.11, 0.62, 0.62], material: "gray_dark" }));
  part("rear_dorsal", box({ parent: "body", at: [0, 0.37, 0.66], size: [0.07, 0.25, 0.28], material: "gray_dark" }));
  part("tail", box({ parent: "body", at: [0, 0, 1.02], size: [0.3, 0.3, 0.62], material: "gray", joint: { pivot: [0, 0, -0.29], axis: [0, 1, 0] } }));
  part("tail_tip", box({ parent: "tail", at: [0, 0, 0.46], size: [0.14, 0.26, 0.34], material: "gray_dark", joint: { pivot: [0, 0, -0.16], axis: [0, 1, 0] } }));
  part("tail_upper", box({ parent: "tail_tip", at: [0, 0.39, 0.13], rot: [-8, 0, 0], size: [0.09, 0.66, 0.44], material: "gray_dark" }));
  part("tail_lower", box({ parent: "tail_tip", at: [0, -0.34, 0.12], rot: [8, 0, 0], size: [0.09, 0.52, 0.4], material: "gray_dark" }));
  swim("hammerhead_cruise", { label: "Hammerhead cruise", fps: 19, duration: 1.16, cycleDistance: 1.55, loop: true, samples: 21, body: "body", bodyBob: 0.012, bodySwayDegrees: 2.5, finSwingDegrees: 4, leftFin: "fin_l", rightFin: "fin_r", tail: "tail", tailSwingDegrees: 15, tailTip: "tail_tip", tailTipPhase: 0.11, tailTipSwingDegrees: 22 });
  clip("sensor_sweep", { label: "Sensor sweep", role: "action", nextClip: "hammerhead_cruise", fps: 30, loop: false, keys: [
    ["neck", 0, { rot: [0, 0, 0] }], ["neck", 0.22, { rot: [0, -24, 0] }], ["neck", 0.48, { rot: [0, 28, 0] }], ["neck", 0.72, { rot: [0, -18, 0] }], ["neck", 1.02, { rot: [0, 0, 0] }],
    ["tail", 0, { rot: [0, 0, 0] }], ["tail", 0.22, { rot: [0, 10, 0] }], ["tail", 0.48, { rot: [0, -12, 0] }], ["tail", 1.02, { rot: [0, 0, 0] }],
  ] });
  defaultClip("hammerhead_cruise");
});
