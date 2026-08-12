import { figure } from "../../src/dsl";

// A deliberately simple stone-tipped hunting spear, centered for an item
// entity. The gameplay reach and damage remain explicit server facts.
export default figure("hunting_spear", ({ box, mat, part }) => {
  mat("shaft", "#76512f");
  mat("binding", "#b18a59");
  mat("stone", "#6f7471");
  mat("stone_edge", "#a4aaa4");

  part("shaft", box({
    at: [0, 0.025, 0.08],
    size: [0.055, 0.05, 1.36],
    material: "shaft",
  }));
  part("binding", box({
    at: [0, 0.045, -0.59],
    size: [0.085, 0.06, 0.13],
    material: "binding",
  }));
  part("spearhead_base", box({
    at: [0, 0.033, -0.72],
    size: [0.13, 0.05, 0.18],
    material: "stone",
  }));
  part("spearhead_tip", box({
    at: [0, 0.033, -0.84],
    size: [0.07, 0.04, 0.08],
    material: "stone_edge",
  }));
});
