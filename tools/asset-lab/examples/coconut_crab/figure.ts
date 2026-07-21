import { figure, type CycleTrack, type LocomotionContactSpec } from "../../src/dsl";

// A box-only coconut crab with a massive blue-violet body, raised eye stalks,
// unequal crushing claws, eight jointed legs, lateral walk, and nut crack.
export default figure("coconut_crab", ({ asciiTexture, bob, box, clip, contactSwing, defaultClip, mat, part, swing, walkCycle }) => {
  mat("shell", "#4f6280"); mat("shell_light", "#7382a0"); mat("shell_dark", "#323f58"); mat("violet", "#645476"); mat("joint", "#5b4c68"); mat("leg", "#59657b"); mat("tip", "#303747"); mat("eye", "#14171a");
  asciiTexture("carapace", { palette: { ".": "#4f6280", "l": "#7382a0", "d": "#323f58", "v": "#645476" }, pixels: ["dddddddddddd", "dll......lld", "d..vv..vv..d", "d.v..vv..v.d", "dll......lld", "dddddddddddd"] });
  part("carapace", box({ at: [0, 0.84, 0.08], size: [1.22, 0.56, 0.9], material: "shell", faces: { up: { texture: "carapace" } } }));
  part("abdomen", box({ parent: "carapace", at: [0, -0.1, 0.52], size: [0.82, 0.44, 0.36], material: "violet" }));
  part("face", box({ parent: "carapace", at: [0, -0.03, -0.53], size: [0.84, 0.32, 0.24], material: "shell_light" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_stalk_${side}`, box({ parent: "carapace", at: [sign * 0.28, 0.39, -0.35], rot: [-10, 0, sign * -6], size: [0.08, 0.3, 0.08], material: "joint", joint: { pivot: [0, -0.13, 0], axis: [0, 0, 1] } }));
    part(`eye_${side}`, box({ parent: `eye_stalk_${side}`, at: [0, 0.19, -0.02], size: [0.15, 0.14, 0.14], material: "eye" }));
    const palm = side === "l" ? 0.48 : 0.38;
    part(`claw_arm_${side}`, box({ parent: "carapace", at: [sign * 0.68, -0.03, -0.4], rot: [0, sign * 20, sign * -7], size: [0.54, 0.16, 0.16], material: "joint", joint: { pivot: [sign * -0.25, 0, 0], axis: [0, 1, 0] } }));
    part(`claw_palm_${side}`, box({ parent: `claw_arm_${side}`, at: [sign * 0.43, 0.03, -0.05], size: [palm, side === "l" ? 0.38 : 0.32, 0.4], material: side === "l" ? "shell_light" : "shell" }));
    part(`claw_outer_${side}`, box({ parent: `claw_palm_${side}`, at: [sign * 0.12, 0.06, -0.33], rot: [0, sign * 8, 0], size: [0.16, 0.17, 0.42], material: "shell_light", joint: { pivot: [0, 0, 0.18], axis: [0, 1, 0] } }));
    part(`claw_inner_${side}`, box({ parent: `claw_palm_${side}`, at: [sign * -0.11, -0.05, -0.31], rot: [0, sign * -9, 0], size: [0.14, 0.14, 0.38], material: "shell_dark", joint: { pivot: [0, 0, 0.16], axis: [0, 1, 0] } }));
  }
  const rows = [["front", -0.28, 20], ["front_mid", -0.08, 7], ["rear_mid", 0.13, -7], ["rear", 0.32, -20]] as const;
  for (const [row, z, yaw] of rows) for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`leg_${row}_${side}`, box({ parent: "carapace", at: [sign * 0.73, -0.22, z], rot: [0, sign * yaw, -sign * 14], size: [0.58, 0.09, 0.11], material: "leg", joint: { pivot: [sign * -0.27, 0, 0], axis: [0, 0, 1] } }));
    part(`shin_${row}_${side}`, box({ parent: `leg_${row}_${side}`, at: [sign * 0.47, -0.1, 0], rot: [0, 0, -sign * 13], size: [0.43, 0.075, 0.09], material: "shell_dark" }));
    part(`foot_${row}_${side}`, box({ parent: `shin_${row}_${side}`, at: [sign * 0.3, -0.06, 0], size: [0.26, 0.055, 0.15], material: "tip" }));
  }
  const contacts: LocomotionContactSpec[] = []; const tracks: CycleTrack[] = [bob("carapace", { axis: "y", amount: 0.014, center: 0.016, phase: 0.5 }), swing("carapace", { axis: "z", degrees: 2, phase: 0.25 })]; const stanceRatio = 0.66;
  rows.forEach(([row], rowIndex) => { const leftPhase = rowIndex * 0.17; for (const [side, phase, degrees] of [["l", leftPhase, 13], ["r", (leftPhase + 0.5) % 1, -13]] as const) { contacts.push({ part: `foot_${row}_${side}`, phaseStart: phase, phaseEnd: (phase + stanceRatio) % 1, role: `${row}-${side}`, stanceRatio }); tracks.push(contactSwing(`leg_${row}_${side}`, { axis: "z", degrees, phase, stanceRatio })); } });
  walkCycle("island_scuttle", { label: "Island scuttle", role: "locomotion", fps: 20, duration: 1.02, loop: true, samples: 23, locomotion: { kind: "quadruped-walk", cycleDistance: 0.56, direction: [1, 0, 0], units: "figure", contacts }, tracks });
  clip("coconut_crack", { label: "Coconut crack", role: "action", nextClip: "island_scuttle", fps: 30, loop: false, keys: [
    ["claw_arm_l", 0, { rot: [0, 0, 0] }], ["claw_arm_l", 0.32, { rot: [0, -25, 0] }], ["claw_arm_l", 0.62, { rot: [0, -38, 0] }], ["claw_arm_l", 1.06, { rot: [0, 0, 0] }],
    ["claw_outer_l", 0, { rot: [0, 0, 0] }], ["claw_outer_l", 0.32, { rot: [0, 24, 0] }], ["claw_outer_l", 0.62, { rot: [0, 42, 0] }], ["claw_outer_l", 1.06, { rot: [0, 0, 0] }], ["claw_inner_l", 0, { rot: [0, 0, 0] }], ["claw_inner_l", 0.32, { rot: [0, -24, 0] }], ["claw_inner_l", 0.62, { rot: [0, -42, 0] }], ["claw_inner_l", 1.06, { rot: [0, 0, 0] }],
  ] });
  defaultClip("island_scuttle");
});
