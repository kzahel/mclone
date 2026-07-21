import { figure } from "../../src/dsl";

// A box-only kinkajou with golden coat, huge dark eyes, rounded ears, grasping
// paws, long prehensile tail, canopy prowl, and a separate tail-hook action.
export default figure("kinkajou", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, quadrupedWalk }) => {
  mat("gold", "#b88245"); mat("gold_light", "#d2a467"); mat("gold_dark", "#76502f"); mat("cream", "#ead4a8"); mat("eye", "#191511"); mat("nose", "#3a2c25"); mat("paw", "#5b4132");
  asciiTexture("face", { palette: { ".": "#d2a467", "e": "#191511", "c": "#ead4a8", "n": "#3a2c25" }, pixels: ["ee....ee", "eee..eee", "e......e", "..cccc..", ".ccnncc.", "........"] });
  part("body", box({ at: [0, 0.7, 0.08], size: [0.64, 0.5, 1.06], material: "gold" }));
  part("belly", box({ parent: "body", at: [0, -0.28, -0.04], size: [0.5, 0.11, 0.76], material: "cream" }));
  part("neck", box({ parent: "body", at: [0, 0.14, -0.56], size: [0.46, 0.4, 0.3], material: "gold_dark", joint: { pivot: [0, -0.12, 0.12], axis: [1, 0, 0] } }));
  part("head", box({ parent: "neck", at: [0, 0.12, -0.3], size: [0.56, 0.5, 0.48], material: "gold_light", faces: { north: { texture: "face" } } }));
  part("muzzle", box({ parent: "head", at: [0, -0.13, -0.33], size: [0.34, 0.22, 0.22], material: "cream" }));
  part("nose", box({ parent: "muzzle", at: [0, 0.03, -0.15], size: [0.18, 0.12, 0.08], material: "nose" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) part(`ear_${side}`, box({ parent: "head", at: [sign * 0.23, 0.28, 0.04], size: [0.18, 0.22, 0.13], material: "gold_dark", joint: { pivot: [0, -0.09, 0], axis: [1, 0, 0] } }));
  for (const [suffix, x, z] of [["fl", -0.21, -0.34], ["fr", 0.21, -0.34], ["bl", -0.22, 0.36], ["br", 0.22, 0.36]] as const) {
    part(`leg_${suffix}`, box({ parent: "body", at: [x, -0.38, z], size: [0.15, 0.34, 0.17], material: "gold_dark", joint: { pivot: [0, 0.16, 0], axis: [1, 0, 0] } }));
    part(`paw_${suffix}`, box({ parent: `leg_${suffix}`, at: [0, -0.22, -0.06], size: [0.22, 0.11, 0.3], material: "paw" }));
  }
  part("tail_1", box({ parent: "body", at: [0, 0.24, 0.61], rot: [42, 0, 0], size: [0.24, 0.58, 0.24], material: "gold_dark", joint: { pivot: [0, -0.27, 0], axis: [1, 0, 0] } }));
  for (const [index, height, width, rot] of [[2, 0.54, 0.22, 8], [3, 0.5, 0.2, 12], [4, 0.44, 0.17, 18], [5, 0.36, 0.13, 24]] as const) part(`tail_${index}`, box({ parent: `tail_${index - 1}`, at: [0, index === 2 ? 0.42 : index === 3 ? 0.39 : index === 4 ? 0.35 : 0.29, 0.03], rot: [rot, 0, 0], size: [width, height, width], material: index % 2 ? "gold" : "gold_light", joint: { pivot: [0, -height * 0.44, 0], axis: [1, 0, 0] } }));
  quadrupedWalk("canopy_prowl", { label: "Canopy prowl", fps: 18, duration: 1.14, cycleDistance: 0.56, gait: "walk", loop: true, samples: 21, contactParts: { frontLeft: "paw_fl", frontRight: "paw_fr", backLeft: "paw_bl", backRight: "paw_br" }, body: "body", bodyBob: 0.011, bodyBobCenter: 0.013, head: "head", headSwingDegrees: 2.5, legs: { frontLeft: "leg_fl", frontRight: "leg_fr", backLeft: "leg_bl", backRight: "leg_br" }, stanceRatio: 0.7, swingDegrees: 15, tail: "tail_1", tailSwingDegrees: 7, tracks: [followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "x", axis: "x", degrees: 9, overshoot: 0.55, lag: 0.13 }), followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.4, lag: 0.1 }), followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 5, overshoot: 0.4, lag: 0.1 })] });
  clip("tail_hook", { label: "Tail hook", role: "action", nextClip: "canopy_prowl", fps: 30, loop: false, keys: [
    ["tail_1", 0, { rot: [0, 0, 0] }], ["tail_1", 0.32, { rot: [-18, 0, 0] }], ["tail_1", 0.68, { rot: [-30, 0, 0] }], ["tail_1", 1.18, { rot: [0, 0, 0] }], ["tail_2", 0, { rot: [0, 0, 0] }], ["tail_2", 0.32, { rot: [-24, 0, 0] }], ["tail_2", 0.68, { rot: [-40, 0, 0] }], ["tail_2", 1.18, { rot: [0, 0, 0] }], ["tail_3", 0, { rot: [0, 0, 0] }], ["tail_3", 0.32, { rot: [-30, 0, 0] }], ["tail_3", 0.68, { rot: [-50, 0, 0] }], ["tail_3", 1.18, { rot: [0, 0, 0] }], ["tail_4", 0, { rot: [0, 0, 0] }], ["tail_4", 0.68, { rot: [-58, 0, 0] }], ["tail_4", 1.18, { rot: [0, 0, 0] }],
  ] });
  defaultClip("canopy_prowl");
});
