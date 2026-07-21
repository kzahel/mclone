import { figure } from "../../src/dsl";

// A box-only great blue heron with slate wings, folded S-neck, dagger bill,
// black crest, long planted toes, patient stalk, and a spear-strike action.
export default figure("great_blue_heron", ({ asciiTexture, bipedWalk, box, clip, defaultClip, followThrough, mat, part, swing }) => {
  mat("slate", "#71818a"); mat("slate_light", "#a6b0af"); mat("slate_dark", "#43515a"); mat("white", "#d9d8cd"); mat("black", "#23282b"); mat("bill", "#d9aa4e"); mat("leg", "#8b826b"); mat("eye", "#e1b739");
  asciiTexture("wing", { palette: { ".": "#71818a", "l": "#a6b0af", "d": "#43515a" }, pixels: ["llllllllll", "l........l", "..dddddd..", ".d......d.", "dddddddddd", ".........."] });
  asciiTexture("face", { palette: { ".": "#d9d8cd", "b": "#23282b", "e": "#e1b739" }, pixels: ["bbbbbbbb", "b.e..e.b", "b......b", "........", "........"] });
  part("body", box({ at: [0, 1.7, 0.12], size: [0.7, 0.68, 1.06], material: "slate" }));
  part("breast", box({ parent: "body", at: [0, -0.05, -0.58], size: [0.52, 0.56, 0.2], material: "slate_light" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) part(`wing_${side}`, box({ parent: "body", at: [sign * 0.39, 0.03, 0.05], size: [0.1, 0.54, 0.82], material: "slate_dark", faces: { [side === "l" ? "west" : "east"]: { texture: "wing" } }, joint: { pivot: [sign * -0.04, 0.2, -0.25], axis: [0, 0, 1] } }));
  part("neck_low", box({ parent: "body", at: [0, 0.42, -0.45], rot: [-35, 0, 0], size: [0.28, 0.72, 0.3], material: "slate_light", joint: { pivot: [0, -0.32, 0.09], axis: [1, 0, 0] } }));
  part("neck_mid", box({ parent: "neck_low", at: [0, 0.44, -0.16], rot: [62, 0, 0], size: [0.25, 0.72, 0.26], material: "white", joint: { pivot: [0, -0.32, 0], axis: [1, 0, 0] } }));
  part("neck_high", box({ parent: "neck_mid", at: [0, 0.47, -0.08], rot: [-25, 0, 0], size: [0.22, 0.58, 0.24], material: "white" }));
  part("head", box({ parent: "neck_high", at: [0, 0.38, -0.14], size: [0.42, 0.38, 0.48], material: "white", faces: { north: { texture: "face" } } }));
  part("cap", box({ parent: "head", at: [0, 0.2, 0.04], size: [0.4, 0.12, 0.36], material: "black" }));
  part("crest", box({ parent: "cap", at: [0, 0.03, 0.34], rot: [38, 0, 0], size: [0.08, 0.08, 0.46], material: "black" }));
  part("bill_1", box({ parent: "head", at: [0, -0.05, -0.4], size: [0.32, 0.18, 0.42], material: "bill" }));
  part("bill_2", box({ parent: "bill_1", at: [0, 0, -0.34], size: [0.22, 0.13, 0.34], material: "bill" }));
  part("bill_tip", box({ parent: "bill_2", at: [0, 0, -0.24], size: [0.1, 0.08, 0.18], material: "black" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`leg_${side}`, box({ parent: "body", at: [sign * 0.18, -0.75, 0.08], size: [0.11, 0.9, 0.12], material: "leg", joint: { pivot: [0, 0.43, 0], axis: [1, 0, 0] } }));
    part(`shin_${side}`, box({ parent: `leg_${side}`, at: [0, -0.68, 0.04], size: [0.09, 0.52, 0.1], material: "leg" }));
    part(`foot_${side}`, box({ parent: `shin_${side}`, at: [0, -0.3, -0.18], size: [0.28, 0.08, 0.62], material: "slate_dark" }));
  }
  part("tail", box({ parent: "body", at: [0, 0.05, 0.66], rot: [18, 0, 0], size: [0.48, 0.3, 0.28], material: "slate_dark" }));
  bipedWalk("marsh_stalk", { label: "Marsh stalk", fps: 18, duration: 1.6, cycleDistance: 0.62, loop: true, samples: 25, body: "body", bodyBob: 0.01, bodyBobCenter: 0.012, head: "head", headSwingDegrees: 1.5, leftLeg: "leg_l", rightLeg: "leg_r", leftContact: "foot_l", rightContact: "foot_r", stanceRatio: 0.74, swingDegrees: 14, tracks: [swing("wing_l", { axis: "z", degrees: 2.5, center: -1, frequency: 2 }), swing("wing_r", { axis: "z", degrees: -2.5, center: 1, frequency: 2 }), followThrough("neck_low", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 3, overshoot: 0.35, lag: 0.12 })] });
  clip("spear_strike", { label: "Spear strike", role: "action", nextClip: "marsh_stalk", fps: 30, loop: false, keys: [
    ["neck_low", 0, { rot: [0, 0, 0] }], ["neck_low", 0.2, { rot: [18, 0, 0] }], ["neck_low", 0.34, { rot: [-42, 0, 0] }], ["neck_low", 0.54, { rot: [-20, 0, 0] }], ["neck_low", 0.94, { rot: [0, 0, 0] }],
    ["neck_mid", 0, { rot: [0, 0, 0] }], ["neck_mid", 0.2, { rot: [-25, 0, 0] }], ["neck_mid", 0.34, { rot: [42, 0, 0] }], ["neck_mid", 0.54, { rot: [18, 0, 0] }], ["neck_mid", 0.94, { rot: [0, 0, 0] }],
  ] });
  defaultClip("marsh_stalk");
});
