import { figure } from "../../src/dsl";

// A vanilla-shaped second skeleton whose full torso cuboid is carved into a
// rib cage entirely by binary transparent palette texels. The ordinary
// Skeleton keeps its deeper separate-rib construction for direct comparison.
export default figure("cutout_skeleton", ({ asciiTexture, bipedWalk, box, clip, defaultClip, mat, metadata, part }) => {
  metadata({
    bodyPlans: ["biped"],
    disposition: "hostile",
    groups: ["fantasy", "monster", "humanoid"],
    habitats: ["land", "underground"],
    scale: "medium",
    themes: ["alpha-cutout", "scary", "undead", "vanilla-inspired"],
  });
  mat("bone", "#d8d0b5"); mat("bone_light", "#eee8d0"); mat("bone_dark", "#9c947d");
  asciiTexture("skull", { palette: { ".": "#d8d0b5", "l": "#eee8d0", "d": "#9c947d", "v": "#171816" }, pixels: ["llllllll", "l......l", ".vv..vv.", ".vv..vv.", "...vv...", "d......d", "dd.dd.dd"] });
  asciiTexture("jaw", { palette: { ".": "#9c947d", "t": "#f3ecd5", "v": "#171816" }, pixels: ["tttttttt", "tvtvtvtt", ".v.v.v..", "........"] });
  asciiTexture("ribs_front", { palette: { ".": "transparent", "b": "#d8d0b5", "h": "#eee8d0", "d": "#9c947d" }, pixels: ["hhhhhhhh", "...bb...", ".bbbbbb.", ".b.bb.b.", ".hbhhbh.", ".b.bb.b.", ".bbbbbb.", "...dd...", "..dddd..", "..bbbb.."] });
  asciiTexture("ribs_side", { palette: { ".": "transparent", "b": "#d8d0b5", "h": "#eee8d0", "d": "#9c947d" }, pixels: ["hhhh", ".bb.", "bbbb", ".bb.", "hbbh", ".bb.", "bbbb", ".dd.", ".dd.", ".bb."] });
  asciiTexture("ribs_cap", { palette: { ".": "transparent", "b": "#d8d0b5", "h": "#eee8d0" }, pixels: ["..hhhh..", ".bbbbbb.", "..bbbb..", "...bb..."] });

  part("body", box({ at: [0, 0.92, 0], size: [0.68, 0.72, 0.3], material: "bone", faces: {
    north: { texture: "ribs_front" }, south: { texture: "ribs_front" },
    east: { texture: "ribs_side" }, west: { texture: "ribs_side" },
    up: { texture: "ribs_cap" }, down: { texture: "ribs_cap" },
  } }));
  part("pelvis", box({ parent: "body", at: [0, -0.45, 0], size: [0.46, 0.18, 0.28], material: "bone" }));
  part("neck", box({ parent: "body", at: [0, 0.46, 0], size: [0.18, 0.2, 0.18], material: "bone_dark", joint: { pivot: [0, -0.08, 0], axis: [1, 0, 0] } }));
  part("skull", box({ parent: "neck", at: [0, 0.3, -0.01], size: [0.5, 0.5, 0.46], material: "bone", faces: { north: { texture: "skull" } } }));
  part("brow", box({ parent: "skull", at: [0, 0.16, -0.27], size: [0.42, 0.1, 0.12], material: "bone_light" }));
  part("jaw", box({ parent: "skull", at: [0, -0.3, -0.08], size: [0.36, 0.16, 0.34], material: "bone_dark", faces: { north: { texture: "jaw" } }, joint: { pivot: [0, 0.06, 0.13], axis: [1, 0, 0] } }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`arm_${side}`, box({ parent: "body", at: [sign * 0.43, -0.08, 0], size: [0.14, 0.7, 0.15], material: "bone", joint: { pivot: [0, 0.34, 0], axis: [1, 0, 0] } }));
    part(`hand_${side}`, box({ parent: `arm_${side}`, at: [0, -0.4, -0.02], size: [0.19, 0.14, 0.2], material: "bone_dark" }));
    part(`leg_${side}`, box({ parent: "pelvis", at: [sign * 0.14, -0.21, 0], size: [0.15, 0.42, 0.17], material: "bone", joint: { pivot: [0, 0.2, 0], axis: [1, 0, 0] } }));
    part(`foot_${side}`, box({ parent: `leg_${side}`, at: [0, -0.21, -0.06], size: [0.22, 0.1, 0.34], material: "bone_dark" }));
  }
  bipedWalk("hollow_march", { label: "Hollow march", fps: 18, duration: 1.08, cycleDistance: 0.7, loop: true, samples: 23, armSwingDegrees: 20, body: "body", bodyBob: 0.012, bodyBobCenter: 0.014, head: "neck", headSwingDegrees: 4, leftArm: "arm_l", leftContact: "foot_l", leftLeg: "leg_l", rightArm: "arm_r", rightContact: "foot_r", rightLeg: "leg_r", stanceRatio: 0.66, swingDegrees: 19 });
  clip("hollow_rattle", { label: "Hollow rattle", role: "action", nextClip: "hollow_march", fps: 30, loop: false, keys: [
    ["body", 0, { rot: [0, 0, 0] }], ["body", 0.24, { rot: [0, -14, 0] }], ["body", 0.48, { rot: [0, 16, 0] }], ["body", 0.72, { rot: [0, -11, 0] }], ["body", 1.04, { rot: [0, 0, 0] }],
    ["neck", 0, { rot: [0, 0, 0] }], ["neck", 0.24, { rot: [0, 12, 6] }], ["neck", 0.48, { rot: [0, -14, -7] }], ["neck", 0.72, { rot: [0, 10, 5] }], ["neck", 1.04, { rot: [0, 0, 0] }],
    ["jaw", 0, { rot: [0, 0, 0] }], ["jaw", 0.24, { rot: [-22, 0, 0] }], ["jaw", 0.48, { rot: [-5, 0, 0] }], ["jaw", 0.72, { rot: [-24, 0, 0] }], ["jaw", 1.04, { rot: [0, 0, 0] }],
  ] });
  defaultClip("hollow_march");
});
