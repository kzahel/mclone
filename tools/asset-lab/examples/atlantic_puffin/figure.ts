import { figure } from "../../src/dsl";

// A box-only Atlantic puffin with black mantle, white face, multicolor deep
// bill, orange webbed feet, cliff waddle, and a separate wing-drying display.
export default figure("atlantic_puffin", ({ asciiTexture, bipedWalk, box, clip, defaultClip, followThrough, mat, part, swing }) => {
  mat("black", "#202426"); mat("black_light", "#394043"); mat("white", "#ece8d8"); mat("orange", "#dc762e"); mat("red", "#b84032"); mat("yellow", "#e4b74b"); mat("blue", "#668d95"); mat("eye", "#171412");
  asciiTexture("face", { palette: { ".": "#ece8d8", "b": "#202426", "e": "#171412" }, pixels: ["bbbbbbbb", "b.e..e.b", "b......b", "........", "........"] });
  asciiTexture("wing", { palette: { ".": "#202426", "l": "#394043", "w": "#ece8d8" }, pixels: ["llllllll", "l......l", "..llll..", ".l....l.", "wwwwwwww"] });
  part("body", box({ at: [0, 0.72, 0.05], size: [0.72, 0.78, 0.72], material: "black" }));
  part("belly", box({ parent: "body", at: [0, -0.06, -0.42], size: [0.5, 0.58, 0.16], material: "white" }));
  part("head", box({ parent: "body", at: [0, 0.48, -0.06], size: [0.64, 0.54, 0.54], material: "black_light", faces: { north: { texture: "face" } }, joint: { pivot: [0, -0.23, 0.08], axis: [1, 0, 0] } }));
  part("face_patch", box({ parent: "head", at: [0, -0.02, -0.31], size: [0.52, 0.38, 0.12], material: "white" }));
  part("bill_base", box({ parent: "head", at: [0, -0.1, -0.43], size: [0.42, 0.28, 0.3], material: "orange" }));
  part("bill_mid", box({ parent: "bill_base", at: [0, 0, -0.23], size: [0.34, 0.24, 0.22], material: "yellow" }));
  part("bill_tip", box({ parent: "bill_mid", at: [0, 0, -0.16], size: [0.22, 0.18, 0.12], material: "red" }));
  part("bill_cere", box({ parent: "bill_base", at: [0, 0.1, 0.02], size: [0.36, 0.06, 0.08], material: "blue" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({ parent: "body", at: [sign * 0.4, 0.02, 0.02], rot: [5, 0, sign * 5], size: [0.13, 0.58, 0.54], material: "black_light", faces: { [side === "l" ? "west" : "east"]: { texture: "wing" } }, joint: { pivot: [sign * -0.05, 0.25, -0.12], axis: [0, 0, 1] } }));
    part(`leg_${side}`, box({ parent: "body", at: [sign * 0.18, -0.51, 0.02], size: [0.13, 0.24, 0.14], material: "orange", joint: { pivot: [0, 0.12, 0], axis: [1, 0, 0] } }));
    part(`foot_${side}`, box({ parent: `leg_${side}`, at: [0, -0.16, -0.11], size: [0.34, 0.1, 0.48], material: "orange" }));
  }
  part("tail", box({ parent: "body", at: [0, -0.18, 0.46], rot: [-16, 0, 0], size: [0.34, 0.26, 0.2], material: "black" }));
  bipedWalk("cliff_waddle", { label: "Cliff waddle", fps: 18, duration: 1.04, cycleDistance: 0.42, loop: true, samples: 19, body: "body", bodyBob: 0.017, bodyBobCenter: 0.019, head: "head", headSwingDegrees: 2, leftLeg: "leg_l", rightLeg: "leg_r", leftContact: "foot_l", rightContact: "foot_r", stanceRatio: 0.71, swingDegrees: 13, tracks: [swing("body", { axis: "z", degrees: 4, phase: 0.25 }), followThrough("head", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.45, lag: 0.11 })] });
  clip("wing_dry", { label: "Wing drying", role: "action", nextClip: "cliff_waddle", fps: 30, loop: false, keys: [
    ["wing_l", 0, { rot: [0, 0, 0] }], ["wing_l", 0.3, { rot: [0, 0, -55] }], ["wing_l", 0.7, { rot: [0, 0, -68] }], ["wing_l", 1.18, { rot: [0, 0, 0] }], ["wing_r", 0, { rot: [0, 0, 0] }], ["wing_r", 0.3, { rot: [0, 0, 55] }], ["wing_r", 0.7, { rot: [0, 0, 68] }], ["wing_r", 1.18, { rot: [0, 0, 0] }],
    ["head", 0, { rot: [0, 0, 0] }], ["head", 0.5, { rot: [-8, 0, 0] }], ["head", 1.18, { rot: [0, 0, 0] }],
  ] });
  defaultClip("cliff_waddle");
});
