import { figure } from "../../src/dsl";

export default figure("carrot", ({ box, mat, part }) => {
  mat("root", "#e97817");
  mat("root_light", "#f39a2d");
  mat("leaf", "#3d9635");
  mat("leaf_light", "#67b84e");

  part("root_upper", box({
    at: [0, 0, 0],
    size: [0.19, 0.28, 0.19],
    material: "root_light",
  }));
  part("root_tip", box({
    at: [0, -0.18, 0],
    size: [0.1, 0.12, 0.1],
    material: "root",
  }));
  part("leaf_center", box({
    at: [0, 0.2, 0],
    rot: [0, 0, -5],
    size: [0.055, 0.22, 0.055],
    material: "leaf_light",
  }));
  part("leaf_left", box({
    at: [-0.07, 0.18, 0],
    rot: [0, 0, 24],
    size: [0.055, 0.2, 0.055],
    material: "leaf",
  }));
  part("leaf_right", box({
    at: [0.07, 0.18, 0],
    rot: [0, 0, -24],
    size: [0.055, 0.2, 0.055],
    material: "leaf",
  }));
});
