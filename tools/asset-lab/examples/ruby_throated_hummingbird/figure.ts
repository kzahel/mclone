import { figure } from "../../src/dsl";

// A box-only ruby-throated hummingbird with emerald back, iridescent gorget,
// needle bill, tiny feet, rapid hover, and a separate flower-feeding dip.
export default figure("ruby_throated_hummingbird", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, wingFlap }) => {
  mat("green", "#3f7a55"); mat("green_light", "#65a16e"); mat("green_dark", "#254b3d"); mat("white", "#ddd9c8"); mat("ruby", "#b52e4a"); mat("wing", "#58665e"); mat("black", "#1b211f"); mat("foot", "#67554b");
  asciiTexture("gorget", { palette: { ".": "#b52e4a", "l": "#df5367", "d": "#76263d", "w": "#ddd9c8" }, pixels: ["dd....dd", "dlllllld", ".llllll.", "..dddd..", "wwwwwwww"] });
  asciiTexture("wing", { palette: { ".": "#58665e", "l": "#7b887d", "d": "#35463f" }, pixels: ["llllllll", "l......l", ".d.d.d..", "..d.d.d.", "dddddddd"] });
  part("body", box({ at: [0, 0.78, 0.08], size: [0.48, 0.48, 0.66], material: "green" }));
  part("breast", box({ parent: "body", at: [0, -0.05, -0.39], size: [0.34, 0.34, 0.15], material: "white" }));
  part("throat", box({ parent: "body", at: [0, 0.17, -0.34], size: [0.32, 0.28, 0.2], material: "ruby", faces: { north: { texture: "gorget" } } }));
  part("head", box({ parent: "throat", at: [0, 0.22, -0.15], size: [0.4, 0.38, 0.4], material: "green_light", joint: { pivot: [0, -0.15, 0.09], axis: [1, 0, 0] } }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) part(`eye_${side}`, box({ parent: "head", at: [sign * 0.205, 0.06, -0.08], size: [0.04, 0.1, 0.12], material: "black" }));
  part("bill_1", box({ parent: "head", at: [0, -0.02, -0.38], size: [0.11, 0.1, 0.46], material: "black" }));
  part("bill_tip", box({ parent: "bill_1", at: [0, 0, -0.34], size: [0.06, 0.06, 0.3], material: "black" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({ parent: "body", at: [sign * 0.42, 0.09, 0.02], size: [0.64, 0.055, 0.54], material: "wing", faces: { up: { texture: "wing" }, down: { texture: "wing" } }, joint: { pivot: [sign * -0.3, 0, -0.08], axis: [0, 0, 1] } }));
    part(`wing_tip_${side}`, box({ parent: `wing_${side}`, at: [sign * 0.44, 0, 0.04], rot: [0, sign * -7, 0], size: [0.32, 0.045, 0.42], material: "green_dark" }));
    part(`leg_${side}`, box({ parent: "body", at: [sign * 0.1, -0.28, 0.03], rot: [-30, 0, 0], size: [0.06, 0.16, 0.06], material: "foot" }));
  }
  part("tail_center", box({ parent: "body", at: [0, 0.02, 0.48], rot: [8, 0, 0], size: [0.14, 0.05, 0.5], material: "green_dark" }));
  part("tail_l", box({ parent: "body", at: [-0.11, 0.01, 0.46], rot: [7, -8, 0], size: [0.12, 0.05, 0.44], material: "green_dark" }));
  part("tail_r", box({ parent: "body", at: [0.11, 0.01, 0.46], rot: [7, 8, 0], size: [0.12, 0.05, 0.44], material: "green_dark" }));
  wingFlap("ruby_hover", { label: "Ruby hover", fps: 30, duration: 0.56, cycleDistance: 0.16, loop: true, samples: 25, body: "body", bodyBob: 0.025, degrees: 54, frequency: 3, leftWing: "wing_l", rightWing: "wing_r", tracks: [followThrough("head", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.4, lag: 0.06 }), followThrough("tail_center", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.55, lag: 0.09 })] });
  clip("flower_feed", { label: "Flower feed", role: "action", nextClip: "ruby_hover", fps: 30, loop: false, keys: [
    ["body", 0, { rot: [0, 0, 0] }], ["body", 0.3, { rot: [-10, 0, 0] }], ["body", 0.64, { rot: [-16, 0, 0] }], ["body", 1.02, { rot: [0, 0, 0] }],
    ["head", 0, { rot: [0, 0, 0] }], ["head", 0.3, { rot: [-12, 0, 0] }], ["head", 0.64, { rot: [-20, 0, 0] }], ["head", 1.02, { rot: [0, 0, 0] }],
    ["wing_l", 0, { rot: [0, 0, 0] }], ["wing_l", 0.22, { rot: [0, 0, -36] }], ["wing_l", 0.42, { rot: [0, 0, 38] }], ["wing_l", 0.64, { rot: [0, 0, -32] }], ["wing_l", 1.02, { rot: [0, 0, 0] }], ["wing_r", 0, { rot: [0, 0, 0] }], ["wing_r", 0.22, { rot: [0, 0, 36] }], ["wing_r", 0.42, { rot: [0, 0, -38] }], ["wing_r", 0.64, { rot: [0, 0, 32] }], ["wing_r", 1.02, { rot: [0, 0, 0] }],
  ] });
  defaultClip("ruby_hover");
});
