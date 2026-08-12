import { figure } from "../../src/dsl";

// A sheltered wild colony in a split fallen branch. The dark cavity and
// exposed comb read from the front while the bark shell keeps it natural.
export default figure("bee_nest", ({ asciiTexture, box, mat, part }) => {
  mat("bark", "#64462d");
  mat("bark_light", "#8a6240");
  mat("heartwood", "#b08351");
  mat("cavity", "#211910");
  mat("comb", "#d5a72d");
  mat("comb_light", "#f0cf58");

  asciiTexture("bark_stripe", {
    palette: { ".": "#64462d", "l": "#8a6240", "d": "#49311f" },
    pixels: [
      "ll...d..ll",
      "..dd...l..",
      "l...ll...d",
      "..l...dd..",
      "dd...l...l",
      "...ll..d..",
    ],
  });
  asciiTexture("honeycomb", {
    palette: { ".": "#d5a72d", "l": "#f0cf58", "d": "#9a6e1e" },
    pixels: [
      ".l..l..l",
      "l.dd.dd.",
      ".d..d..l",
      "l.dd.dd.",
      ".l..l..l",
      "..dd.dd.",
    ],
  });

  part("log_floor", box({
    at: [0, 0.12, 0],
    size: [1.02, 0.24, 0.72],
    material: "bark",
    faces: { up: { texture: "bark_stripe" } },
  }));
  part("log_roof", box({
    at: [0, 0.7, 0.02],
    size: [1.01, 0.2, 0.76],
    material: "bark",
    faces: { up: { texture: "bark_stripe" } },
  }));
  part("log_side_l", box({
    at: [-0.43, 0.41, 0.02],
    size: [0.19, 0.48, 0.74],
    material: "bark_light",
    faces: { west: { texture: "bark_stripe" } },
  }));
  part("log_side_r", box({
    at: [0.43, 0.41, 0.02],
    size: [0.19, 0.48, 0.74],
    material: "bark_light",
    faces: { east: { texture: "bark_stripe" } },
  }));
  part("cavity_back", box({
    at: [0, 0.42, 0.29],
    size: [0.68, 0.39, 0.08],
    material: "cavity",
  }));
  part("exposed_comb", box({
    at: [0, 0.44, -0.325],
    size: [0.5, 0.31, 0.055],
    material: "comb",
    faces: { north: { texture: "honeycomb" } },
  }));
  part("broken_end_l", box({
    at: [-0.54, 0.38, 0.18],
    rot: [0, 0, 13],
    size: [0.12, 0.5, 0.32],
    material: "heartwood",
  }));
  part("comb_glint", box({
    at: [0.11, 0.48, -0.36],
    size: [0.12, 0.09, 0.025],
    material: "comb_light",
  }));
});
