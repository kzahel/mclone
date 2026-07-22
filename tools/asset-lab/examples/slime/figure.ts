import { figure } from "../../src/dsl";

// A box-only acid slime with a stepped gelatinous silhouette, trapped core,
// grounded squash idle, and a separate airborne slam with a hard landing.
export default figure("slime", ({ asciiTexture, box, clip, defaultClip, mat, metadata, part }) => {
  metadata({
    bodyPlans: ["blob"],
    disposition: "hostile",
    groups: ["fantasy", "monster"],
    habitats: ["land", "underground"],
    scale: "small",
    themes: ["acid", "odd", "scary"],
  });
  mat("gel", { color: "#58a84f", roughness: 0.36 }); mat("gel_light", { color: "#82ca65", roughness: 0.3 }); mat("gel_dark", "#2f6f3e"); mat("core", "#b7d54a"); mat("void", "#18231b");
  asciiTexture("slime_face", { palette: { ".": "#58a84f", "l": "#82ca65", "d": "#2f6f3e", "v": "#18231b" }, pixels: ["llllllllll", "l........l", "l.vv..vv.l", "l.vv..vv.l", "l........l", "l..vvvv..l", "l.v....v.l", "dd......dd"] });
  asciiTexture("mottle", { palette: { ".": "#58a84f", "l": "#82ca65", "d": "#2f6f3e", "c": "#b7d54a" }, pixels: ["ll..cc..ll", ".l......l.", "..dd..dd..", "c..l..l..c", "dddddddddd"] });
  part("body", box({ at: [0, 0.44, 0], size: [0.9, 0.84, 0.9], material: "gel", faces: { north: { texture: "slime_face" }, east: { texture: "mottle" }, west: { texture: "mottle" }, up: { texture: "mottle" } } }));
  part("top_lobe", box({ parent: "body", at: [-0.17, 0.48, 0.08], size: [0.46, 0.14, 0.54], material: "gel_light" }));
  part("crown_drop", box({ parent: "top_lobe", at: [0.12, 0.12, 0.05], size: [0.2, 0.16, 0.28], material: "core" }));
  part("side_lobe_l", box({ parent: "body", at: [-0.49, -0.285, 0.08], size: [0.16, 0.24, 0.52], material: "gel_dark" }));
  part("side_lobe_r", box({ parent: "body", at: [0.49, -0.285, -0.04], size: [0.16, 0.24, 0.46], material: "gel_light" }));
  part("core", box({ parent: "body", at: [0, -0.02, 0.18], size: [0.34, 0.34, 0.3], material: "core" }));
  clip("restless_ooze", { label: "Restless ooze", role: "idle", fps: 24, loop: true, keys: [
    ["body", 0, { at: [0, 0, 0], scale: [1, 1, 1] }], ["body", 0.3, { at: [0, -0.076, 0], scale: [1.08, 0.82, 1.08] }], ["body", 0.6, { at: [0, 0.059, 0], scale: [0.94, 1.14, 0.94] }], ["body", 0.9, { at: [0, -0.055, 0], scale: [1.06, 0.87, 1.06] }], ["body", 1.2, { at: [0, 0, 0], scale: [1, 1, 1] }],
    ["crown_drop", 0, { rot: [0, 0, 0] }], ["crown_drop", 0.3, { rot: [0, 0, -8] }], ["crown_drop", 0.6, { rot: [0, 0, 9] }], ["crown_drop", 0.9, { rot: [0, 0, -6] }], ["crown_drop", 1.2, { rot: [0, 0, 0] }],
  ] });
  clip("acid_slam", { label: "Acid slam", role: "action", nextClip: "restless_ooze", fps: 30, loop: false, keys: [
    ["body", 0, { at: [0, 0, 0], scale: [1, 1, 1] }], ["body", 0.22, { at: [0, -0.118, 0], scale: [1.18, 0.72, 1.18] }], ["body", 0.42, { at: [0, 0.35, -0.08], scale: [0.9, 1.18, 0.9] }], ["body", 0.58, { at: [0, 0.58, -0.18], scale: [0.86, 1.12, 0.86] }], ["body", 0.7, { at: [0, -0.176, -0.28], scale: [1.25, 0.58, 1.25] }], ["body", 0.86, { at: [0, -0.067, -0.18], scale: [1.1, 0.84, 1.1] }], ["body", 1.16, { at: [0, 0, 0], scale: [1, 1, 1] }],
  ] });
  defaultClip("restless_ooze");
});
