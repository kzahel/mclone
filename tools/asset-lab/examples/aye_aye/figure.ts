import { figure } from "../../src/dsl";

// A box-only aye-aye with huge ears, pale mask, amber eyes, elongated probing
// fingers, a plume tail, careful branch creep, and a separate tapping action.
export default figure("aye_aye", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, quadrupedWalk }) => {
  mat("fur", "#373331"); mat("fur_light", "#514a45"); mat("fur_dark", "#211f1e");
  mat("cream", "#c7b69d"); mat("pink", "#a9786d"); mat("eye", "#d3aa43"); mat("pupil", "#171412"); mat("hand", "#66504a");
  asciiTexture("face", { palette: { ".": "#c7b69d", "d": "#514a45", "e": "#d3aa43", "p": "#171412" }, pixels: ["dd....dd", ".ep..pe.", ".ep..pe.", "..dddd..", "........"] });
  asciiTexture("coat", { palette: { ".": "#373331", "l": "#514a45", "d": "#211f1e" }, pixels: ["d.ld.ld.ld.l.", ".d.ld.ld.ld.l", "l.dl.dl.dl.d.", "............."] });

  part("body", box({ at: [0, 0.78, 0.05], size: [0.62, 0.52, 1.0], material: "fur", faces: { east: { texture: "coat" }, west: { texture: "coat" } } }));
  part("belly", box({ parent: "body", at: [0, -0.3, -0.02], size: [0.44, 0.11, 0.7], material: "fur_dark" }));
  part("shoulders", box({ parent: "body", at: [0, 0.08, -0.45], size: [0.66, 0.5, 0.34], material: "fur_light" }));
  part("neck", box({ parent: "body", at: [0, 0.12, -0.58], size: [0.36, 0.38, 0.3], material: "fur_dark", joint: { pivot: [0, 0, 0.12], axis: [1, 0, 0] } }));
  part("head", box({ parent: "neck", at: [0, 0.08, -0.3], size: [0.54, 0.5, 0.46], material: "cream", faces: { north: { texture: "face" } } }));
  part("muzzle", box({ parent: "head", at: [0, -0.13, -0.32], size: [0.3, 0.2, 0.2], material: "fur_light" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({ parent: "head", at: [sign * 0.3, 0.24, 0.05], rot: [0, sign * -8, sign * 8], size: [0.3, 0.38, 0.1], material: "pink", joint: { pivot: [sign * -0.12, -0.14, 0], axis: [1, 0, 0] } }));
  }
  for (const [suffix, x, z, front] of [["fl", -0.21, -0.32, true], ["fr", 0.21, -0.32, true], ["bl", -0.22, 0.35, false], ["br", 0.22, 0.35, false]] as const) {
    part(`${front ? "arm" : "leg"}_${suffix}`, box({ parent: "body", at: [x, -0.42, z], size: [0.13, front ? 0.46 : 0.38, 0.15], material: "fur_dark", joint: { pivot: [0, front ? 0.22 : 0.18, 0], axis: [1, 0, 0] } }));
    part(`${front ? "hand" : "foot"}_${suffix}`, box({ parent: `${front ? "arm" : "leg"}_${suffix}`, at: [0, front ? -0.28 : -0.24, -0.05], size: [0.2, 0.1, 0.26], material: "hand" }));
    if (front) part(`finger_${suffix}`, box({ parent: `hand_${suffix}`, at: [0, 0.06, -0.2], size: [0.035, 0.035, 0.28], material: "pink", joint: { pivot: [0, 0, 0.12], axis: [1, 0, 0] } }));
  }
  part("tail_1", box({ parent: "body", at: [0, 0.25, 0.58], rot: [-42, 0, 0], size: [0.36, 0.54, 0.36], material: "fur_dark", joint: { pivot: [0, -0.24, 0], axis: [0, 0, 1] } }));
  for (const [index, width, height, material] of [[2, 0.4, 0.48, "fur_light"], [3, 0.38, 0.44, "fur"], [4, 0.32, 0.38, "fur_dark"]] as const) {
    part(`tail_${index}`, box({ parent: `tail_${index - 1}`, at: [0, index === 2 ? 0.42 : index === 3 ? 0.38 : 0.33, 0.02], rot: [-7, 0, 0], size: [width, height, width], material, joint: { pivot: [0, -height * 0.44, 0], axis: [0, 0, 1] } }));
  }

  quadrupedWalk("branch_creep", { label: "Branch creep", fps: 18, duration: 1.18, cycleDistance: 0.5, gait: "walk", loop: true, samples: 21,
    contactParts: { frontLeft: "hand_fl", frontRight: "hand_fr", backLeft: "foot_bl", backRight: "foot_br" }, body: "body", bodyBob: 0.012, bodyBobCenter: 0.014, head: "head", headSwingDegrees: 2.5,
    legs: { frontLeft: "arm_fl", frontRight: "arm_fr", backLeft: "leg_bl", backRight: "leg_br" }, stanceRatio: 0.7, swingDegrees: 15, tail: "tail_1", tailSwingDegrees: 7,
    tracks: [followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.13 }), followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.6, lag: 0.13 }), followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "z", axis: "z", degrees: 11, overshoot: 0.7, lag: 0.16 })] });
  clip("tap_probe", { label: "Tap probe", role: "action", nextClip: "branch_creep", fps: 30, loop: false, keys: [
    ["arm_fl", 0, { rot: [0, 0, 0] }], ["arm_fl", 0.25, { rot: [42, 0, 0] }], ["arm_fl", 0.48, { rot: [34, 0, 0] }], ["arm_fl", 0.7, { rot: [42, 0, 0] }], ["arm_fl", 1.02, { rot: [0, 0, 0] }],
    ["finger_fl", 0, { rot: [0, 0, 0] }], ["finger_fl", 0.25, { rot: [18, 0, 0] }], ["finger_fl", 0.38, { rot: [-20, 0, 0] }], ["finger_fl", 0.5, { rot: [14, 0, 0] }], ["finger_fl", 0.62, { rot: [-18, 0, 0] }], ["finger_fl", 1.02, { rot: [0, 0, 0] }],
    ["head", 0, { rot: [0, 0, 0] }], ["head", 0.45, { rot: [10, -8, 0] }], ["head", 1.02, { rot: [0, 0, 0] }],
  ] });
  defaultClip("branch_creep");
});
