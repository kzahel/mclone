import { figure } from "../../src/dsl";

// A box-only blue-footed booby with brown wings, white body, dagger bill,
// vivid cyan feet, deliberate courtship walk, and a sky-point display.
export default figure("blue_footed_booby", ({ asciiTexture, bipedWalk, box, clip, defaultClip, followThrough, mat, part, swing }) => {
  mat("brown", "#6a5b4e"); mat("brown_light", "#8b7763"); mat("brown_dark", "#413a35"); mat("white", "#e6e2d6"); mat("cream", "#cdbf9e"); mat("bill", "#89999a"); mat("blue", "#49b9c7"); mat("eye", "#d6b23f"); mat("black", "#191a19");
  asciiTexture("wing", { palette: { ".": "#6a5b4e", "l": "#8b7763", "d": "#413a35", "w": "#e6e2d6" }, pixels: ["llllllllll", "l........l", "..dddddd..", ".d......d.", "wwwwwwwwww"] });
  asciiTexture("face", { palette: { ".": "#e6e2d6", "b": "#6a5b4e", "e": "#d6b23f", "p": "#191a19" }, pixels: ["bb....bb", "b.eppe.b", "b......b", "........"] });
  part("body", box({ at: [0, 0.92, 0.08], size: [0.7, 0.84, 1.02], material: "white" }));
  part("breast", box({ parent: "body", at: [0, -0.04, -0.56], size: [0.54, 0.62, 0.18], material: "cream" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) part(`wing_${side}`, box({ parent: "body", at: [sign * 0.4, 0.06, 0.04], size: [0.12, 0.64, 0.8], material: "brown", faces: { [side === "l" ? "west" : "east"]: { texture: "wing" } }, joint: { pivot: [sign * -0.05, 0.24, -0.22], axis: [0, 0, 1] } }));
  part("neck", box({ parent: "body", at: [0, 0.42, -0.42], rot: [-8, 0, 0], size: [0.38, 0.5, 0.38], material: "white", joint: { pivot: [0, -0.22, 0.1], axis: [1, 0, 0] } }));
  part("head", box({ parent: "neck", at: [0, 0.34, -0.16], size: [0.5, 0.44, 0.48], material: "brown_light", faces: { north: { texture: "face" } } }));
  part("bill_1", box({ parent: "head", at: [0, -0.06, -0.4], size: [0.38, 0.18, 0.44], material: "bill" }));
  part("bill_tip", box({ parent: "bill_1", at: [0, 0, -0.32], size: [0.24, 0.12, 0.28], material: "bill" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`leg_${side}`, box({ parent: "body", at: [sign * 0.19, -0.56, 0.02], size: [0.13, 0.34, 0.14], material: "blue", joint: { pivot: [0, 0.16, 0], axis: [1, 0, 0] } }));
    part(`foot_${side}`, box({ parent: `leg_${side}`, at: [0, -0.22, -0.13], size: [0.42, 0.11, 0.58], material: "blue" }));
  }
  part("tail", box({ parent: "body", at: [0, 0.02, 0.64], rot: [18, 0, 0], size: [0.46, 0.22, 0.34], material: "brown_dark" }));
  bipedWalk("courtship_walk", { label: "Courtship walk", fps: 18, duration: 1.32, cycleDistance: 0.48, loop: true, samples: 23, body: "body", bodyBob: 0.014, bodyBobCenter: 0.016, head: "head", headSwingDegrees: 2, leftLeg: "leg_l", rightLeg: "leg_r", leftContact: "foot_l", rightContact: "foot_r", stanceRatio: 0.74, swingDegrees: 17, tracks: [swing("body", { axis: "z", degrees: 3.5, phase: 0.25 }), followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.12 })] });
  clip("sky_point", { label: "Sky point", role: "action", nextClip: "courtship_walk", fps: 30, loop: false, keys: [
    ["neck", 0, { rot: [0, 0, 0] }], ["neck", 0.32, { rot: [28, 0, 0] }], ["neck", 0.7, { rot: [42, 0, 0] }], ["neck", 1.2, { rot: [0, 0, 0] }], ["head", 0, { rot: [0, 0, 0] }], ["head", 0.32, { rot: [22, 0, 0] }], ["head", 0.7, { rot: [34, 0, 0] }], ["head", 1.2, { rot: [0, 0, 0] }],
    ["wing_l", 0, { rot: [0, 0, 0] }], ["wing_l", 0.7, { rot: [0, 0, -16] }], ["wing_l", 1.2, { rot: [0, 0, 0] }], ["wing_r", 0, { rot: [0, 0, 0] }], ["wing_r", 0.7, { rot: [0, 0, 16] }], ["wing_r", 1.2, { rot: [0, 0, 0] }],
  ] });
  defaultClip("courtship_walk");
});
