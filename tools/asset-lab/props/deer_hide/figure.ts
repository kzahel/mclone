import { figure } from "../../src/dsl";

// A folded hide with a warm outer coat and readable pale underside.
export default figure("deer_hide", ({ box, mat, part }) => {
  mat("coat", "#956744");
  mat("coat_dark", "#6f472f");
  mat("underside", "#d0b18c");

  part("folded_hide", box({
    at: [0, 0.065, 0],
    size: [0.5, 0.1, 0.36],
    material: "coat_dark",
  }));
  part("upper_fold", box({
    at: [-0.06, 0.135, -0.02],
    rot: [0, 8, 0],
    size: [0.38, 0.08, 0.28],
    material: "coat",
  }));
  part("turned_corner", box({
    at: [0.16, 0.18, 0.1],
    rot: [0, -16, 7],
    size: [0.17, 0.045, 0.13],
    material: "underside",
  }));
});
