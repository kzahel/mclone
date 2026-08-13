import { figure } from "../../src/dsl";

// A small tied sheaf, centered for the ordinary dropped-item presentation.
export default figure("wheat_bundle", ({ box, mat, part }) => {
  mat("stalk", "#b98a2f");
  mat("grain", "#e2bd58");
  mat("grain_light", "#f0d477");
  mat("tie", "#76502a");

  const stalks = [
    [-0.13, -0.018, -7],
    [-0.065, 0.012, 4],
    [0, -0.01, -2],
    [0.065, 0.018, 7],
    [0.13, -0.015, -5],
  ] as const;
  for (const [index, [x, z, rotation]] of stalks.entries()) {
    part(`stalk_${index}`, box({
      at: [x, 0, z],
      rot: [0, 0, rotation],
      size: [0.025, 0.44, 0.025],
      material: "stalk",
    }));
    part(`grain_${index}`, box({
      at: [x - rotation * 0.0015, 0.235, z],
      rot: [0, 0, rotation],
      size: [0.052, 0.13, 0.055],
      material: index % 2 === 0 ? "grain_light" : "grain",
    }));
  }
  part("tie", box({
    at: [0, -0.015, 0],
    size: [0.32, 0.075, 0.08],
    material: "tie",
  }));
});
