import { figure, type CycleTrack, type LocomotionContactSpec } from "../../src/dsl";

// A box-only Atlantic horseshoe crab with a broad domed shield, compound eyes,
// eight articulated walking legs, book gills, long telson, and seafloor gait.
export default figure("horseshoe_crab", ({ asciiTexture, bob, box, clip, contactSwing, defaultClip, mat, part, swing, walkCycle }) => {
  mat("shell", "#5d4935"); mat("shell_light", "#806747"); mat("shell_dark", "#342d26"); mat("rim", "#9a8059"); mat("leg", "#ad9268"); mat("gill", "#c0aa81"); mat("eye", "#171816");
  asciiTexture("shield", { palette: { ".": "#5d4935", "l": "#806747", "d": "#342d26", "r": "#9a8059" }, pixels: ["dddddddddddd", "drrllllllrrd", "dr........rd", "d..l....l..d", "d...llll...d", "dd........dd", "dddddddddddd"] });
  asciiTexture("abdomen", { palette: { ".": "#5d4935", "l": "#806747", "d": "#342d26" }, pixels: ["dddddddddd", "dlllllllld", "d..d..d..d", ".d..dd..d.", "dddddddddd"] });
  part("shield", box({ at: [0, 0.33, -0.12], size: [1.3, 0.34, 0.92], material: "shell", faces: { up: { texture: "shield" } } }));
  part("brow", box({ parent: "shield", at: [0, 0.19, -0.18], size: [0.92, 0.16, 0.52], material: "shell_light" }));
  part("front_rim", box({ parent: "shield", at: [0, -0.03, -0.52], size: [1.12, 0.2, 0.18], material: "rim" }));
  part("abdomen", box({ parent: "shield", at: [0, -0.025, 0.52], size: [0.9, 0.26, 0.42], material: "shell_dark", faces: { up: { texture: "abdomen" } }, joint: { pivot: [0, 0, -0.18], axis: [1, 0, 0] } }));
  part("rear_spines", box({ parent: "abdomen", at: [0, 0.02, 0.27], size: [0.72, 0.18, 0.18], material: "rim" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`eye_${side}`, box({ parent: "brow", at: [sign * 0.32, 0.12, -0.05], size: [0.12, 0.08, 0.2], material: "eye" }));
    part(`side_spine_${side}`, box({ parent: "abdomen", at: [sign * 0.53, 0, 0.04], rot: [0, 0, sign * -16], size: [0.28, 0.1, 0.12], material: "rim" }));
  }
  const rows = [["front", -0.28], ["front_mid", -0.08], ["rear_mid", 0.12], ["rear", 0.29]] as const;
  for (const [row, z] of rows) for (const [side, sign] of [["l", -1], ["r", 1]] as const) {
    part(`leg_${row}_${side}`, box({ parent: "shield", at: [sign * 0.54, -0.22, z], rot: [0, sign * (row === "front" ? 18 : row === "rear" ? -16 : 0), 0], size: [0.5, 0.08, 0.1], material: "leg", joint: { pivot: [sign * -0.23, 0, 0], axis: [0, 1, 0] } }));
    part(`foot_${row}_${side}`, box({ parent: `leg_${row}_${side}`, at: [sign * 0.38, -0.075, 0], size: [0.32, 0.07, 0.09], material: "leg" }));
  }
  for (const [index, z] of [[1, 0.02], [2, 0.14], [3, 0.26]] as const) part(`book_gill_${index}`, box({ parent: "abdomen", at: [0, -0.16, z - 0.2], size: [0.5 - index * 0.05, 0.04, 0.1], material: "gill" }));
  part("telson_1", box({ parent: "abdomen", at: [0, 0.02, 0.44], rot: [-6, 0, 0], size: [0.14, 0.14, 0.72], material: "shell_dark", joint: { pivot: [0, 0, -0.34], axis: [1, 0, 0] } }));
  part("telson_2", box({ parent: "telson_1", at: [0, 0.02, 0.52], rot: [-4, 0, 0], size: [0.08, 0.08, 0.44], material: "rim" }));
  const contacts: LocomotionContactSpec[] = [];
  const tracks: CycleTrack[] = [bob("shield", { axis: "y", amount: 0.008, center: 0.008, phase: 0.5 }), swing("abdomen", { axis: "y", degrees: 2.2, phase: 0.25 }), swing("telson_1", { axis: "y", degrees: 5, phase: 0.6 })];
  for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
    const [row] = rows[rowIndex]!;
    for (const [side, sign, phase] of [["l", -1, rowIndex * 0.16], ["r", 1, (rowIndex * 0.16 + 0.5) % 1]] as const) {
      contacts.push({ part: `foot_${row}_${side}`, phaseStart: phase, phaseEnd: (phase + 0.68) % 1, role: `${row}-${side}`, stanceRatio: 0.68 });
      tracks.push(contactSwing(`leg_${row}_${side}`, { axis: "y", degrees: sign * 13, phase, stanceRatio: 0.68 }));
    }
  }
  walkCycle("seafloor_scuttle", { label: "Seafloor scuttle", role: "locomotion", fps: 20, duration: 1.08, loop: true, samples: 23, locomotion: { kind: "quadruped-walk", cycleDistance: 0.48, direction: [0, 0, -1], units: "figure", contacts }, tracks });
  clip("telson_lift", { label: "Telson lift", role: "action", nextClip: "seafloor_scuttle", fps: 30, loop: false, keys: [
    ["telson_1", 0, { rot: [0, 0, 0] }], ["telson_1", 0.32, { rot: [-22, 0, 0] }], ["telson_1", 0.66, { rot: [-34, 0, 0] }], ["telson_1", 1.1, { rot: [0, 0, 0] }],
    ["telson_2", 0, { rot: [0, 0, 0] }], ["telson_2", 0.42, { rot: [-14, 0, 0] }], ["telson_2", 0.72, { rot: [-20, 0, 0] }], ["telson_2", 1.1, { rot: [0, 0, 0] }],
    ["abdomen", 0, { rot: [0, 0, 0] }], ["abdomen", 0.66, { rot: [-4, 0, 0] }], ["abdomen", 1.1, { rot: [0, 0, 0] }],
  ] });
  defaultClip("seafloor_scuttle");
});
