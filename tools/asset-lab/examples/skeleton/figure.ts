import { figure } from "../../src/dsl";

// A box-only graveyard skeleton with a hollow-eyed skull, articulated jaw,
// layered ribs, long bones, a planted march, and a separate bone-rattle action.
export default figure("skeleton", ({ asciiTexture, bipedWalk, box, clip, defaultClip, mat, metadata, part }) => {
  metadata({
    bodyPlans: ["biped"],
    disposition: "hostile",
    groups: ["fantasy", "monster", "humanoid"],
    habitats: ["land", "underground"],
    scale: "medium",
    themes: ["scary", "undead", "graveyard"],
  });
  mat("bone", "#d8d0b5"); mat("bone_light", "#eee8d0"); mat("bone_dark", "#9c947d"); mat("void", "#171816"); mat("tooth", "#f3ecd5");
  asciiTexture("skull", { palette: { ".": "#d8d0b5", "l": "#eee8d0", "d": "#9c947d", "v": "#171816" }, pixels: ["llllllll", "l......l", ".vv..vv.", ".vv..vv.", "...vv...", "d......d", "dd.dd.dd"] });
  asciiTexture("jaw", { palette: { ".": "#9c947d", "t": "#f3ecd5", "v": "#171816" }, pixels: ["tttttttt", "tvtvtvtt", ".v.v.v..", "........"] });
  part("spine", box({ at: [0, 0.78, 0], size: [0.16, 0.72, 0.18], material: "bone_dark" }));
  part("pelvis", box({ parent: "spine", at: [0, -0.29, 0], size: [0.48, 0.2, 0.3], material: "bone" }));
  for (const [index, y, width] of [[1, -0.04, 0.58], [2, 0.1, 0.66], [3, 0.24, 0.72]] as const) {
    part(`rib_${index}`, box({ parent: "spine", at: [0, y, -0.01], size: [width, 0.08, 0.28], material: index === 2 ? "bone_light" : "bone" }));
  }
  part("shoulders", box({ parent: "spine", at: [0, 0.34, 0], size: [0.82, 0.12, 0.24], material: "bone" }));
  part("neck", box({ parent: "spine", at: [0, 0.48, 0], size: [0.18, 0.2, 0.18], material: "bone_dark", joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] } }));
  part("skull", box({ parent: "neck", at: [0, 0.3, -0.01], size: [0.5, 0.5, 0.46], material: "bone", faces: { north: { texture: "skull" } } }));
  part("brow", box({ parent: "skull", at: [0, 0.16, -0.27], size: [0.42, 0.1, 0.12], material: "bone_light" }));
  part("jaw", box({ parent: "skull", at: [0, -0.3, -0.08], size: [0.36, 0.16, 0.34], material: "bone_dark", faces: { north: { texture: "jaw" } }, joint: { pivot: [0, 0.06, 0.13], axis: [1, 0, 0] } }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`upper_arm_${side}`, box({ parent: "shoulders", at: [sign * 0.46, -0.2, 0], size: [0.14, 0.5, 0.15], material: "bone", joint: { pivot: [0, 0.24, 0], axis: [1, 0, 0] } }));
    part(`elbow_${side}`, box({ parent: `upper_arm_${side}`, at: [0, -0.3, 0], size: [0.2, 0.16, 0.18], material: "bone_light" }));
    part(`forearm_${side}`, box({ parent: `elbow_${side}`, at: [0, -0.25, 0], size: [0.13, 0.42, 0.14], material: "bone_dark" }));
    part(`hand_${side}`, box({ parent: `forearm_${side}`, at: [0, -0.25, -0.02], size: [0.2, 0.14, 0.2], material: "bone" }));
    part(`leg_${side}`, box({ parent: "spine", at: [sign * 0.15, -0.48, 0], size: [0.15, 0.36, 0.17], material: "bone", joint: { pivot: [0, 0.18, 0], axis: [1, 0, 0] } }));
    part(`knee_${side}`, box({ parent: `leg_${side}`, at: [0, -0.16, -0.015], size: [0.2, 0.18, 0.18], material: "bone_light" }));
    part(`foot_${side}`, box({ parent: `leg_${side}`, at: [0, -0.25, -0.06], size: [0.22, 0.1, 0.34], material: "bone_dark" }));
  }
  bipedWalk("graveyard_march", { label: "Graveyard march", fps: 18, duration: 1.08, cycleDistance: 0.7, loop: true, samples: 23, armSwingDegrees: 20, body: "spine", bodyBob: 0.012, bodyBobCenter: 0.014, head: "neck", headSwingDegrees: 4, leftArm: "upper_arm_l", leftContact: "foot_l", leftLeg: "leg_l", rightArm: "upper_arm_r", rightContact: "foot_r", rightLeg: "leg_r", stanceRatio: 0.66, swingDegrees: 19 });
  clip("bone_rattle", { label: "Bone rattle", role: "action", nextClip: "graveyard_march", fps: 30, loop: false, keys: [
    ["spine", 0, { rot: [0, 0, 0] }], ["spine", 0.22, { rot: [0, -9, 0] }], ["spine", 0.38, { rot: [0, 11, 0] }], ["spine", 0.54, { rot: [0, -12, 0] }], ["spine", 0.72, { rot: [0, 9, 0] }], ["spine", 1.05, { rot: [0, 0, 0] }],
    ["neck", 0, { rot: [0, 0, 0] }], ["neck", 0.22, { rot: [0, 14, 7] }], ["neck", 0.38, { rot: [0, -16, -8] }], ["neck", 0.54, { rot: [0, 17, 8] }], ["neck", 0.72, { rot: [0, -12, -6] }], ["neck", 1.05, { rot: [0, 0, 0] }],
    ["jaw", 0, { rot: [0, 0, 0] }], ["jaw", 0.22, { rot: [-24, 0, 0] }], ["jaw", 0.38, { rot: [-5, 0, 0] }], ["jaw", 0.54, { rot: [-28, 0, 0] }], ["jaw", 0.72, { rot: [-4, 0, 0] }], ["jaw", 1.05, { rot: [0, 0, 0] }],
    ["upper_arm_l", 0, { rot: [0, 0, 0] }], ["upper_arm_l", 0.54, { rot: [-18, 0, -12] }], ["upper_arm_l", 1.05, { rot: [0, 0, 0] }], ["upper_arm_r", 0, { rot: [0, 0, 0] }], ["upper_arm_r", 0.54, { rot: [18, 0, 12] }], ["upper_arm_r", 1.05, { rot: [0, 0, 0] }],
  ] });
  defaultClip("graveyard_march");
});
