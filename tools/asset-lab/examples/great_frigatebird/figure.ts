import { figure } from "../../src/dsl";

// A box-only male great frigatebird with broad black wings, long hooked bill,
// forked tail, scarlet gular pouch, ocean soar, and inflated display action.
export default figure("great_frigatebird", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, wingFlap }) => {
  mat("black", "#252c2c"); mat("black_light", "#465251"); mat("black_dark", "#151919"); mat("iridescent", "#405f58"); mat("red", "#bd3040"); mat("red_light", "#e05255"); mat("bill", "#a9a28e"); mat("eye", "#d3aa46"); mat("foot", "#6b5d58");
  asciiTexture("wing", { palette: { ".": "#252c2c", "l": "#465251", "d": "#151919", "i": "#405f58" }, pixels: ["llllllllllll", "l..........l", ".d.d.d.d.d..", "..i..i..i...", ".dddddddddd."] });
  asciiTexture("face", { palette: { ".": "#252c2c", "e": "#d3aa46", "p": "#151919", "i": "#405f58" }, pixels: ["ii......ii", "i.ep..pe.i", "..iiiiii..", ".i......i."] });
  asciiTexture("pouch", { palette: { ".": "#bd3040", "l": "#e05255", "d": "#7f2433" }, pixels: ["dd......dd", "dlllllllld", ".llllllll.", "..llllll..", "...dddd..."] });
  part("body", box({ at: [0, 0.9, 0.05], size: [0.56, 0.52, 0.98], material: "black" }));
  part("back", box({ parent: "body", at: [0, 0.25, 0.02], size: [0.5, 0.16, 0.74], material: "iridescent" }));
  part("breast", box({ parent: "body", at: [0, -0.08, -0.46], size: [0.44, 0.42, 0.2], material: "black_light" }));
  part("pouch", box({ parent: "breast", at: [0, -0.01, -0.2], size: [0.4, 0.38, 0.2], material: "red", faces: { north: { texture: "pouch" } } }));
  part("neck", box({ parent: "body", at: [0, 0.28, -0.43], rot: [-16, 0, 0], size: [0.36, 0.42, 0.38], material: "iridescent", joint: { pivot: [0, -0.19, 0.14], axis: [1, 0, 0] } }));
  part("head", box({ parent: "neck", at: [0, 0.28, -0.18], rot: [10, 0, 0], size: [0.46, 0.42, 0.46], material: "black", faces: { north: { texture: "face" } } }));
  part("bill_1", box({ parent: "head", at: [0, -0.05, -0.42], size: [0.26, 0.16, 0.48], material: "bill" }));
  part("bill_tip", box({ parent: "bill_1", at: [0, -0.04, -0.34], rot: [-8, 0, 0], size: [0.17, 0.11, 0.26], material: "bill" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({ parent: "body", at: [sign * 0.66, 0.12, -0.02], rot: [0, sign * -8, 0], size: [0.94, 0.075, 0.62], material: "black", faces: { up: { texture: "wing" }, down: { texture: "wing" } }, joint: { pivot: [sign * -0.43, 0, -0.04], axis: [0, 0, 1] } }));
    part(`wing_mid_${side}`, box({ parent: `wing_${side}`, at: [sign * 0.68, 0, 0.03], rot: [0, sign * -10, 0], size: [0.55, 0.06, 0.72], material: "black_light", faces: { up: { texture: "wing" }, down: { texture: "wing" } }, joint: { pivot: [sign * -0.25, 0, -0.04], axis: [0, 0, 1] } }));
    part(`wing_tip_${side}`, box({ parent: `wing_mid_${side}`, at: [sign * 0.42, 0, 0.1], rot: [0, sign * -14, 0], size: [0.34, 0.05, 0.78], material: "black_dark" }));
    part(`leg_${side}`, box({ parent: "body", at: [sign * 0.12, -0.31, 0.12], rot: [-28, 0, 0], size: [0.07, 0.2, 0.07], material: "foot" }));
  }
  part("tail_l", box({ parent: "body", at: [-0.14, 0.02, 0.68], rot: [8, -8, 0], size: [0.2, 0.07, 0.52], material: "black_dark", joint: { pivot: [0, 0, -0.23], axis: [0, 1, 0] } }));
  part("tail_r", box({ parent: "body", at: [0.14, 0.02, 0.68], rot: [8, 8, 0], size: [0.2, 0.07, 0.52], material: "black_dark", joint: { pivot: [0, 0, -0.23], axis: [0, 1, 0] } }));
  wingFlap("ocean_soar", { label: "Ocean soar", fps: 24, duration: 1.18, cycleDistance: 1.3, loop: true, samples: 27, body: "body", bodyBob: 0.018, degrees: 18, frequency: 1, leftWing: "wing_l", rightWing: "wing_r", tracks: [followThrough("wing_mid_l", { source: "wing_l", sourceChannel: "rot", sourceAxis: "z", axis: "z", degrees: 12, overshoot: 0.38, lag: 0.09 }), followThrough("wing_mid_r", { source: "wing_r", sourceChannel: "rot", sourceAxis: "z", axis: "z", degrees: 12, overshoot: 0.38, lag: 0.09 }), followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.35, lag: 0.1 })] });
  clip("gular_display", { label: "Gular display", role: "action", nextClip: "ocean_soar", fps: 30, loop: false, keys: [
    ["pouch", 0, { scale: [1, 1, 1] }], ["pouch", 0.42, { scale: [1.45, 1.36, 1.5] }], ["pouch", 0.88, { scale: [1.55, 1.48, 1.62] }], ["pouch", 1.34, { scale: [1, 1, 1] }],
    ["neck", 0, { rot: [0, 0, 0] }], ["neck", 0.52, { rot: [12, 0, 0] }], ["neck", 0.88, { rot: [18, 0, 0] }], ["neck", 1.34, { rot: [0, 0, 0] }],
    ["wing_l", 0, { rot: [0, 0, 0] }], ["wing_l", 0.52, { rot: [0, 0, -20] }], ["wing_l", 0.88, { rot: [0, 0, -26] }], ["wing_l", 1.34, { rot: [0, 0, 0] }], ["wing_r", 0, { rot: [0, 0, 0] }], ["wing_r", 0.52, { rot: [0, 0, 20] }], ["wing_r", 0.88, { rot: [0, 0, 26] }], ["wing_r", 1.34, { rot: [0, 0, 0] }],
    ["tail_l", 0, { rot: [0, 0, 0] }], ["tail_l", 0.88, { rot: [0, -14, 0] }], ["tail_l", 1.34, { rot: [0, 0, 0] }], ["tail_r", 0, { rot: [0, 0, 0] }], ["tail_r", 0.88, { rot: [0, 14, 0] }], ["tail_r", 1.34, { rot: [0, 0, 0] }],
  ] });
  defaultClip("ocean_soar");
});
