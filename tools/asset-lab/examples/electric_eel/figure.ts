import { figure } from "../../src/dsl";

// A box-only electric eel with a blunt face, long dark body, orange throat,
// continuous underside fin, traveling ribbon swim, and electric-pulse action.
export default figure("electric_eel", ({ asciiTexture, bob, box, clip, defaultClip, mat, part, swing, walkCycle }) => {
  mat("olive", "#4e5740"); mat("olive_light", "#788064"); mat("olive_dark", "#29352e"); mat("orange", "#bc7048"); mat("fin", "#849070"); mat("eye", "#dfbf4a"); mat("pulse", "#d7d86c");
  asciiTexture("skin", { palette: { ".": "#4e5740", "l": "#788064", "d": "#29352e", "o": "#bc7048" }, pixels: ["d..l.d..l.d.", ".d..l.d..l.d", "..dd..ll..dd", "oooooooooooo"] });
  asciiTexture("face", { palette: { ".": "#788064", "d": "#29352e", "e": "#dfbf4a" }, pixels: ["dd....dd", "d.e..e.d", "d......d", "........", "dddddddd"] });
  part("head", box({ at: [0, 0.88, -0.72], size: [0.62, 0.48, 0.62], material: "olive_light", faces: { north: { texture: "face" } }, joint: { pivot: [0, 0, 0.27], axis: [0, 1, 0] } }));
  part("snout", box({ parent: "head", at: [0, -0.08, -0.38], size: [0.5, 0.26, 0.24], material: "olive_light" }));
  part("throat", box({ parent: "head", at: [0, -0.27, -0.02], size: [0.5, 0.13, 0.45], material: "orange" }));
  part("body_1", box({ parent: "head", at: [0, 0, 0.53], size: [0.54, 0.44, 0.62], material: "olive", faces: { east: { texture: "skin" }, west: { texture: "skin" } }, joint: { pivot: [0, 0, -0.28], axis: [0, 1, 0] } }));
  for (const [index, width, height, length, material] of [[2, 0.52, 0.42, 0.68, "olive_dark"], [3, 0.48, 0.38, 0.68, "olive"], [4, 0.42, 0.32, 0.64, "olive_light"], [5, 0.34, 0.26, 0.58, "olive"], [6, 0.25, 0.19, 0.5, "olive_dark"], [7, 0.15, 0.12, 0.4, "olive"]] as const) part(`body_${index}`, box({ parent: `body_${index - 1}`, at: [0, 0, index === 2 ? 0.57 : index === 3 ? 0.62 : index === 4 ? 0.6 : index === 5 ? 0.55 : index === 6 ? 0.48 : 0.4], size: [width, height, length], material, faces: { east: { texture: "skin" }, west: { texture: "skin" } }, joint: { pivot: [0, 0, -length * 0.44], axis: [0, 1, 0] } }));
  for (const [index, parent, z, length] of [[1, "body_1", 0.08, 0.5], [2, "body_2", 0.12, 0.56], [3, "body_3", 0.1, 0.56], [4, "body_4", 0.08, 0.5], [5, "body_5", 0.06, 0.42]] as const) part(`fin_${index}`, box({ parent, at: [0, -0.26, z], size: [0.08, 0.18, length], material: index % 2 ? "fin" : "pulse" }));
  walkCycle("electric_ribbon_swim", { label: "Electric ribbon swim", role: "locomotion", fps: 24, duration: 1.18, loop: true, samples: 31, locomotion: { kind: "swim", cycleDistance: 0.92, direction: [0, 0, -1], units: "figure" }, tracks: [bob("head", { axis: "y", amount: 0.02, phase: 0.5 }), swing("head", { axis: "y", degrees: 2.5, phase: 0 }), swing("body_1", { axis: "y", degrees: 5, phase: 0.08 }), swing("body_2", { axis: "y", degrees: 8, phase: 0.18 }), swing("body_3", { axis: "y", degrees: 11, phase: 0.28 }), swing("body_4", { axis: "y", degrees: 14, phase: 0.38 }), swing("body_5", { axis: "y", degrees: 17, phase: 0.48 }), swing("body_6", { axis: "y", degrees: 20, phase: 0.58 }), swing("body_7", { axis: "y", degrees: 23, phase: 0.68 })] });
  clip("electric_pulse", { label: "Electric pulse", role: "action", nextClip: "electric_ribbon_swim", fps: 30, loop: false, keys: [
    ["head", 0, { scale: [1, 1, 1] }], ["head", 0.16, { scale: [1.04, 1.04, 0.97] }], ["head", 0.26, { scale: [0.97, 0.97, 1.03] }], ["head", 0.38, { scale: [1.04, 1.04, 0.97] }], ["head", 0.54, { scale: [1, 1, 1] }], ["head", 0.76, { scale: [1.03, 1.03, 0.98] }], ["head", 1.0, { scale: [1, 1, 1] }],
    ["body_2", 0, { rot: [0, 0, 0] }], ["body_2", 0.16, { rot: [0, 12, 0] }], ["body_2", 0.26, { rot: [0, -12, 0] }], ["body_2", 0.38, { rot: [0, 10, 0] }], ["body_2", 0.54, { rot: [0, 0, 0] }], ["body_2", 0.76, { rot: [0, -8, 0] }], ["body_2", 1.0, { rot: [0, 0, 0] }],
  ] });
  defaultClip("electric_ribbon_swim");
});
