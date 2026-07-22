import { figure } from "../../src/dsl";

// A box-only stone gargoyle with glowing eyes, brow horns, clawed limbs,
// broad two-stage wings, a barbed tail, land stalk, glide, and awakening action.
export default figure("gargoyle", ({ asciiTexture, bipedWalk, box, clip, defaultClip, followThrough, mat, metadata, part, wingFlap }) => {
  metadata({
    bodyPlans: ["biped", "winged"],
    disposition: "hostile",
    groups: ["fantasy", "monster", "construct"],
    habitats: ["land", "air"],
    scale: "large",
    themes: ["scary", "stone", "nocturnal"],
  });
  mat("stone", "#626c6a"); mat("stone_light", "#87918c"); mat("stone_dark", "#343d3c"); mat("moss", "#4e684d"); mat("eye", "#e4b840"); mat("claw", "#232928");
  asciiTexture("stone_face", { palette: { ".": "#626c6a", "l": "#87918c", "d": "#343d3c", "e": "#e4b840" }, pixels: ["dd......dd", "d.ee..ee.d", "..ee..ee..", "...dddd...", "..d....d..", ".d.dddd.d."] });
  asciiTexture("wing_ribs", { palette: { ".": "#343d3c", "l": "#626c6a", "m": "#4e684d" }, pixels: ["llllllllllll", "l..........l", ".l..l..l..l.", "..m..m..m...", "...l..l.....", "............"] });
  asciiTexture("chest", { palette: { ".": "#626c6a", "l": "#87918c", "d": "#343d3c", "m": "#4e684d" }, pixels: ["dd......dd", "dlllllllld", ".ll....ll.", "..m....m..", "...dddd..."] });
  part("torso", box({ at: [0, 0.84, 0], size: [0.72, 0.76, 0.42], material: "stone" }));
  part("chest_plate", box({ parent: "torso", at: [0, 0.05, -0.25], size: [0.58, 0.58, 0.16], material: "stone_light", faces: { north: { texture: "chest" } } }));
  part("shoulder_bar", box({ parent: "torso", at: [0, 0.28, 0], size: [0.94, 0.18, 0.46], material: "stone_dark" }));
  part("neck", box({ parent: "torso", at: [0, 0.5, -0.02], size: [0.4, 0.3, 0.34], material: "stone_dark", joint: { pivot: [0, -0.13, 0.1], axis: [1, 0, 0] } }));
  part("head", box({ parent: "neck", at: [0, 0.29, -0.1], size: [0.62, 0.5, 0.5], material: "stone", faces: { north: { texture: "stone_face" } } }));
  part("muzzle", box({ parent: "head", at: [0, -0.12, -0.33], size: [0.42, 0.22, 0.22], material: "stone_dark" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`horn_${side}`, box({ parent: "head", at: [sign * 0.23, 0.35, 0.03], rot: [-12, 0, sign * -18], size: [0.13, 0.38, 0.13], material: "claw" }));
    part(`ear_${side}`, box({ parent: "head", at: [sign * 0.37, 0.06, 0.02], rot: [0, 0, sign * -18], size: [0.22, 0.12, 0.16], material: "stone_light" }));
    part(`upper_arm_${side}`, box({ parent: "shoulder_bar", at: [sign * 0.51, -0.28, -0.01], rot: [-5, 0, sign * -5], size: [0.24, 0.58, 0.27], material: "stone", joint: { pivot: [0, 0.28, 0], axis: [1, 0, 0] } }));
    part(`forearm_${side}`, box({ parent: `upper_arm_${side}`, at: [0, -0.42, -0.04], size: [0.26, 0.42, 0.3], material: "stone_dark" }));
    part(`claw_${side}`, box({ parent: `forearm_${side}`, at: [0, -0.27, -0.1], size: [0.3, 0.16, 0.38], material: "claw" }));
    part(`leg_${side}`, box({ parent: "torso", at: [sign * 0.2, -0.56, 0.02], size: [0.27, 0.28, 0.3], material: "stone_dark", joint: { pivot: [0, 0.14, 0], axis: [1, 0, 0] } }));
    part(`foot_${side}`, box({ parent: `leg_${side}`, at: [0, -0.2, -0.1], size: [0.36, 0.12, 0.48], material: "claw" }));
    part(`wing_${side}`, box({ parent: "shoulder_bar", at: [sign * 0.68, 0.1, 0.14], rot: [0, sign * -8, sign * 24], size: [0.86, 0.075, 0.78], material: "stone_dark", faces: { up: { texture: "wing_ribs" }, down: { texture: "wing_ribs" } }, joint: { pivot: [sign * -0.4, 0, -0.08], axis: [0, 0, 1] } }));
    part(`wing_tip_${side}`, box({ parent: `wing_${side}`, at: [sign * 0.62, 0, 0.08], rot: [0, sign * -13, 0], size: [0.5, 0.055, 0.72], material: "stone", faces: { up: { texture: "wing_ribs" }, down: { texture: "wing_ribs" } } }));
  }
  part("tail_1", box({ parent: "torso", at: [0, 0.02, 0.28], rot: [55, 0, 0], size: [0.22, 0.52, 0.22], material: "stone_dark", joint: { pivot: [0, -0.24, 0], axis: [1, 0, 0] } }));
  part("tail_2", box({ parent: "tail_1", at: [0, 0.38, 0.03], rot: [18, 0, 0], size: [0.18, 0.4, 0.18], material: "stone" }));
  part("tail_barb", box({ parent: "tail_2", at: [0, 0.28, 0], rot: [0, 0, 45], size: [0.25, 0.25, 0.12], material: "claw" }));
  bipedWalk("stone_stalk", { label: "Stone stalk", fps: 18, duration: 1.18, cycleDistance: 0.62, loop: true, samples: 23, armSwingDegrees: 10, body: "torso", bodyBob: 0.012, bodyBobCenter: 0.014, head: "neck", headSwingDegrees: 3, leftArm: "upper_arm_l", leftContact: "foot_l", leftLeg: "leg_l", rightArm: "upper_arm_r", rightContact: "foot_r", rightLeg: "leg_r", stanceRatio: 0.69, swingDegrees: 16 });
  wingFlap("night_glide", { label: "Night glide", fps: 24, duration: 1.0, cycleDistance: 1.18, loop: true, samples: 25, body: "torso", bodyBob: 0.026, degrees: 28, frequency: 1, leftWing: "wing_l", rightWing: "wing_r", tracks: [followThrough("wing_tip_l", { source: "wing_l", sourceChannel: "rot", sourceAxis: "z", axis: "z", degrees: 15, overshoot: 0.42, lag: 0.1 }), followThrough("wing_tip_r", { source: "wing_r", sourceChannel: "rot", sourceAxis: "z", axis: "z", degrees: 15, overshoot: 0.42, lag: 0.1 }), followThrough("tail_1", { source: "torso", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 8, overshoot: 0.48, lag: 0.14 })] });
  clip("stone_awaken", { label: "Stone awaken", role: "action", nextClip: "stone_stalk", fps: 30, loop: false, keys: [
    ["neck", 0, { rot: [0, 0, 0] }], ["neck", 0.26, { rot: [-18, 0, 0] }], ["neck", 0.54, { rot: [10, 0, 0] }], ["neck", 1.12, { rot: [0, 0, 0] }],
    ["wing_l", 0, { rot: [0, 0, 0] }], ["wing_l", 0.26, { rot: [0, 0, 20] }], ["wing_l", 0.62, { rot: [0, 0, -34] }], ["wing_l", 1.12, { rot: [0, 0, 0] }], ["wing_r", 0, { rot: [0, 0, 0] }], ["wing_r", 0.26, { rot: [0, 0, -20] }], ["wing_r", 0.62, { rot: [0, 0, 34] }], ["wing_r", 1.12, { rot: [0, 0, 0] }],
    ["upper_arm_l", 0, { rot: [0, 0, 0] }], ["upper_arm_l", 0.62, { rot: [-24, 0, -10] }], ["upper_arm_l", 1.12, { rot: [0, 0, 0] }], ["upper_arm_r", 0, { rot: [0, 0, 0] }], ["upper_arm_r", 0.62, { rot: [-24, 0, 10] }], ["upper_arm_r", 1.12, { rot: [0, 0, 0] }],
    ["tail_1", 0, { rot: [0, 0, 0] }], ["tail_1", 0.62, { rot: [15, 0, 0] }], ["tail_1", 1.12, { rot: [0, 0, 0] }],
  ] });
  defaultClip("stone_stalk");
});
