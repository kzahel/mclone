import { figure } from "../../src/dsl";

// A box-only white-nosed coati with long flexible snout, masked eyes, dark
// paws, upright ringed tail, foraging walk, and a separate scent-probe action.
export default figure("white_nosed_coati", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, quadrupedWalk }) => {
  mat("brown", "#765038"); mat("brown_light", "#9b6b48"); mat("brown_dark", "#44352c"); mat("cream", "#dfcfb2"); mat("black", "#252321"); mat("paw", "#342e29");
  asciiTexture("face", { palette: { ".": "#9b6b48", "c": "#dfcfb2", "b": "#252321" }, pixels: ["bb....bb", "bcc..ccb", "bcbbbbcb", "..cccc..", "........"] });
  asciiTexture("rings", { palette: { ".": "#765038", "d": "#44352c", "c": "#dfcfb2" }, pixels: ["cccccccc", "........", "dddddddd", "........"] });
  part("body", box({ at: [0, 0.7, 0.05], size: [0.68, 0.5, 1.1], material: "brown" }));
  part("belly", box({ parent: "body", at: [0, -0.29, -0.02], size: [0.5, 0.11, 0.78], material: "cream" }));
  part("shoulders", box({ parent: "body", at: [0, 0.05, -0.48], size: [0.72, 0.48, 0.36], material: "brown_light" }));
  part("head", box({ parent: "body", at: [0, 0.04, -0.68], size: [0.5, 0.42, 0.42], material: "brown_light", faces: { north: { texture: "face" } }, joint: { pivot: [0, 0, 0.18], axis: [1, 0, 0] } }));
  part("snout_1", box({ parent: "head", at: [0, -0.1, -0.32], size: [0.3, 0.2, 0.24], material: "cream" }));
  part("snout_2", box({ parent: "snout_1", at: [0, -0.02, -0.2], size: [0.22, 0.15, 0.2], material: "brown_dark" }));
  part("nose", box({ parent: "snout_2", at: [0, 0, -0.13], size: [0.16, 0.12, 0.08], material: "black" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) part(`ear_${side}`, box({ parent: "head", at: [sign * 0.2, 0.24, 0.04], size: [0.14, 0.16, 0.1], material: "brown_dark", joint: { pivot: [0, -0.06, 0], axis: [1, 0, 0] } }));
  for (const [suffix, x, z] of [["fl", -0.22, -0.34], ["fr", 0.22, -0.34], ["bl", -0.23, 0.36], ["br", 0.23, 0.36]] as const) {
    part(`leg_${suffix}`, box({ parent: "body", at: [x, -0.38, z], size: [0.14, 0.34, 0.16], material: "brown_dark", joint: { pivot: [0, 0.16, 0], axis: [1, 0, 0] } }));
    part(`paw_${suffix}`, box({ parent: `leg_${suffix}`, at: [0, -0.22, -0.05], size: [0.21, 0.11, 0.27], material: "paw" }));
  }
  part("tail_1", box({ parent: "body", at: [0, 0.28, 0.58], rot: [-48, 0, 0], size: [0.28, 0.52, 0.28], material: "brown_dark", faces: { east: { texture: "rings" }, west: { texture: "rings" } }, joint: { pivot: [0, -0.24, 0], axis: [0, 0, 1] } }));
  for (const [index, width, height, material] of [[2, 0.27, 0.46, "cream"], [3, 0.24, 0.42, "brown"], [4, 0.2, 0.36, "cream"], [5, 0.15, 0.3, "brown_dark"]] as const) part(`tail_${index}`, box({ parent: `tail_${index - 1}`, at: [0, index === 2 ? 0.4 : index === 3 ? 0.37 : index === 4 ? 0.32 : 0.27, 0.02], rot: [-5, 0, 0], size: [width, height, width], material, faces: { east: { texture: "rings" }, west: { texture: "rings" } }, joint: { pivot: [0, -height * 0.44, 0], axis: [0, 0, 1] } }));
  quadrupedWalk("forage_walk", { label: "Forage walk", fps: 19, duration: 1.02, cycleDistance: 0.66, gait: "walk", loop: true, samples: 19, contactParts: { frontLeft: "paw_fl", frontRight: "paw_fr", backLeft: "paw_bl", backRight: "paw_br" }, body: "body", bodyBob: 0.012, bodyBobCenter: 0.014, head: "head", headSwingDegrees: 3, legs: { frontLeft: "leg_fl", frontRight: "leg_fr", backLeft: "leg_bl", backRight: "leg_br" }, stanceRatio: 0.68, swingDegrees: 17, tail: "tail_1", tailSwingDegrees: 7,
    tracks: [followThrough("tail_2", { source: "tail_1", sourceChannel: "rot", sourceAxis: "z", axis: "z", degrees: 10, overshoot: 0.7, lag: 0.15 }), followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.11 }), followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 6, overshoot: 0.5, lag: 0.11 })] });
  clip("scent_probe", { label: "Scent probe", role: "action", nextClip: "forage_walk", fps: 30, loop: false, keys: [
    ["head", 0, { rot: [0, 0, 0] }], ["head", 0.32, { rot: [20, -8, 0] }], ["head", 0.62, { rot: [24, 9, 0] }], ["head", 0.9, { rot: [18, -6, 0] }], ["head", 1.18, { rot: [0, 0, 0] }],
    ["tail_1", 0, { rot: [0, 0, 0] }], ["tail_1", 0.5, { rot: [0, 0, 10] }], ["tail_1", 0.9, { rot: [0, 0, -8] }], ["tail_1", 1.18, { rot: [0, 0, 0] }],
  ] });
  defaultClip("forage_walk");
});
