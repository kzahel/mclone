import { figure } from "../../src/dsl";

// A box-only Gaboon viper with a heavy leaf-patterned body, broad triangular
// head, paired nasal horns, slow ground slither, and explosive strike.
export default figure("gaboon_viper", ({ asciiTexture, box, clip, defaultClip, mat, part, slither, swing }) => {
  mat("tan", "#9d8061"); mat("tan_light", "#c1a480"); mat("brown", "#5e493c"); mat("cream", "#ded0ae"); mat("black", "#252322"); mat("horn", "#d0b78c"); mat("eye", "#d4ad48"); mat("mouth", "#663b43"); mat("fang", "#eee4ca");
  asciiTexture("leaf", { palette: { ".": "#9d8061", "l": "#c1a480", "d": "#5e493c", "c": "#ded0ae", "b": "#252322" }, pixels: ["bb..cc..bb..", "b.ll..ll..b.", "..dd..dd..dd", ".c..bb..c...", "dd..ll..dd.."] });
  asciiTexture("face", { palette: { ".": "#c1a480", "d": "#5e493c", "e": "#d4ad48", "b": "#252322" }, pixels: ["dd......dd", "d.eb..be.d", "..dddddd..", ".d......d.", ".........."] });
  part("head", box({ at: [0, 0.34, -0.66], size: [0.86, 0.36, 0.62], material: "tan_light", faces: { north: { texture: "face" } }, joint: { pivot: [0, 0, 0.27], axis: [0, 1, 0] } }));
  part("snout", box({ parent: "head", at: [0, -0.03, -0.4], size: [0.62, 0.26, 0.28], material: "brown" }));
  part("lower_jaw", box({ parent: "head", at: [0, -0.23, -0.31], size: [0.58, 0.13, 0.38], material: "mouth", joint: { pivot: [0, 0.05, 0.16], axis: [1, 0, 0] } }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`nasal_horn_${side}`, box({ parent: "snout", at: [sign * 0.14, 0.2, -0.08], rot: [-18, 0, sign * -7], size: [0.08, 0.28, 0.08], material: "horn" }));
    part(`fang_${side}`, box({ parent: "lower_jaw", at: [sign * 0.14, 0.12, -0.09], rot: [180, 0, 0], size: [0.05, 0.18, 0.05], material: "fang" }));
  }
  part("body_1", box({ parent: "head", at: [0, 0, 0.55], size: [0.68, 0.34, 0.64], material: "tan", faces: { east: { texture: "leaf" }, west: { texture: "leaf" }, up: { texture: "leaf" } }, joint: { pivot: [0, 0, -0.29], axis: [0, 1, 0] } }));
  for (const [index, width, height, length, material] of [[2, 0.72, 0.36, 0.7, "brown"], [3, 0.68, 0.34, 0.7, "tan_light"], [4, 0.6, 0.3, 0.64, "tan"], [5, 0.5, 0.25, 0.58, "brown"], [6, 0.38, 0.2, 0.5, "tan_light"], [7, 0.24, 0.14, 0.42, "tan"], [8, 0.13, 0.09, 0.34, "brown"]] as const) part(`body_${index}`, box({ parent: `body_${index - 1}`, at: [0, index <= 3 ? -0.025 : -0.01, index === 2 ? 0.58 : index === 3 ? 0.64 : index === 4 ? 0.61 : index === 5 ? 0.55 : index === 6 ? 0.48 : index === 7 ? 0.4 : 0.32], size: [width, height, length], material, faces: { east: { texture: "leaf" }, west: { texture: "leaf" }, up: { texture: "leaf" } }, joint: { pivot: [0, 0, -length * 0.46], axis: [0, 1, 0] } }));
  slither("leaf_litter_slither", { label: "Leaf-litter slither", fps: 18, duration: 1.38, cycleDistance: 0.64, loop: true, samples: 29, body: "head", bodyBob: 0.006, degrees: 7, phaseStep: 0.12, segments: ["head", "body_1", "body_2", "body_3", "body_4", "body_5", "body_6", "body_7", "body_8"], tracks: [swing("lower_jaw", { axis: "x", degrees: 2, center: 1.5, frequency: 2 })] });
  clip("ambush_strike", { label: "Ambush strike", role: "action", nextClip: "leaf_litter_slither", fps: 30, loop: false, keys: [
    ["head", 0, { at: [0, 0, 0], rot: [0, 0, 0] }], ["head", 0.2, { at: [0, 0, 0.12], rot: [0, 0, 0] }], ["head", 0.34, { at: [0, 0.28, -0.34], rot: [-8, 0, 0] }], ["head", 0.56, { at: [0, 0.1, -0.16], rot: [3, 0, 0] }], ["head", 0.9, { at: [0, 0, 0], rot: [0, 0, 0] }],
    ["lower_jaw", 0, { rot: [0, 0, 0] }], ["lower_jaw", 0.2, { rot: [-10, 0, 0] }], ["lower_jaw", 0.34, { rot: [-38, 0, 0] }], ["lower_jaw", 0.52, { rot: [-4, 0, 0] }], ["lower_jaw", 0.9, { rot: [0, 0, 0] }],
  ] });
  defaultClip("leaf_litter_slither");
});
