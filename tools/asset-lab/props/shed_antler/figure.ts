import { figure } from "../../src/dsl";

// One shed antler with a burr, curved beam, and three readable tines.
export default figure("shed_antler", ({ box, mat, part }) => {
  mat("antler_dark", "#6c543d");
  mat("antler", "#a58a65");
  mat("tip", "#d1bea0");

  part("burr", box({
    at: [0.27, 0.07, 0.25],
    rot: [0, -28, 0],
    size: [0.16, 0.12, 0.14],
    material: "antler_dark",
  }));
  part("beam_base", box({
    at: [0.15, 0.1, 0.12],
    rot: [0, 38, -8],
    size: [0.09, 0.1, 0.35],
    material: "antler",
  }));
  part("beam_tip", box({
    at: [-0.07, 0.12, -0.12],
    rot: [0, 48, -5],
    size: [0.075, 0.085, 0.32],
    material: "antler",
  }));
  part("tine_inner", box({
    at: [0.11, 0.17, -0.02],
    rot: [15, 5, -38],
    size: [0.065, 0.065, 0.24],
    material: "antler",
  }));
  part("tine_middle", box({
    at: [-0.04, 0.18, -0.16],
    rot: [12, -6, -42],
    size: [0.06, 0.06, 0.22],
    material: "antler",
  }));
  part("tine_tip", box({
    at: [-0.19, 0.16, -0.3],
    rot: [8, -10, -48],
    size: [0.055, 0.055, 0.18],
    material: "tip",
  }));
});
