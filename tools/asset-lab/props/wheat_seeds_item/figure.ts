import { figure } from "../../src/dsl";

// A compact seed pouch with a green crop mark, centered for pickup display.
export default figure("wheat_seeds_item", ({ box, mat, part }) => {
  mat("cloth", "#a77b43");
  mat("cloth_light", "#c69a59");
  mat("cord", "#604321");
  mat("sprout", "#45a83d");

  part("pouch", box({
    at: [0, 0, 0],
    size: [0.34, 0.27, 0.19],
    material: "cloth",
  }));
  part("pouch_face", box({
    at: [0, 0.01, 0.105],
    size: [0.28, 0.21, 0.035],
    material: "cloth_light",
  }));
  part("neck", box({
    at: [0, 0.16, 0],
    size: [0.19, 0.08, 0.15],
    material: "cloth_light",
  }));
  part("cord", box({
    at: [0, 0.13, 0],
    size: [0.23, 0.045, 0.18],
    material: "cord",
  }));
  part("sprout_stem", box({
    at: [0, 0.005, 0.127],
    size: [0.035, 0.13, 0.025],
    material: "sprout",
  }));
  part("sprout_leaf_l", box({
    at: [-0.052, 0.055, 0.127],
    rot: [0, 0, 24],
    size: [0.11, 0.045, 0.025],
    material: "sprout",
  }));
  part("sprout_leaf_r", box({
    at: [0.052, 0.055, 0.127],
    rot: [0, 0, -24],
    size: [0.11, 0.045, 0.025],
    material: "sprout",
  }));
});
