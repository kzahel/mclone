import { figure } from "../../src/dsl";

// A box-only ocean sunfish with a tall disk body, tiny mouth, tall dorsal and
// anal fins, scalloped clavus, gentle sculling swim, and a basking roll.
export default figure("ocean_sunfish", ({ asciiTexture, box, clip, defaultClip, mat, part, swim }) => {
  mat("silver", "#9ca9a5"); mat("silver_light", "#c6cfca"); mat("silver_dark", "#657572"); mat("fin", "#556b69"); mat("eye", "#18201f"); mat("mouth", "#5f5550");
  asciiTexture("mottle", { palette: { ".": "#9ca9a5", "l": "#c6cfca", "d": "#657572" }, pixels: ["..ll..dd..", ".d..ll..d.", "dd......ll", "..d.ll.d..", "ll..dd..ll", ".........."] });
  asciiTexture("face", { palette: { ".": "#c6cfca", "d": "#657572", "e": "#18201f" }, pixels: ["dddddddd", "d.e....d", "d.e....d", "d......d", "........", "dddddddd"] });
  part("body", box({ at: [0, 1.05, 0.04], size: [0.42, 1.3, 1.15], material: "silver", faces: { east: { texture: "mottle" }, west: { texture: "mottle" } } }));
  part("belly", box({ parent: "body", at: [0, -0.47, -0.08], size: [0.45, 0.34, 0.82], material: "silver_light" }));
  part("face", box({ parent: "body", at: [0, 0.03, -0.65], size: [0.44, 0.7, 0.24], material: "silver_light", faces: { north: { texture: "face" } } }));
  part("mouth", box({ parent: "face", at: [0, -0.12, -0.19], size: [0.24, 0.17, 0.15], material: "mouth" }));
  for (const [side, sign] of [["l", -1], ["r", 1]] as const) part(`pectoral_${side}`, box({ parent: "body", at: [sign * 0.29, -0.02, -0.2], rot: [0, sign * -12, sign * 8], size: [0.38, 0.07, 0.34], material: "fin", joint: { pivot: [sign * -0.17, 0, -0.08], axis: [0, 0, 1] } }));
  part("dorsal", box({ parent: "body", at: [0, 0.91, 0.18], rot: [-6, 0, 0], size: [0.1, 0.72, 0.4], material: "fin", joint: { pivot: [0, -0.31, -0.08], axis: [0, 1, 0] } }));
  part("anal", box({ parent: "body", at: [0, -0.91, 0.18], rot: [6, 0, 0], size: [0.1, 0.72, 0.4], material: "fin", joint: { pivot: [0, 0.31, -0.08], axis: [0, 1, 0] } }));
  part("tail_base", box({ parent: "body", at: [0, 0, 0.68], size: [0.26, 0.82, 0.22], material: "silver_dark", joint: { pivot: [0, 0, -0.1], axis: [0, 1, 0] } }));
  for (const [index, y, h, width, z] of [[1, 0.34, 0.32, 0.12, 0.19], [2, 0.1, 0.3, 0.15, 0.21], [3, -0.14, 0.3, 0.12, 0.23], [4, -0.38, 0.28, 0.15, 0.25]] as const) part(`clavus_${index}`, box({ parent: "tail_base", at: [0, y, z], size: [width, h, 0.34], material: index % 2 ? "fin" : "silver_dark" }));
  swim("open_ocean_swim", { label: "Open-ocean scull", fps: 20, duration: 1.22, cycleDistance: 0.76, loop: true, samples: 23, body: "body", bodyBob: 0.025, bodySwayDegrees: 2, finSwingDegrees: 13, leftFin: "pectoral_l", rightFin: "pectoral_r", tail: "tail_base", tailSwingDegrees: 5 });
  clip("surface_bask", { label: "Surface bask", role: "action", nextClip: "open_ocean_swim", fps: 30, loop: false, keys: [
    ["body", 0, { rot: [0, 0, 0] }], ["body", 0.3, { rot: [0, 0, 28] }], ["body", 0.66, { rot: [0, 0, 72] }], ["body", 0.96, { rot: [0, 0, 28] }], ["body", 1.3, { rot: [0, 0, 0] }],
    ["dorsal", 0, { rot: [0, 0, 0] }], ["dorsal", 0.66, { rot: [0, 9, 0] }], ["dorsal", 1.3, { rot: [0, 0, 0] }],
  ] });
  defaultClip("open_ocean_swim");
});
