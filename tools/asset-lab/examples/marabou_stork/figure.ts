import { figure } from "../../src/dsl";

// A box-only marabou stork with dark wings, white belly, bare pink head and
// neck, hanging throat pouch, long legs, stalking gait, and wing display.
export default figure("marabou_stork", ({ asciiTexture, bipedWalk, box, clip, defaultClip, followThrough, mat, part, swing }) => {
  mat("black", "#303433"); mat("black_light", "#4b504d"); mat("white", "#deddd3"); mat("skin", "#b97973"); mat("skin_dark", "#80514f"); mat("beak", "#a59677"); mat("leg", "#c6b39a"); mat("foot", "#62594f");
  asciiTexture("wing", { palette: { ".": "#303433", "l": "#4b504d", "w": "#deddd3" }, pixels: ["llllllllll", "l........l", "..llllll..", ".wwwwwwww.", ".........."] });
  asciiTexture("face", { palette: { ".": "#b97973", "d": "#80514f", "e": "#171817" }, pixels: ["dd....dd", ".de..ed.", "..dddd..", "........"] });
  part("body", box({ at: [0, 1.5, 0.05], size: [0.82, 0.76, 1.02], material: "black" }));
  part("breast", box({ parent: "body", at: [0, -0.02, -0.56], size: [0.62, 0.58, 0.18], material: "white" }));
  part("rump", box({ parent: "body", at: [0, 0, 0.54], size: [0.7, 0.6, 0.18], material: "black_light" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({ parent: "body", at: [sign * 0.47, 0.03, 0.03], size: [0.14, 0.58, 0.82], material: "black_light", faces: { [side === "l" ? "west" : "east"]: { texture: "wing" } }, joint: { pivot: [sign * -0.05, 0.2, -0.2], axis: [0, 0, 1] } }));
    part(`wing_tip_${side}`, box({ parent: `wing_${side}`, at: [sign * 0.08, -0.1, 0.18], size: [0.08, 0.42, 0.5], material: "black" }));
  }
  part("neck_lower", box({ parent: "body", at: [0, 0.55, -0.38], rot: [-10, 0, 0], size: [0.32, 0.58, 0.32], material: "skin_dark", joint: { pivot: [0, -0.26, 0.08], axis: [1, 0, 0] } }));
  part("neck_upper", box({ parent: "neck_lower", at: [0, 0.43, -0.08], rot: [9, 0, 0], size: [0.28, 0.46, 0.28], material: "skin" }));
  part("head", box({ parent: "neck_upper", at: [0, 0.3, -0.12], size: [0.44, 0.38, 0.42], material: "skin", faces: { north: { texture: "face" } } }));
  part("beak", box({ parent: "head", at: [0, -0.06, -0.42], size: [0.34, 0.2, 0.46], material: "beak" }));
  part("pouch", box({ parent: "neck_lower", at: [0, -0.02, -0.22], rot: [8, 0, 0], size: [0.24, 0.48, 0.2], material: "skin", joint: { pivot: [0, 0.21, 0], axis: [1, 0, 0] } }));
  for (const [side, x] of [["l", -0.25], ["r", 0.25]] as const) {
    part(`leg_${side}`, box({ parent: "body", at: [x, -0.64, 0.05], size: [0.17, 0.64, 0.18], material: "leg", joint: { pivot: [0, 0.31, 0], axis: [1, 0, 0] } }));
    part(`shin_${side}`, box({ parent: `leg_${side}`, at: [0, -0.48, 0.02], size: [0.12, 0.5, 0.13], material: "leg" }));
    part(`foot_${side}`, box({ parent: `shin_${side}`, at: [0, -0.3, -0.18], size: [0.34, 0.13, 0.52], material: "foot" }));
  }
  part("tail", box({ parent: "rump", at: [0, 0.04, 0.27], rot: [14, 0, 0], size: [0.52, 0.14, 0.44], material: "white" }));
  bipedWalk("wetland_stalk", { label: "Wetland stalk", fps: 19, duration: 1.08, cycleDistance: 0.68, loop: true, samples: 21, body: "body", bodyBob: 0.014, bodyBobCenter: 0.017, head: "head", headSwingDegrees: 2.4, leftLeg: "leg_l", rightLeg: "leg_r", leftContact: "foot_l", rightContact: "foot_r", stanceRatio: 0.7, swingDegrees: 17,
    tracks: [swing("wing_l", { axis: "z", degrees: 3, center: -1.5, frequency: 2 }), swing("wing_r", { axis: "z", degrees: -3, center: 1.5, frequency: 2 }), followThrough("neck_lower", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.5, lag: 0.11 }), followThrough("pouch", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.7, lag: 0.15 })] });
  clip("wing_display", { label: "Wing display", role: "action", nextClip: "wetland_stalk", fps: 30, loop: false, keys: [
    ["wing_l", 0, { rot: [0, 0, 0] }], ["wing_l", 0.36, { rot: [0, 0, -62] }], ["wing_l", 0.72, { rot: [0, 0, -68] }], ["wing_l", 1.12, { rot: [0, 0, 0] }],
    ["wing_r", 0, { rot: [0, 0, 0] }], ["wing_r", 0.36, { rot: [0, 0, 62] }], ["wing_r", 0.72, { rot: [0, 0, 68] }], ["wing_r", 1.12, { rot: [0, 0, 0] }],
    ["neck_lower", 0, { rot: [0, 0, 0] }], ["neck_lower", 0.48, { rot: [-8, 0, 0] }], ["neck_lower", 1.12, { rot: [0, 0, 0] }],
  ] });
  defaultClip("wetland_stalk");
});
