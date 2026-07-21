import { figure } from "../../src/dsl";

// A box-only swordfish with cobalt back, silver flanks, long rostrum, swept
// fins, crescent tail, fast swim, and a separate lateral bill-slash action.
export default figure("swordfish", ({ asciiTexture, box, clip, defaultClip, mat, part, swim }) => {
  mat("blue", "#315c78"); mat("blue_light", "#527e94"); mat("blue_dark", "#1f3d52"); mat("silver", "#c5c8be"); mat("bill", "#6d8190"); mat("eye", "#111718");
  asciiTexture("flank", { palette: { ".": "#315c78", "l": "#527e94", "s": "#c5c8be", "d": "#1f3d52" }, pixels: ["dddddddddddddd", "dlllllllllllld", "..............", "ssssssssssssss", "ssssssssssssss"] });
  asciiTexture("fin", { palette: { ".": "#1f3d52", "l": "#527e94" }, pixels: ["llllllll", "l......l", ".l....l.", "..l..l..", "........"] });
  part("body", box({ at: [0, 0.95, 0.05], size: [0.62, 0.5, 1.5], material: "blue", faces: { east: { texture: "flank" }, west: { texture: "flank" } } }));
  part("belly", box({ parent: "body", at: [0, -0.3, -0.04], size: [0.48, 0.12, 1.08], material: "silver" }));
  part("shoulder", box({ parent: "body", at: [0, 0.02, -0.7], size: [0.66, 0.48, 0.4], material: "blue_light" }));
  part("head", box({ parent: "shoulder", at: [0, 0, -0.36], size: [0.58, 0.42, 0.4], material: "blue_light" }));
  part("bill_base", box({ parent: "head", at: [0, 0.03, -0.42], size: [0.22, 0.18, 0.52], material: "bill" }));
  part("bill_mid", box({ parent: "bill_base", at: [0, 0, -0.46], size: [0.15, 0.12, 0.48], material: "bill" }));
  part("bill_tip", box({ parent: "bill_mid", at: [0, 0, -0.38], size: [0.08, 0.07, 0.34], material: "blue_dark" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_${side}`, box({ parent: "head", at: [sign * 0.3, 0.1, -0.08], size: [0.04, 0.1, 0.12], material: "eye" }));
    part(`fin_${side}`, box({ parent: "body", at: [sign * 0.46, -0.08, -0.28], rot: [0, sign * -18, sign * 12], size: [0.55, 0.07, 0.36], material: "blue_dark", joint: { pivot: [sign * -0.25, 0, -0.1], axis: [0, 0, 1] } }));
  }
  part("dorsal_fin", box({ parent: "body", at: [0, 0.42, -0.3], rot: [-12, 0, 0], size: [0.08, 0.52, 0.56], material: "blue_dark", faces: { east: { texture: "fin" }, west: { texture: "fin" } } }));
  part("tail", box({ parent: "body", at: [0, 0, 0.96], size: [0.24, 0.24, 0.54], material: "blue", joint: { pivot: [0, 0, -0.25], axis: [0, 1, 0] } }));
  part("tail_tip", box({ parent: "tail", at: [0, 0, 0.42], size: [0.12, 0.2, 0.32], material: "blue_dark", joint: { pivot: [0, 0, -0.14], axis: [0, 1, 0] } }));
  part("tail_upper", box({ parent: "tail_tip", at: [0, 0.34, 0.12], rot: [-10, 0, 0], size: [0.07, 0.58, 0.36], material: "blue_dark", faces: { east: { texture: "fin" }, west: { texture: "fin" } } }));
  part("tail_lower", box({ parent: "tail_tip", at: [0, -0.31, 0.1], rot: [10, 0, 0], size: [0.07, 0.5, 0.34], material: "blue_dark", faces: { east: { texture: "fin" }, west: { texture: "fin" } } }));
  swim("speed_swim", { label: "Speed swim", fps: 22, duration: 0.86, cycleDistance: 1.8, loop: true, samples: 21, body: "body", bodyBob: 0.01, bodySwayDegrees: 2.4, finSwingDegrees: 4, leftFin: "fin_l", rightFin: "fin_r", tail: "tail", tailSwingDegrees: 13, tailTip: "tail_tip", tailTipPhase: 0.11, tailTipSwingDegrees: 20 });
  clip("bill_slash", { label: "Bill slash", role: "action", nextClip: "speed_swim", fps: 30, loop: false, keys: [
    ["body", 0, { rot: [0, 0, 0] }], ["body", 0.18, { rot: [0, -24, 0] }], ["body", 0.32, { rot: [0, 38, 0] }], ["body", 0.48, { rot: [0, -30, 0] }], ["body", 0.72, { rot: [0, 0, 0] }],
    ["tail", 0, { rot: [0, 0, 0] }], ["tail", 0.18, { rot: [0, 18, 0] }], ["tail", 0.32, { rot: [0, -26, 0] }], ["tail", 0.48, { rot: [0, 20, 0] }], ["tail", 0.72, { rot: [0, 0, 0] }],
  ] });
  defaultClip("speed_swim");
});
