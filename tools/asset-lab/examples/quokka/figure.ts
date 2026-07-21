import { figure } from "../../src/dsl";

// A box-only quokka with round ears, compact haunches, smiling muzzle, sturdy
// paws, tapered tail, gentle island walk, and a separate curious-reach action.
export default figure("quokka", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, quadrupedWalk }) => {
  mat("brown", "#80664f"); mat("brown_light", "#a28768"); mat("brown_dark", "#4d4036"); mat("cream", "#d7c3a2"); mat("skin", "#9c7762"); mat("eye", "#171512"); mat("nose", "#302724");
  asciiTexture("face", { palette: { ".": "#a28768", "e": "#171512", "c": "#d7c3a2", "n": "#302724" }, pixels: ["ee....ee", "e......e", "..cccc..", ".ccnncc.", "..c..c..", "...cc..."] });
  part("body", box({ at: [0, 0.69, 0.08], size: [0.68, 0.62, 0.96], material: "brown" }));
  part("belly", box({ parent: "body", at: [0, -0.28, -0.22], size: [0.52, 0.18, 0.58], material: "cream" }));
  part("haunch", box({ parent: "body", at: [0, -0.04, 0.42], size: [0.8, 0.6, 0.48], material: "brown_dark" }));
  part("neck", box({ parent: "body", at: [0, 0.27, -0.45], rot: [-8, 0, 0], size: [0.48, 0.42, 0.4], material: "brown_light", joint: { pivot: [0, -0.18, 0.14], axis: [1, 0, 0] } }));
  part("head", box({ parent: "neck", at: [0, 0.17, -0.3], size: [0.6, 0.5, 0.5], material: "brown_light", faces: { north: { texture: "face" } } }));
  part("muzzle", box({ parent: "head", at: [0, -0.13, -0.34], size: [0.4, 0.25, 0.24], material: "cream" }));
  part("nose", box({ parent: "muzzle", at: [0, 0.04, -0.15], size: [0.18, 0.12, 0.08], material: "nose" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({ parent: "head", at: [sign * 0.22, 0.3, 0.05], size: [0.2, 0.24, 0.14], material: "brown_dark", joint: { pivot: [0, -0.1, 0], axis: [1, 0, 0] } }));
    part(`ear_inner_${side}`, box({ parent: `ear_${side}`, at: [0, 0, -0.085], size: [0.1, 0.14, 0.03], material: "skin" }));
  }
  for (const [suffix, x, z, h] of [["fl", -0.22, -0.3, 0.3], ["fr", 0.22, -0.3, 0.3], ["bl", -0.26, 0.34, 0.36], ["br", 0.26, 0.34, 0.36]] as const) {
    part(`leg_${suffix}`, box({ parent: "body", at: [x, -0.39, z], size: [0.17, h, 0.19], material: suffix.startsWith("b") ? "brown_dark" : "brown_light", joint: { pivot: [0, h * 0.45, 0], axis: [1, 0, 0] } }));
    part(`paw_${suffix}`, box({ parent: `leg_${suffix}`, at: [0, -h * 0.58, -0.07], size: [0.24, 0.11, 0.32], material: "brown_dark" }));
  }
  part("tail_1", box({ parent: "haunch", at: [0, -0.1, 0.42], rot: [-18, 0, 0], size: [0.32, 0.3, 0.6], material: "brown_dark", joint: { pivot: [0, 0, -0.26], axis: [0, 1, 0] } }));
  part("tail_2", box({ parent: "tail_1", at: [0, 0.02, 0.46], size: [0.24, 0.22, 0.44], material: "brown" }));
  part("tail_tip", box({ parent: "tail_2", at: [0, 0, 0.34], size: [0.14, 0.14, 0.3], material: "brown_dark" }));
  quadrupedWalk("island_walk", { label: "Island walk", fps: 19, duration: 1.05, cycleDistance: 0.58, gait: "walk", loop: true, samples: 21, contactParts: { frontLeft: "paw_fl", frontRight: "paw_fr", backLeft: "paw_bl", backRight: "paw_br" }, body: "body", bodyBob: 0.012, bodyBobCenter: 0.014, head: "head", headSwingDegrees: 3, legs: { frontLeft: "leg_fl", frontRight: "leg_fr", backLeft: "leg_bl", backRight: "leg_br" }, stanceRatio: 0.7, swingDegrees: 16, tail: "tail_1", tailSwingDegrees: 6, tracks: [followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.45, lag: 0.11 }), followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.45, lag: 0.11 })] });
  clip("curious_reach", { label: "Curious reach", role: "action", nextClip: "island_walk", fps: 30, loop: false, keys: [
    ["neck", 0, { rot: [0, 0, 0] }], ["neck", 0.35, { rot: [-16, 0, 0] }], ["neck", 0.7, { rot: [-12, 12, 0] }], ["neck", 1.12, { rot: [0, 0, 0] }],
    ["leg_fl", 0, { rot: [0, 0, 0] }], ["leg_fl", 0.35, { rot: [-35, 0, 0] }], ["leg_fl", 0.7, { rot: [-48, 0, 0] }], ["leg_fl", 1.12, { rot: [0, 0, 0] }],
  ] });
  defaultClip("island_walk");
});
