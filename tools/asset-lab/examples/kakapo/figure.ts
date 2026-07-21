import { figure } from "../../src/dsl";

// A box-only kakapo with moss-green plumage, owl-like facial disc, stout feet,
// tiny wings, slow waddle, and a separate inflated booming display.
export default figure("kakapo", ({ asciiTexture, bipedWalk, box, clip, defaultClip, followThrough, mat, part, swing }) => {
  mat("green", "#657641"); mat("green_light", "#89955b"); mat("green_dark", "#3d4f31"); mat("face", "#c2bd82"); mat("beak", "#d8c9a2"); mat("eye", "#242119"); mat("foot", "#8b7d65");
  asciiTexture("plumes", { palette: { ".": "#657641", "l": "#89955b", "d": "#3d4f31" }, pixels: ["d.ld.ld.ld", ".d.ld.ld.l", "l.dl.dl.d.", ".........."] });
  asciiTexture("face_disc", { palette: { ".": "#c2bd82", "g": "#657641", "e": "#242119" }, pixels: ["gg....gg", "g.e..e.g", "..eeee..", ".gg..gg.", "........"] });
  part("body", box({ at: [0, 0.72, 0.04], size: [0.78, 0.72, 0.9], material: "green", faces: { east: { texture: "plumes" }, west: { texture: "plumes" } } }));
  part("breast", box({ parent: "body", at: [0, -0.02, -0.5], size: [0.62, 0.58, 0.18], material: "green_light" }));
  part("throat", box({ parent: "body", at: [0, 0.28, -0.44], size: [0.54, 0.38, 0.26], material: "green_light" }));
  part("head", box({ parent: "throat", at: [0, 0.28, -0.08], size: [0.64, 0.54, 0.5], material: "face", faces: { north: { texture: "face_disc" } }, joint: { pivot: [0, -0.22, 0.12], axis: [1, 0, 0] } }));
  part("beak", box({ parent: "head", at: [0, -0.12, -0.34], size: [0.3, 0.2, 0.2], material: "beak" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`wing_${side}`, box({ parent: "body", at: [sign * 0.43, 0.01, 0.03], size: [0.12, 0.5, 0.68], material: "green_dark", faces: { [side === "l" ? "west" : "east"]: { texture: "plumes" } }, joint: { pivot: [sign * -0.04, 0.16, -0.2], axis: [0, 0, 1] } }));
    part(`leg_${side}`, box({ parent: "body", at: [sign * 0.2, -0.48, 0.02], size: [0.14, 0.3, 0.15], material: "foot", joint: { pivot: [0, 0.14, 0], axis: [1, 0, 0] } }));
    part(`foot_${side}`, box({ parent: `leg_${side}`, at: [0, -0.2, -0.11], size: [0.32, 0.11, 0.42], material: "foot" }));
  }
  part("tail", box({ parent: "body", at: [0, 0.02, 0.58], rot: [12, 0, 0], size: [0.52, 0.14, 0.48], material: "green_dark", joint: { pivot: [0, 0, -0.21], axis: [1, 0, 0] } }));
  bipedWalk("moss_waddle", { label: "Moss waddle", fps: 17, duration: 1.12, cycleDistance: 0.42, loop: true, samples: 19, body: "body", bodyBob: 0.016, bodyBobCenter: 0.019, head: "head", headSwingDegrees: 2.5, leftLeg: "leg_l", rightLeg: "leg_r", leftContact: "foot_l", rightContact: "foot_r", stanceRatio: 0.72, swingDegrees: 14,
    tracks: [swing("body", { axis: "z", degrees: 3.5, phase: 0.25 }), swing("wing_l", { axis: "z", degrees: 3, center: -1.5, frequency: 2 }), swing("wing_r", { axis: "z", degrees: -3, center: 1.5, frequency: 2 }), followThrough("head", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 4, overshoot: 0.5, lag: 0.11 })] });
  clip("boom_display", { label: "Boom display", role: "action", nextClip: "moss_waddle", fps: 30, loop: false, keys: [
    ["throat", 0, { scale: [1, 1, 1] }], ["throat", 0.38, { scale: [1, 1.28, 1.45] }], ["throat", 0.72, { scale: [1, 1.34, 1.52] }], ["throat", 1.12, { scale: [1, 1, 1] }],
    ["head", 0, { rot: [0, 0, 0] }], ["head", 0.52, { rot: [-8, 0, 0] }], ["head", 1.12, { rot: [0, 0, 0] }],
    ["wing_l", 0, { rot: [0, 0, 0] }], ["wing_l", 0.52, { rot: [0, 0, -12] }], ["wing_l", 1.12, { rot: [0, 0, 0] }],
    ["wing_r", 0, { rot: [0, 0, 0] }], ["wing_r", 0.52, { rot: [0, 0, 12] }], ["wing_r", 1.12, { rot: [0, 0, 0] }],
  ] });
  defaultClip("moss_waddle");
});
