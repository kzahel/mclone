import { figure } from "../../src/dsl";

// The ordinary player-placeable sleeping mat. Its checked semantic source
// remains presentation-only; placement, sleep state, quorum, and persistence
// are authoritative gameplay facts.
export default figure("sleeping_mat", ({ box, mat, part }) => {
  mat("mat_dark", "#405448");
  mat("mat", "#637761");
  mat("stripe", "#b97945");
  mat("pillow", "#d7c99f");
  mat("binding", "#493b31");

  part("mat_base", box({
    at: [0, 0.045, 0],
    size: [1.08, 0.09, 1.82],
    material: "mat_dark",
  }));
  part("mat_top", box({
    at: [0, 0.098, 0.05],
    size: [0.98, 0.035, 1.66],
    material: "mat",
  }));
  part("stripe_north", box({
    at: [0, 0.12, -0.33],
    size: [0.99, 0.02, 0.14],
    material: "stripe",
  }));
  part("stripe_south", box({
    at: [0, 0.12, 0.4],
    size: [0.99, 0.02, 0.14],
    material: "stripe",
  }));
  part("pillow", box({
    at: [0, 0.16, -0.7],
    size: [0.7, 0.12, 0.32],
    material: "pillow",
  }));
  part("binding_west", box({
    at: [-0.535, 0.09, 0],
    size: [0.035, 0.11, 1.84],
    material: "binding",
  }));
  part("binding_east", box({
    at: [0.535, 0.09, 0],
    size: [0.035, 0.11, 1.84],
    material: "binding",
  }));
  part("foot_fold", box({
    at: [0, 0.145, 0.82],
    rot: [-3, 0, 0],
    size: [1.02, 0.1, 0.18],
    material: "mat_dark",
  }));
});
