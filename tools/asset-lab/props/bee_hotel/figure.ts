import { figure } from "../../src/dsl";

// A deliberately built pollinator shelter with a rain cap, timber frame, and
// dark nesting holes. Ground anchoring makes it a readable placed habitat.
export default figure("bee_hotel", ({ asciiTexture, box, mat, part }) => {
  mat("post", "#6c4728");
  mat("wood", "#b48149");
  mat("wood_light", "#d0a365");
  mat("roof", "#755239");
  mat("hole", "#281e16");

  asciiTexture("nesting_holes", {
    palette: { ".": "#b48149", "o": "#281e16", "l": "#d0a365" },
    pixels: [
      "l........l",
      ".oo.oo.oo.",
      ".oo.oo.oo.",
      "...oo.oo..",
      "...oo.oo..",
      ".oo.oo.oo.",
      ".oo.oo.oo.",
      "l........l",
    ],
  });

  part("post_l", box({ at: [-0.35, 0.47, 0.08], size: [0.13, 0.94, 0.15], material: "post" }));
  part("post_r", box({ at: [0.35, 0.47, 0.08], size: [0.13, 0.94, 0.15], material: "post" }));
  part("hotel_body", box({
    at: [0, 0.88, 0],
    size: [0.86, 0.68, 0.42],
    material: "wood",
    faces: { north: { texture: "nesting_holes" } },
  }));
  part("inner_shelf", box({ at: [0, 0.88, -0.23], size: [0.74, 0.07, 0.05], material: "wood_light" }));
  part("roof", box({ at: [0, 1.27, 0], rot: [0, 0, -4], size: [1.02, 0.13, 0.58], material: "roof" }));
  part("foot_l", box({ at: [-0.35, 0.055, 0.08], size: [0.28, 0.11, 0.3], material: "post" }));
  part("foot_r", box({ at: [0.35, 0.055, 0.08], size: [0.28, 0.11, 0.3], material: "post" }));
});
