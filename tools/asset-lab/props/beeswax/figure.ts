import { figure } from "../../src/dsl";

// Three small hand-cut wax cakes with warm comb-colored faces.
export default figure("beeswax", ({ asciiTexture, box, mat, part }) => {
  mat("wax", "#d6a62c");
  mat("wax_light", "#f0cd58");
  mat("wax_shadow", "#a7761e");
  asciiTexture("wax_face", {
    palette: { ".": "#d6a62c", "l": "#f0cd58", "d": "#a7761e" },
    pixels: [
      "ll....ll",
      "l......l",
      "..d..d..",
      ".d....d.",
      "..d..d..",
      "l......l",
    ],
  });

  part("cake_center", box({
    at: [0, 0, 0],
    rot: [0, -8, 0],
    size: [0.54, 0.18, 0.43],
    material: "wax",
    faces: { up: { texture: "wax_face" } },
  }));
  part("cake_left", box({ at: [-0.34, -0.03, 0.12], rot: [0, 18, -3], size: [0.3, 0.14, 0.3], material: "wax_shadow" }));
  part("cake_right", box({ at: [0.33, -0.02, 0.08], rot: [0, -20, 4], size: [0.28, 0.15, 0.28], material: "wax_light" }));
});
