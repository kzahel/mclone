import { figure } from "../../src/dsl";

// A restrained, non-graphic ecology marker: leaf litter, a little pale
// weathered material, and no identifiable carcass anatomy.
export default figure("wildlife_remains", ({ box, mat, part }) => {
  mat("leaf_dark", "#55452d");
  mat("leaf_dry", "#806744");
  mat("weathered", "#b8ad8c");

  part("litter_center", box({
    at: [0, 0.035, 0],
    size: [0.68, 0.07, 0.78],
    material: "leaf_dark",
  }));
  for (const [name, at, rot, size] of [
    ["leaf_north", [-0.16, 0.07, -0.35], [0, -18, 4], [0.38, 0.035, 0.17]],
    ["leaf_south", [0.2, 0.065, 0.34], [0, 24, -3], [0.42, 0.04, 0.16]],
    ["leaf_west", [-0.34, 0.06, 0.04], [0, 67, 2], [0.34, 0.035, 0.15]],
    ["leaf_east", [0.34, 0.075, -0.08], [0, -63, -2], [0.36, 0.04, 0.14]],
  ] as const) {
    part(name, box({ at, rot, size, material: "leaf_dry" }));
  }
  part("weathered_stem_a", box({
    at: [-0.08, 0.11, 0.02],
    rot: [2, -31, 5],
    size: [0.055, 0.055, 0.56],
    material: "weathered",
  }));
  part("weathered_stem_b", box({
    at: [0.1, 0.115, 0.0],
    rot: [-3, 28, -4],
    size: [0.045, 0.05, 0.42],
    material: "weathered",
  }));
});
