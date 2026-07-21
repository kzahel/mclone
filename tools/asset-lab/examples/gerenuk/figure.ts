import { figure } from "../../src/dsl";

// A box-only male gerenuk with a slender body, exceptionally long neck and
// legs, lyre horns, careful walk, and a separate upright browsing reach.
export default figure("gerenuk", ({ asciiTexture, box, clip, defaultClip, followThrough, mat, part, quadrupedWalk }) => {
  mat("tan", "#b78155"); mat("tan_light", "#d2aa7a"); mat("tan_dark", "#79513c"); mat("white", "#e4d9bf"); mat("black", "#2b2925"); mat("horn", "#493d34"); mat("hoof", "#332d29"); mat("eye", "#181512");
  asciiTexture("face", { palette: { ".": "#b78155", "w": "#e4d9bf", "e": "#181512", "d": "#79513c" }, pixels: ["dd....dd", "d.e..e.d", "..wwww..", ".w....w.", "...ww...", "........"] });
  part("body", box({ at: [0, 1.4, 0.1], size: [0.68, 0.58, 1.22], material: "tan", joint: { pivot: [0, -0.27, 0.45], axis: [1, 0, 0] } }));
  part("belly", box({ parent: "body", at: [0, -0.32, 0], size: [0.54, 0.12, 0.9], material: "white" }));
  part("shoulder", box({ parent: "body", at: [0, 0.03, -0.5], size: [0.72, 0.62, 0.4], material: "tan_dark" }));
  part("neck_low", box({ parent: "body", at: [0, 0.48, -0.5], rot: [-13, 0, 0], size: [0.34, 0.88, 0.36], material: "tan", joint: { pivot: [0, -0.4, 0.1], axis: [1, 0, 0] } }));
  part("neck_high", box({ parent: "neck_low", at: [0, 0.63, -0.06], rot: [8, 0, 0], size: [0.29, 0.64, 0.31], material: "tan_light" }));
  part("head", box({ parent: "neck_high", at: [0, 0.43, -0.15], size: [0.46, 0.44, 0.56], material: "tan", faces: { north: { texture: "face" } } }));
  part("muzzle", box({ parent: "head", at: [0, -0.12, -0.4], size: [0.32, 0.22, 0.28], material: "white" }));
  part("nose", box({ parent: "muzzle", at: [0, 0, -0.18], size: [0.2, 0.13, 0.09], material: "black" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`ear_${side}`, box({ parent: "head", at: [sign * 0.26, 0.18, 0.05], rot: [0, sign * -9, sign * 18], size: [0.17, 0.34, 0.13], material: "tan_dark", joint: { pivot: [sign * -0.05, -0.14, 0], axis: [0, 0, 1] } }));
    part(`horn_${side}_1`, box({ parent: "head", at: [sign * 0.13, 0.34, 0.08], rot: [-8, 0, sign * -8], size: [0.09, 0.42, 0.09], material: "horn" }));
    part(`horn_${side}_2`, box({ parent: `horn_${side}_1`, at: [0, 0.31, 0.05], rot: [18, 0, sign * 6], size: [0.075, 0.34, 0.075], material: "horn" }));
  }
  for (const [suffix, x, z] of [["fl", -0.22, -0.4], ["fr", 0.22, -0.4], ["bl", -0.23, 0.42], ["br", 0.23, 0.42]] as const) {
    part(`leg_${suffix}`, box({ parent: "body", at: [x, -0.56, z], size: [0.13, 0.72, 0.15], material: "tan_light", joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] } }));
    part(`shin_${suffix}`, box({ parent: `leg_${suffix}`, at: [0, -0.52, 0.02], size: [0.1, 0.44, 0.12], material: "tan_dark" }));
    part(`hoof_${suffix}`, box({ parent: `shin_${suffix}`, at: [0, -0.26, -0.07], size: [0.19, 0.1, 0.28], material: "hoof" }));
  }
  part("tail", box({ parent: "body", at: [0, 0.08, 0.7], rot: [-10, 0, 0], size: [0.18, 0.42, 0.2], material: "tan_dark", joint: { pivot: [0, 0.18, -0.05], axis: [1, 0, 0] } }));
  part("tail_tip", box({ parent: "tail", at: [0, -0.27, 0.05], size: [0.24, 0.2, 0.24], material: "black" }));
  quadrupedWalk("acacia_walk", { label: "Acacia walk", fps: 19, duration: 1.24, cycleDistance: 0.82, gait: "walk", loop: true, samples: 23, contactParts: { frontLeft: "hoof_fl", frontRight: "hoof_fr", backLeft: "hoof_bl", backRight: "hoof_br" }, body: "body", bodyBob: 0.012, bodyBobCenter: 0.014, head: "head", headSwingDegrees: 2.5, legs: { frontLeft: "leg_fl", frontRight: "leg_fr", backLeft: "leg_bl", backRight: "leg_br" }, stanceRatio: 0.69, swingDegrees: 18, tail: "tail", tailSwingDegrees: 6, tracks: [followThrough("neck_low", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.4, lag: 0.12 })] });
  clip("upright_browse", { label: "Upright browse", role: "action", nextClip: "acacia_walk", fps: 30, loop: false, keys: [
    ["body", 0, { rot: [0, 0, 0] }], ["body", 0.38, { rot: [34, 0, 0] }], ["body", 0.76, { rot: [52, 0, 0] }], ["body", 1.26, { rot: [0, 0, 0] }],
    ["neck_low", 0, { rot: [0, 0, 0] }], ["neck_low", 0.38, { rot: [-10, 0, 0] }], ["neck_low", 0.76, { rot: [-18, 0, 0] }], ["neck_low", 1.26, { rot: [0, 0, 0] }],
    ["leg_fl", 0, { rot: [0, 0, 0] }], ["leg_fl", 0.5, { rot: [-26, 0, 0] }], ["leg_fl", 0.92, { rot: [-20, 0, 0] }], ["leg_fl", 1.26, { rot: [0, 0, 0] }], ["leg_fr", 0, { rot: [0, 0, 0] }], ["leg_fr", 0.5, { rot: [-26, 0, 0] }], ["leg_fr", 0.92, { rot: [-20, 0, 0] }], ["leg_fr", 1.26, { rot: [0, 0, 0] }],
  ] });
  defaultClip("acacia_walk");
});
