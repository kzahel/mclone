import { figure } from "../../src/dsl";

// A low woven bank nest with a soft central cup and one readable mallard egg.
// The ground anchor is the underside of base at y=0.
export default figure("mallard_nest", ({ asciiTexture, box, mat, part }) => {
  mat("reed_dark", "#60452b");
  mat("reed", "#947044");
  mat("reed_light", "#b2915c");
  mat("lining", "#746642");
  mat("egg_shadow", "#a9ab8c");
  mat("egg", "#d9d9b4");
  mat("egg_light", "#ededce");

  asciiTexture("woven_top", {
    palette: { "d": "#60452b", "r": "#947044", "l": "#b2915c" },
    pixels: [
      "drldrldr",
      "rldrldrl",
      "ldrldrld",
      "drldrldr",
      "rldrldrl",
      "ldrldrld",
    ],
  });

  part("woven_base", box({
    at: [0, 0.04, 0],
    size: [0.9, 0.08, 0.7],
    material: "reed_dark",
    faces: { up: { texture: "woven_top" } },
  }));
  part("soft_lining", box({
    at: [0, 0.095, 0],
    size: [0.58, 0.07, 0.42],
    material: "lining",
  }));

  for (const [name, at, rot, size, material] of [
    ["rim_north", [0, 0.135, -0.31], [0, 0, 0], [0.48, 0.13, 0.12], "reed"],
    ["rim_south", [0, 0.145, 0.31], [0, 0, 0], [0.48, 0.15, 0.12], "reed_light"],
    ["rim_west", [-0.39, 0.13, 0], [0, 0, 0], [0.1, 0.12, 0.34], "reed_light"],
    ["rim_east", [0.39, 0.15, 0], [0, 0, 0], [0.1, 0.16, 0.34], "reed"],
    ["rim_north_west", [-0.29, 0.15, -0.24], [0, -43, 0], [0.36, 0.12, 0.11], "reed_dark"],
    ["rim_north_east", [0.29, 0.14, -0.24], [0, 43, 0], [0.36, 0.11, 0.11], "reed_light"],
    ["rim_south_west", [-0.29, 0.16, 0.24], [0, 43, 0], [0.36, 0.14, 0.11], "reed"],
    ["rim_south_east", [0.29, 0.145, 0.24], [0, -43, 0], [0.36, 0.12, 0.11], "reed_dark"],
  ] as const) {
    part(name, box({ at, rot, size, material }));
  }

  part("crossed_reed_a", box({
    at: [-0.22, 0.215, -0.29],
    rot: [0, -18, 4],
    size: [0.42, 0.055, 0.07],
    material: "reed_light",
  }));
  part("crossed_reed_b", box({
    at: [0.22, 0.225, 0.29],
    rot: [0, 16, -3],
    size: [0.42, 0.05, 0.07],
    material: "reed_dark",
  }));

  part("egg_base", box({
    at: [0.04, 0.17, -0.015],
    rot: [0, -8, 0],
    size: [0.2, 0.14, 0.18],
    material: "egg_shadow",
  }));
  part("egg_middle", box({
    at: [0.04, 0.265, -0.015],
    rot: [0, -8, 0],
    size: [0.25, 0.17, 0.22],
    material: "egg",
  }));
  part("egg_crown", box({
    at: [0.04, 0.375, -0.015],
    rot: [0, -8, 0],
    size: [0.15, 0.09, 0.14],
    material: "egg_light",
  }));
});
