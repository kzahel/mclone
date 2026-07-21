import { figure } from "../../src/dsl";

// A box-only Japanese macaque with dense winter fur, a bare red face, compact
// tail, planted hand-walk, and a separate full-body snow shake.
export default figure("japanese_macaque", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, quadrupedWalk, swing }) => {
  mat("fur", "#9a9388"); mat("fur_light", "#b9b2a6"); mat("fur_dark", "#625f5b"); mat("skin", "#b96f68"); mat("skin_light", "#d29588"); mat("black", "#242322"); mat("snow", "#e7e7df");
  asciiTexture("winter_fur", { palette: { ".": "#9a9388", "l": "#b9b2a6", "d": "#625f5b", "s": "#e7e7df" }, pixels: ["dd..ll..dd", ".d.llll.d.", "..dd..dd..", "l..s..s..l", "dddddddddd"] });
  asciiTexture("red_face", { palette: { ".": "#b96f68", "l": "#d29588", "e": "#242322", "f": "#b9b2a6" }, pixels: ["ff......ff", "f.ee..ee.f", "..el..le..", "...llll...", "..l....l..", "...llll..."] });
  part("body", box({ at: [0, 0.84, 0.06], size: [0.7, 0.64, 0.82], material: "fur", faces: { east: { texture: "winter_fur" }, west: { texture: "winter_fur" } } }));
  part("rump", box({ parent: "body", at: [0, 0.01, 0.36], size: [0.74, 0.6, 0.42], material: "fur_light" }));
  part("chest", box({ parent: "body", at: [0, 0.02, -0.39], size: [0.62, 0.54, 0.28], material: "fur_light" }));
  part("neck", box({ parent: "chest", at: [0, 0.25, -0.16], rot: [-10, 0, 0], size: [0.48, 0.38, 0.34], material: "fur_dark", joint: { pivot: [0, -0.16, 0.12], axis: [1, 0, 0] } }));
  part("head", box({ parent: "neck", at: [0, 0.28, -0.16], rot: [7, 0, 0], size: [0.6, 0.56, 0.5], material: "fur_light", faces: { north: { texture: "red_face" } } }));
  part("face", box({ parent: "head", at: [0, -0.05, -0.3], size: [0.4, 0.38, 0.16], material: "skin" }));
  part("muzzle", box({ parent: "face", at: [0, -0.13, -0.13], size: [0.3, 0.18, 0.16], material: "skin_light" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({ parent: "head", at: [sign * 0.32, 0.01, 0], size: [0.11, 0.2, 0.12], material: "skin" }));
    part(`upper_arm_${side}`, box({ parent: "chest", at: [sign * 0.31, -0.29, -0.01], rot: [-4, 0, sign * -3], size: [0.19, 0.44, 0.22], material: "fur_dark", joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] } }));
    part(`forearm_${side}`, box({ parent: `upper_arm_${side}`, at: [0, -0.33, -0.03], size: [0.18, 0.34, 0.2], material: "fur", joint: { pivot: [0, 0.16, 0], axis: [1, 0, 0] } }));
    part(`hand_${side}`, box({ parent: `forearm_${side}`, at: [0, -0.2, -0.08], size: [0.25, 0.06, 0.32], material: "skin" }));
    part(`thigh_${side}`, box({ parent: "rump", at: [sign * 0.23, -0.35, 0.03], rot: [-6, 0, 0], size: [0.27, 0.4, 0.3], material: "fur", joint: { pivot: [0, 0.19, 0], axis: [1, 0, 0] } }));
    part(`shin_${side}`, box({ parent: `thigh_${side}`, at: [0, -0.29, -0.02], size: [0.2, 0.32, 0.22], material: "fur_dark", joint: { pivot: [0, 0.15, 0], axis: [1, 0, 0] } }));
    part(`foot_${side}`, box({ parent: `shin_${side}`, at: [0, -0.18, -0.1], size: [0.27, 0.06, 0.38], material: "skin" }));
  }
  part("tail", box({ parent: "rump", at: [0, 0.03, 0.28], rot: [52, 0, 0], size: [0.22, 0.34, 0.22], material: "fur_dark", joint: { pivot: [0, -0.15, 0], axis: [1, 0, 0] } }));
  quadrupedWalk("snow_trudge", { label: "Snow trudge", fps: 20, duration: 1.04, cycleDistance: 0.58, gait: "walk", loop: true, samples: 23, contactParts: { frontLeft: "hand_l", frontRight: "hand_r", backLeft: "foot_l", backRight: "foot_r" }, body: "body", bodyBob: 0.014, bodyBobCenter: 0.016, head: "neck", headSwingDegrees: 3, legs: { frontLeft: "upper_arm_l", frontRight: "upper_arm_r", backLeft: "thigh_l", backRight: "thigh_r" }, stanceRatio: 0.67, swingDegrees: 16, tail: "tail", tailSwingDegrees: 5, tracks: [swing("body", { axis: "z", degrees: 2.2, phase: 0.25 }), followThrough("head", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.1 })] });
  clip("snow_shake", { label: "Snow shake", role: "action", nextClip: "snow_trudge", fps: 30, loop: false, keys: [
    ["body", 0, { rot: [0, 0, 0] }], ["body", 0.22, { rot: [0, -9, 0] }], ["body", 0.38, { rot: [0, 11, 0] }], ["body", 0.54, { rot: [0, -12, 0] }], ["body", 0.7, { rot: [0, 9, 0] }], ["body", 1.02, { rot: [0, 0, 0] }],
    ["neck", 0, { rot: [0, 0, 0] }], ["neck", 0.22, { rot: [0, 13, -6] }], ["neck", 0.38, { rot: [0, -15, 7] }], ["neck", 0.54, { rot: [0, 15, -7] }], ["neck", 0.7, { rot: [0, -11, 5] }], ["neck", 1.02, { rot: [0, 0, 0] }],
    ["upper_arm_l", 0, { rot: [0, 0, 0] }], ["upper_arm_l", 0.38, { rot: [0, 0, -8] }], ["upper_arm_l", 0.7, { rot: [0, 0, 7] }], ["upper_arm_l", 1.02, { rot: [0, 0, 0] }], ["upper_arm_r", 0, { rot: [0, 0, 0] }], ["upper_arm_r", 0.38, { rot: [0, 0, 8] }], ["upper_arm_r", 0.7, { rot: [0, 0, -7] }], ["upper_arm_r", 1.02, { rot: [0, 0, 0] }],
  ] });
  defaultClip("snow_trudge");
});
