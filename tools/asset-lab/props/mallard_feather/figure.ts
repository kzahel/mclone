import { figure } from "../../src/dsl";

// A centered, top-readable wing feather with a pale quill, blue speculum
// accent, and asymmetric vane. It is an item, so its origin is its center.
export default figure("mallard_feather", ({ box, mat, part }) => {
  mat("quill", "#e2d8bd");
  mat("vane_dark", "#424a4d");
  mat("vane", "#777d7d");
  mat("speculum", "#315fa5");
  mat("edge", "#eee9dc");

  part("quill", box({
    at: [0, 0.018, 0.02],
    size: [0.035, 0.036, 0.86],
    material: "quill",
  }));
  part("left_lower_vane", box({
    at: [-0.085, 0.022, 0.17],
    rot: [0, -8, 0],
    size: [0.17, 0.03, 0.31],
    material: "vane_dark",
  }));
  part("right_lower_vane", box({
    at: [0.075, 0.024, 0.16],
    rot: [0, 7, 0],
    size: [0.15, 0.034, 0.34],
    material: "vane",
  }));
  part("left_speculum", box({
    at: [-0.095, 0.031, -0.09],
    rot: [0, 5, 0],
    size: [0.19, 0.038, 0.24],
    material: "speculum",
  }));
  part("right_speculum", box({
    at: [0.085, 0.033, -0.1],
    rot: [0, -5, 0],
    size: [0.17, 0.042, 0.25],
    material: "speculum",
  }));
  part("left_tip", box({
    at: [-0.065, 0.041, -0.31],
    rot: [0, -12, 0],
    size: [0.13, 0.032, 0.2],
    material: "edge",
  }));
  part("right_tip", box({
    at: [0.055, 0.043, -0.32],
    rot: [0, 10, 0],
    size: [0.11, 0.036, 0.2],
    material: "vane",
  }));
});
