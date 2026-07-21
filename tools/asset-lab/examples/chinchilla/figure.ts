import { figure } from "../../src/dsl";

// A box-only chinchilla with dense silver coat, huge round ears, whiskered
// muzzle, plush curled tail, quick highland hop, and dust-bath shimmy.
export default figure("chinchilla", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, quadrupedWalk }) => {
  mat("gray", "#8a8e91"); mat("gray_light", "#b3b6b5"); mat("gray_dark", "#555c61"); mat("white", "#e1ded2"); mat("pink", "#c98f96"); mat("black", "#201e1c"); mat("whisker", "#d9d0bc");
  asciiTexture("fur", { palette: { ".": "#8a8e91", "l": "#b3b6b5", "d": "#555c61", "w": "#e1ded2" }, pixels: ["dd..ll..dd", ".d..ll..d.", "..dd..dd..", "wwwwwwwwww"] });
  asciiTexture("face", { palette: { ".": "#b3b6b5", "e": "#201e1c", "w": "#e1ded2", "p": "#c98f96" }, pixels: ["ee....ee", "e......e", "..wwww..", ".wwppww.", "...ww..."] });
  part("body", box({ at: [0, 0.51, 0.08], size: [0.74, 0.62, 0.84], material: "gray", faces: { east: { texture: "fur" }, west: { texture: "fur" } } }));
  part("rump", box({ parent: "body", at: [0, 0.03, 0.34], size: [0.8, 0.66, 0.46], material: "gray_light" }));
  part("belly", box({ parent: "body", at: [0, -0.34, -0.07], size: [0.56, 0.12, 0.56], material: "white" }));
  part("head", box({ parent: "body", at: [0, 0.11, -0.5], size: [0.66, 0.56, 0.5], material: "gray_light", faces: { north: { texture: "face" } }, joint: { pivot: [0, 0, 0.21], axis: [1, 0, 0] } }));
  part("muzzle", box({ parent: "head", at: [0, -0.15, -0.33], size: [0.4, 0.23, 0.2], material: "white" }));
  part("nose", box({ parent: "muzzle", at: [0, 0.03, -0.14], size: [0.14, 0.1, 0.08], material: "pink" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({ parent: "head", at: [sign * 0.24, 0.38, 0.04], size: [0.28, 0.38, 0.13], material: "gray_dark", joint: { pivot: [0, -0.17, 0], axis: [1, 0, 0] } }));
    part(`ear_inner_${side}`, box({ parent: `ear_${side}`, at: [0, 0, -0.08], size: [0.16, 0.26, 0.025], material: "pink" }));
    for (const [index, y] of [[1, -0.05], [2, -0.13]] as const) part(`whisker_${side}_${index}`, box({ parent: "muzzle", at: [sign * 0.31, y, -0.08], rot: [0, sign * -8, sign * 5], size: [0.38, 0.025, 0.025], material: "whisker" }));
  }
  for (const [suffix, x, z, rear] of [["fl", -0.22, -0.22, false], ["fr", 0.22, -0.22, false], ["bl", -0.25, 0.27, true], ["br", 0.25, 0.27, true]] as const) {
    part(`leg_${suffix}`, box({ parent: "body", at: [x, -0.36, z], size: [rear ? 0.2 : 0.15, rear ? 0.28 : 0.22, rear ? 0.22 : 0.17], material: "gray_dark", joint: { pivot: [0, rear ? 0.13 : 0.1, 0], axis: [1, 0, 0] } }));
    part(`foot_${suffix}`, box({ parent: `leg_${suffix}`, at: [0, rear ? -0.18 : -0.15, -0.07], size: [rear ? 0.28 : 0.22, 0.09, rear ? 0.38 : 0.28], material: "pink" }));
  }
  part("tail_1", box({ parent: "rump", at: [0, 0.3, 0.4], rot: [58, 0, 0], size: [0.32, 0.52, 0.32], material: "gray_dark", joint: { pivot: [0, -0.24, 0], axis: [1, 0, 0] } }));
  part("tail_2", box({ parent: "tail_1", at: [0, 0.37, 0.03], rot: [22, 0, 0], size: [0.3, 0.42, 0.3], material: "gray_light" }));
  part("tail_tip", box({ parent: "tail_2", at: [0, 0.3, 0.02], rot: [25, 0, 0], size: [0.22, 0.32, 0.22], material: "white" }));
  quadrupedWalk("highland_hop", { label: "Highland hop", fps: 22, duration: 0.7, cycleDistance: 0.52, gait: "trot", loop: true, samples: 23, contactParts: { frontLeft: "foot_fl", frontRight: "foot_fr", backLeft: "foot_bl", backRight: "foot_br" }, body: "body", bodyBob: 0.026, bodyBobCenter: 0.026, head: "head", headSwingDegrees: 3, legs: { frontLeft: "leg_fl", frontRight: "leg_fr", backLeft: "leg_bl", backRight: "leg_br" }, stanceRatio: 0.56, swingDegrees: 22, tail: "tail_1", tailSwingDegrees: 9, tracks: [followThrough("ear_l", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.1 }), followThrough("ear_r", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 10, overshoot: 0.65, lag: 0.1 })] });
  clip("dust_bath", { label: "Dust bath", role: "action", nextClip: "highland_hop", fps: 30, loop: false, keys: [
    ["body", 0, { rot: [0, 0, 0] }], ["body", 0.2, { rot: [0, -12, 0] }], ["body", 0.38, { rot: [0, 14, 0] }], ["body", 0.56, { rot: [0, -15, 0] }], ["body", 0.76, { rot: [0, 12, 0] }], ["body", 1.06, { rot: [0, 0, 0] }],
    ["tail_1", 0, { rot: [0, 0, 0] }], ["tail_1", 0.38, { rot: [12, 0, 0] }], ["tail_1", 0.76, { rot: [-10, 0, 0] }], ["tail_1", 1.06, { rot: [0, 0, 0] }],
  ] });
  defaultClip("highland_hop");
});
