import { figure } from "../../src/dsl";

// A compact raw venison cut with a pale fat edge, centered for pickup display.
export default figure("venison", ({ box, mat, part }) => {
  mat("meat_dark", "#71352f");
  mat("meat", "#a54d45");
  mat("fat", "#d7b99a");

  part("cut", box({
    at: [0, 0.08, 0],
    rot: [0, -12, 0],
    size: [0.46, 0.15, 0.3],
    material: "meat_dark",
  }));
  part("center", box({
    at: [-0.03, 0.155, -0.015],
    rot: [0, -12, 0],
    size: [0.34, 0.08, 0.22],
    material: "meat",
  }));
  part("fat_edge", box({
    at: [0.2, 0.13, 0.01],
    rot: [0, -12, 4],
    size: [0.055, 0.09, 0.25],
    material: "fat",
  }));
});
