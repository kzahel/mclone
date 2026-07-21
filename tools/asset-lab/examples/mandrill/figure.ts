import { figure } from "../../src/dsl";

// A box-only adult male mandrill with olive mantle, gold beard, vivid blue and
// red muzzle, colorful rump, knuckle walk, and a separate threat-yawn action.
export default figure("mandrill", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, quadrupedWalk }) => {
  mat("fur", "#565341"); mat("fur_dark", "#2d302b"); mat("fur_light", "#77735b"); mat("skin", "#3a3531"); mat("blue", "#548c9e"); mat("red", "#b44c45"); mat("beard", "#d1a85a"); mat("tooth", "#e5ddc4"); mat("rump", "#a36d85"); mat("eye", "#d49d40");
  asciiTexture("face", { palette: { ".": "#3a3531", "b": "#548c9e", "r": "#b44c45", "e": "#d49d40" }, pixels: ["bb....bb", "b.e..e.b", "bbrrrrbb", "b.rrrr.b", "bbrrrrbb", "........"] });
  asciiTexture("mantle", { palette: { ".": "#565341", "l": "#77735b", "d": "#2d302b" }, pixels: ["ll......ll", ".ll....ll.", "..dddddd..", ".d......d.", "ll......ll", ".........."] });
  part("torso", box({ at: [0, 1.02, 0.08], size: [0.82, 0.84, 0.68], material: "fur", faces: { north: { texture: "mantle" } } }));
  part("shoulders", box({ parent: "torso", at: [0, 0.25, -0.13], size: [1.08, 0.36, 0.68], material: "fur_light" }));
  part("rump", box({ parent: "torso", at: [0, -0.1, 0.43], size: [0.62, 0.45, 0.28], material: "rump" }));
  part("neck", box({ parent: "torso", at: [0, 0.46, -0.2], rot: [-9, 0, 0], size: [0.56, 0.34, 0.42], material: "fur_dark", joint: { pivot: [0, -0.13, 0.12], axis: [1, 0, 0] } }));
  part("head", box({ parent: "neck", at: [0, 0.27, -0.18], size: [0.64, 0.54, 0.5], material: "skin", faces: { north: { texture: "face" } } }));
  part("crown", box({ parent: "head", at: [0, 0.27, 0.08], size: [0.5, 0.17, 0.4], material: "fur_dark" }));
  part("muzzle", box({ parent: "head", at: [0, -0.12, -0.38], size: [0.4, 0.29, 0.32], material: "red" }));
  part("beard", box({ parent: "head", at: [0, -0.35, -0.13], size: [0.5, 0.3, 0.32], material: "beard" }));
  part("jaw", box({ parent: "muzzle", at: [0, -0.19, -0.03], size: [0.38, 0.13, 0.3], material: "skin", joint: { pivot: [0, 0.05, 0.1], axis: [1, 0, 0] } }));
  part("teeth", box({ parent: "jaw", at: [0, 0.06, -0.16], size: [0.3, 0.07, 0.04], material: "tooth" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({ parent: "head", at: [sign * 0.35, 0.04, 0], size: [0.14, 0.2, 0.11], material: "skin" }));
    part(`arm_${side}`, box({ parent: "shoulders", at: [sign * 0.45, -0.43, -0.04], size: [0.27, 0.68, 0.31], material: "fur_dark", joint: { pivot: [0, 0.32, 0], axis: [1, 0, 0] } }));
    part(`forearm_${side}`, box({ parent: `arm_${side}`, at: [0, -0.49, -0.07], size: [0.31, 0.58, 0.34], material: "fur" }));
    part(`knuckle_${side}`, box({ parent: `forearm_${side}`, at: [0, -0.36, -0.09], size: [0.34, 0.17, 0.36], material: "skin" }));
    part(`leg_${side}`, box({ parent: "torso", at: [sign * 0.23, -0.57, 0.2], rot: [-7, 0, 0], size: [0.3, 0.52, 0.34], material: "fur_dark", joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] } }));
    part(`foot_${side}`, box({ parent: `leg_${side}`, at: [0, -0.32, -0.1], size: [0.34, 0.17, 0.43], material: "skin" }));
  }
  quadrupedWalk("savanna_knuckle_walk", { label: "Savanna knuckle walk", fps: 18, duration: 1.08, cycleDistance: 0.7, gait: "walk", loop: true, samples: 21, contactParts: { frontLeft: "knuckle_l", frontRight: "knuckle_r", backLeft: "foot_l", backRight: "foot_r" }, body: "torso", bodyBob: 0.014, bodyBobCenter: 0.016, head: "head", headSwingDegrees: 3, legs: { frontLeft: "arm_l", frontRight: "arm_r", backLeft: "leg_l", backRight: "leg_r" }, stanceRatio: 0.68, swingDegrees: 14, tracks: [followThrough("neck", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.11 })] });
  clip("threat_yawn", { label: "Threat yawn", role: "action", nextClip: "savanna_knuckle_walk", fps: 30, loop: false, keys: [
    ["neck", 0, { rot: [0, 0, 0] }], ["neck", 0.3, { rot: [-12, 0, 0] }], ["neck", 0.62, { rot: [-16, 0, 0] }], ["neck", 1.12, { rot: [0, 0, 0] }],
    ["jaw", 0, { rot: [0, 0, 0] }], ["jaw", 0.25, { rot: [0, 0, 0] }], ["jaw", 0.42, { rot: [38, 0, 0] }], ["jaw", 0.78, { rot: [42, 0, 0] }], ["jaw", 1.12, { rot: [0, 0, 0] }],
  ] });
  defaultClip("savanna_knuckle_walk");
});
