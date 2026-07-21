import { figure } from "../../src/dsl";

// A box-only giant leaf insect with a broad veined abdomen, scalloped lobes,
// six leaf-like legs, long antennae, slow creep, and camouflage rocking.
export default figure("leaf_insect", ({ asciiTexture, bob, box, clip, contactSwing, defaultClip, mat, part, swing, walkCycle }) => {
  mat("leaf", "#73a657"); mat("leaf_light", "#9abc69"); mat("leaf_dark", "#3f743e"); mat("vein", "#d0c875"); mat("brown", "#695842"); mat("eye", "#402f22");
  asciiTexture("leaf_veins", { palette: { ".": "#73a657", "l": "#9abc69", "d": "#3f743e", "v": "#d0c875" }, pixels: ["....vv....", "...v..v...", "..v.vv.v..", ".v..vv..v.", "v...vv...v", ".v..vv..v.", "..v.vv.v..", "...v..v..."] });
  asciiTexture("face", { palette: { ".": "#73a657", "e": "#402f22", "d": "#3f743e" }, pixels: ["dd....dd", "d.e..e.d", "d......d", "..dddd..", "........"] });
  part("abdomen", box({ at: [0, 0.65, 0.2], size: [0.82, 0.16, 1.12], material: "leaf", faces: { up: { texture: "leaf_veins" } } }));
  for (const [index, x, z, size] of [[1, -0.49, -0.32, [0.24, 0.12, 0.34]], [2, 0.49, -0.32, [0.24, 0.12, 0.34]], [3, -0.53, 0.1, [0.3, 0.12, 0.38]], [4, 0.53, 0.1, [0.3, 0.12, 0.38]], [5, -0.43, 0.48, [0.22, 0.1, 0.28]], [6, 0.43, 0.48, [0.22, 0.1, 0.28]]] as const) part(`leaf_lobe_${index}`, box({ parent: "abdomen", at: [x, 0, z], rot: [0, index % 2 ? -8 : 8, 0], size, material: index > 4 ? "leaf_dark" : "leaf_light" }));
  part("thorax", box({ parent: "abdomen", at: [0, 0.03, -0.69], size: [0.46, 0.27, 0.38], material: "leaf_dark" }));
  part("head", box({ parent: "thorax", at: [0, 0, -0.34], size: [0.42, 0.3, 0.34], material: "leaf", faces: { north: { texture: "face" } } }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`antenna_${side}`, box({ parent: "head", at: [sign * 0.12, 0.17, -0.12], rot: [-38, 0, sign * 18], size: [0.035, 0.42, 0.035], material: "brown", joint: { pivot: [0, -0.19, 0], axis: [0, 0, 1] } }));
    part(`antenna_tip_${side}`, box({ parent: `antenna_${side}`, at: [0, 0.28, -0.04], rot: [-12, 0, sign * 6], size: [0.025, 0.26, 0.025], material: "brown" }));
  }
  for (const [row, z, yaw] of [["front", -0.38, 20], ["mid", 0, 4], ["rear", 0.38, -18]] as const) for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`leg_${row}_${side}`, box({ parent: "abdomen", at: [sign * 0.47, -0.08, z], rot: [0, sign * -yaw, sign * 22], size: [0.48, 0.08, 0.1], material: "leaf_light", joint: { pivot: [sign * -0.21, 0, 0], axis: [0, 1, 0] } }));
    part(`shin_${row}_${side}`, box({ parent: `leg_${row}_${side}`, at: [sign * 0.32, -0.16, 0], rot: [0, 0, sign * 30], size: [0.38, 0.065, 0.08], material: "leaf_dark" }));
    part(`foot_${row}_${side}`, box({ parent: `shin_${row}_${side}`, at: [sign * 0.26, -0.055, -0.04], size: [0.26, 0.04, 0.16], material: "brown" }));
  }
  walkCycle("leaf_creep", { label: "Leaf creep", role: "locomotion", fps: 18, duration: 1.24, loop: true, samples: 23, locomotion: { kind: "quadruped-walk", cycleDistance: 0.38, direction: [0, 0, -1], units: "figure", contacts: [{ part: "foot_front_l", phaseStart: 0, phaseEnd: 0.7, role: "front-left", stanceRatio: 0.7 }, { part: "foot_rear_r", phaseStart: 0, phaseEnd: 0.7, role: "back-right", stanceRatio: 0.7 }, { part: "foot_front_r", phaseStart: 0.5, phaseEnd: 0.2, role: "front-right", stanceRatio: 0.7 }, { part: "foot_rear_l", phaseStart: 0.5, phaseEnd: 0.2, role: "back-left", stanceRatio: 0.7 }] }, tracks: [
    bob("abdomen", { axis: "y", amount: 0.008, center: 0.01, phase: 0.5 }), swing("abdomen", { axis: "z", degrees: 2.5, phase: 0.25 }),
    contactSwing("leg_front_l", { axis: "y", degrees: 12, phase: 0, stanceRatio: 0.7 }), contactSwing("leg_rear_r", { axis: "y", degrees: 12, phase: 0, stanceRatio: 0.7 }), contactSwing("leg_front_r", { axis: "y", degrees: 12, phase: 0.5, stanceRatio: 0.7 }), contactSwing("leg_rear_l", { axis: "y", degrees: 12, phase: 0.5, stanceRatio: 0.7 }),
    swing("leg_mid_l", { axis: "y", degrees: 8, phase: 0.25 }), swing("leg_mid_r", { axis: "y", degrees: 8, phase: 0.75 }), swing("antenna_l", { axis: "z", degrees: 5, phase: 0.2 }), swing("antenna_r", { axis: "z", degrees: -5, phase: 0.2 }),
  ] });
  clip("camouflage_sway", { label: "Camouflage sway", role: "action", nextClip: "leaf_creep", fps: 30, loop: false, keys: [
    ["abdomen", 0, { rot: [0, 0, 0] }], ["abdomen", 0.32, { rot: [0, 0, -9] }], ["abdomen", 0.7, { rot: [0, 0, 10] }], ["abdomen", 1.06, { rot: [0, 0, -7] }], ["abdomen", 1.4, { rot: [0, 0, 0] }],
    ["antenna_l", 0, { rot: [0, 0, 0] }], ["antenna_l", 0.7, { rot: [0, 0, 8] }], ["antenna_l", 1.4, { rot: [0, 0, 0] }], ["antenna_r", 0, { rot: [0, 0, 0] }], ["antenna_r", 0.7, { rot: [0, 0, -8] }], ["antenna_r", 1.4, { rot: [0, 0, 0] }],
  ] });
  defaultClip("leaf_creep");
});
