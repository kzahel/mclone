import { figure } from "../../src/dsl";

export default figure("oak_fence_item", ({ box, mat, part }) => {
  mat("oak", "#9b6b35");
  mat("oak_light", "#bd884b");

  part("post", box({
    at: [0, 0, 0],
    size: [0.14, 0.48, 0.14],
    material: "oak_light",
  }));
  part("rail_upper", box({
    at: [0, 0.1, 0],
    size: [0.48, 0.09, 0.09],
    material: "oak",
  }));
  part("rail_lower", box({
    at: [0, -0.1, 0],
    size: [0.48, 0.09, 0.09],
    material: "oak",
  }));
});
