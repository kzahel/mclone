import { figure } from "../../src/dsl";

// A box-only Texas horned lizard with broad flat spiny body, crown horns,
// mottled desert scales, short sprawled legs, sand scuttle, and body puff.
export default figure("texas_horned_lizard", ({ asciiTexture, bob, box, clip, defaultClip, mat, part, swing, walkCycle }) => {
  mat("sand", "#a67d55"); mat("sand_light", "#c5a173"); mat("sand_dark", "#684e3d"); mat("cream", "#ddc597"); mat("horn", "#e0c58d"); mat("eye", "#1b1713"); mat("toe", "#514137");
  asciiTexture("scales", { palette: { ".": "#a67d55", "l": "#c5a173", "d": "#684e3d", "c": "#ddc597" }, pixels: ["dd..ll..dd..", "d..dd..dd..d", "..c..cc..c..", ".dd......dd.", "ll..dd..ll.."] });
  asciiTexture("face", { palette: { ".": "#c5a173", "d": "#684e3d", "e": "#1b1713" }, pixels: ["dd......dd", "d.e....e.d", "..dddddd..", ".........."] });
  part("body", box({ at: [0, 0.38, 0.06], size: [0.94, 0.26, 1.12], material: "sand", faces: { up: { texture: "scales" } } }));
  part("back_plate", box({ parent: "body", at: [0, 0.17, 0.02], size: [0.68, 0.12, 0.82], material: "sand_dark", faces: { up: { texture: "scales" } } }));
  part("belly", box({ parent: "body", at: [0, -0.17, -0.04], size: [0.72, 0.09, 0.84], material: "cream" }));
  for (const [index, x, z] of [[1, -0.54, -0.34], [2, 0.54, -0.34], [3, -0.57, 0], [4, 0.57, 0], [5, -0.52, 0.36], [6, 0.52, 0.36]] as const) part(`side_spike_${index}`, box({ parent: "body", at: [x, 0.08, z], rot: [0, 0, index % 2 ? 24 : -24], size: [0.22, 0.07, 0.1], material: "horn" }));
  part("neck", box({ parent: "body", at: [0, 0.05, -0.66], size: [0.62, 0.3, 0.34], material: "sand_dark", joint: { pivot: [0, 0, 0.14], axis: [0, 1, 0] } }));
  part("head", box({ parent: "neck", at: [0, 0.05, -0.34], size: [0.76, 0.34, 0.48], material: "sand_light", faces: { north: { texture: "face" } } }));
  part("snout", box({ parent: "head", at: [0, -0.04, -0.32], size: [0.56, 0.22, 0.22], material: "sand_light" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`brow_horn_${side}`, box({ parent: "head", at: [sign * 0.24, 0.24, -0.04], rot: [-18, 0, sign * -14], size: [0.09, 0.34, 0.09], material: "horn" }));
    part(`crown_horn_${side}`, box({ parent: "head", at: [sign * 0.17, 0.23, 0.2], rot: [28, 0, sign * -8], size: [0.1, 0.42, 0.1], material: "horn" }));
  }
  for (const [row, z, yaw] of [["front", -0.35, 13], ["rear", 0.36, -13]] as const) for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`leg_${row}_${side}`, box({ parent: "body", at: [sign * 0.52, -0.12, z], rot: [0, sign * -yaw, sign * 10], size: [0.42, 0.1, 0.15], material: "sand_dark", joint: { pivot: [sign * -0.19, 0, 0], axis: [0, 1, 0] } }));
    part(`foot_${row}_${side}`, box({ parent: `leg_${row}_${side}`, at: [sign * 0.31, -0.1, 0], rot: [0, 0, sign * 18], size: [0.3, 0.07, 0.24], material: "toe" }));
  }
  part("tail_1", box({ parent: "body", at: [0, 0, 0.75], size: [0.5, 0.22, 0.55], material: "sand", joint: { pivot: [0, 0, -0.25], axis: [0, 1, 0] } }));
  part("tail_2", box({ parent: "tail_1", at: [0, 0, 0.46], size: [0.32, 0.16, 0.46], material: "sand_light", joint: { pivot: [0, 0, -0.21], axis: [0, 1, 0] } }));
  part("tail_tip", box({ parent: "tail_2", at: [0, 0, 0.38], size: [0.16, 0.11, 0.36], material: "sand_dark" }));
  walkCycle("desert_scuttle", { label: "Desert scuttle", role: "locomotion", fps: 22, duration: 0.82, loop: true, samples: 25, locomotion: { kind: "quadruped-walk", cycleDistance: 0.54, direction: [0, 0, -1], units: "figure" }, tracks: [bob("body", { axis: "y", amount: 0.007, center: 0.009, phase: 0.5 }), swing("neck", { axis: "y", degrees: 3, phase: 0.5 }), swing("leg_front_l", { axis: "y", degrees: 15, phase: 0 }), swing("leg_rear_r", { axis: "y", degrees: -15, phase: 0 }), swing("leg_front_r", { axis: "y", degrees: -15, phase: 0.5 }), swing("leg_rear_l", { axis: "y", degrees: 15, phase: 0.5 }), swing("tail_1", { axis: "y", degrees: 6, phase: 0.1 }), swing("tail_2", { axis: "y", degrees: 10, phase: 0.2 })] });
  clip("defensive_puff", { label: "Defensive puff", role: "action", nextClip: "desert_scuttle", fps: 30, loop: false, keys: [
    ["body", 0, { scale: [1, 1, 1] }], ["body", 0.28, { scale: [1.15, 1.18, 1.08] }], ["body", 0.68, { scale: [1.24, 1.28, 1.12] }], ["body", 1.16, { scale: [1, 1, 1] }],
    ["head", 0, { rot: [0, 0, 0] }], ["head", 0.4, { rot: [-8, 0, 0] }], ["head", 0.68, { rot: [-11, 0, 0] }], ["head", 1.16, { rot: [0, 0, 0] }],
  ] });
  defaultClip("desert_scuttle");
});
