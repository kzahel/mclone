import { figure } from "../../src/dsl";

// A box-only adult male proboscis monkey with russet mantle, pale pot belly,
// long pendant nose, long tail, canopy hand-walk, and a nasal-call action.
export default figure("proboscis_monkey", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, quadrupedWalk }) => {
  mat("russet", "#9b5c3f"); mat("russet_light", "#bf7956"); mat("russet_dark", "#633e31"); mat("cream", "#d2b493"); mat("skin", "#b17968"); mat("nose", "#c08272"); mat("hand", "#4c3730"); mat("eye", "#171412");
  asciiTexture("face", { palette: { ".": "#b17968", "r": "#633e31", "e": "#171412", "n": "#c08272" }, pixels: ["rr....rr", "r.e..e.r", "..nnnn..", ".nnnnnn.", "..nnnn..", "........"] });
  part("body", box({ at: [0, 0.88, 0.06], size: [0.68, 0.72, 0.94], material: "russet" }));
  part("pot_belly", box({ parent: "body", at: [0, -0.22, -0.4], size: [0.58, 0.5, 0.2], material: "cream" }));
  part("shoulders", box({ parent: "body", at: [0, 0.22, -0.32], size: [0.82, 0.42, 0.46], material: "russet_light" }));
  part("neck", box({ parent: "body", at: [0, 0.38, -0.46], rot: [-10, 0, 0], size: [0.44, 0.36, 0.38], material: "russet_dark", joint: { pivot: [0, -0.15, 0.12], axis: [1, 0, 0] } }));
  part("head", box({ parent: "neck", at: [0, 0.25, -0.2], size: [0.54, 0.5, 0.48], material: "skin", faces: { north: { texture: "face" } } }));
  part("cap", box({ parent: "head", at: [0, 0.25, 0.04], size: [0.5, 0.16, 0.38], material: "russet_dark" }));
  part("nose_1", box({ parent: "head", at: [0, -0.03, -0.36], size: [0.25, 0.34, 0.25], material: "nose" }));
  part("nose_tip", box({ parent: "nose_1", at: [0, -0.2, -0.04], rot: [14, 0, 0], size: [0.22, 0.2, 0.22], material: "nose" }));
  part("muzzle", box({ parent: "head", at: [0, -0.22, -0.29], size: [0.36, 0.2, 0.22], material: "cream" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({ parent: "head", at: [sign * 0.3, 0.02, 0.03], size: [0.13, 0.2, 0.11], material: "skin" }));
    part(`arm_${side}`, box({ parent: "shoulders", at: [sign * 0.37, -0.38, -0.02], size: [0.2, 0.58, 0.23], material: "russet_dark", joint: { pivot: [0, 0.27, 0], axis: [1, 0, 0] } }));
    part(`forearm_${side}`, box({ parent: `arm_${side}`, at: [0, -0.42, -0.05], size: [0.22, 0.45, 0.25], material: "russet" }));
    part(`hand_${side}`, box({ parent: `forearm_${side}`, at: [0, -0.27, -0.08], size: [0.25, 0.12, 0.31], material: "hand" }));
    part(`leg_${side}`, box({ parent: "body", at: [sign * 0.23, -0.46, 0.29], rot: [-7, 0, 0], size: [0.28, 0.46, 0.32], material: "russet_dark", joint: { pivot: [0, 0.22, 0], axis: [1, 0, 0] } }));
    part(`foot_${side}`, box({ parent: `leg_${side}`, at: [0, -0.29, -0.1], size: [0.3, 0.14, 0.4], material: "hand" }));
  }
  part("tail_1", box({ parent: "body", at: [0, 0.12, 0.6], rot: [32, 0, 0], size: [0.2, 0.62, 0.2], material: "russet_dark", joint: { pivot: [0, -0.28, 0], axis: [1, 0, 0] } }));
  for (const [index, height, width, rot] of [[2, 0.58, 0.18, 10], [3, 0.54, 0.16, 13], [4, 0.46, 0.13, 16], [5, 0.36, 0.1, 18]] as const) part(`tail_${index}`, box({ parent: `tail_${index - 1}`, at: [0, height * 0.46 + 0.27, 0.03], rot: [rot, 0, 0], size: [width, height, width], material: index > 3 ? "russet_light" : "russet_dark", joint: { pivot: [0, -height * 0.44, 0], axis: [1, 0, 0] } }));
  quadrupedWalk("canopy_hand_walk", { label: "Canopy hand-walk", fps: 19, duration: 1.02, cycleDistance: 0.64, gait: "walk", loop: true, samples: 21, contactParts: { frontLeft: "hand_l", frontRight: "hand_r", backLeft: "foot_l", backRight: "foot_r" }, body: "body", bodyBob: 0.012, bodyBobCenter: 0.014, head: "head", headSwingDegrees: 3, legs: { frontLeft: "arm_l", frontRight: "arm_r", backLeft: "leg_l", backRight: "leg_r" }, stanceRatio: 0.66, swingDegrees: 16, tail: "tail_1", tailSwingDegrees: 7, tracks: [followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 9, overshoot: 0.55, lag: 0.12 })] });
  clip("nasal_call", { label: "Nasal call", role: "action", nextClip: "canopy_hand_walk", fps: 30, loop: false, keys: [
    ["neck", 0, { rot: [0, 0, 0] }], ["neck", 0.28, { rot: [-14, 0, 0] }], ["neck", 0.68, { rot: [-18, 0, 0] }], ["neck", 1.12, { rot: [0, 0, 0] }],
    ["nose_1", 0, { scale: [1, 1, 1] }], ["nose_1", 0.28, { scale: [1.08, 1.12, 1.08] }], ["nose_1", 0.68, { scale: [1.13, 1.18, 1.1] }], ["nose_1", 1.12, { scale: [1, 1, 1] }],
  ] });
  defaultClip("canopy_hand_walk");
});
