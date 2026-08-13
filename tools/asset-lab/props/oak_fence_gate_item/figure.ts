import { figure } from "../../src/dsl";

export default figure("oak_fence_gate_item", ({ box, mat, part }) => {
  mat("oak", "#9b6b35");
  mat("oak_light", "#bd884b");

  part("post_left", box({
    at: [-0.2, 0, 0],
    size: [0.11, 0.46, 0.11],
    material: "oak_light",
  }));
  part("post_right", box({
    at: [0.2, 0, 0],
    size: [0.11, 0.46, 0.11],
    material: "oak_light",
  }));
  part("rail_upper", box({
    at: [0, 0.11, 0],
    size: [0.37, 0.075, 0.075],
    material: "oak",
  }));
  part("rail_lower", box({
    at: [0, -0.11, 0],
    size: [0.37, 0.075, 0.075],
    material: "oak",
  }));
  part("brace", box({
    at: [0, 0, 0.005],
    rot: [0, 0, -24],
    size: [0.38, 0.055, 0.055],
    material: "oak_light",
  }));
});
