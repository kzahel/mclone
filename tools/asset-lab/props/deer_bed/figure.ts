import { figure } from "../../src/dsl";

// A low, irregular resting impression: pressed grass around a dark oval bed.
// It is sign left by ordinary bedding behavior, not a terrain track decal.
export default figure("deer_bed", ({ box, mat, part }) => {
  mat("pressed_dark", "#4c472c");
  mat("pressed", "#6f6a39");
  mat("dry_grass", "#9a8950");

  part("bed_center", box({
    at: [0, 0.018, 0],
    size: [0.72, 0.036, 1.05],
    material: "pressed_dark",
  }));
  for (const [name, at, rot, size] of [
    ["grass_north", [-0.08, 0.045, -0.52], [0, -7, 0], [0.62, 0.05, 0.12]],
    ["grass_south", [0.12, 0.04, 0.52], [0, 9, 0], [0.58, 0.045, 0.12]],
    ["grass_west", [-0.38, 0.038, 0.03], [0, -3, 0], [0.12, 0.042, 0.82]],
    ["grass_east", [0.39, 0.042, -0.06], [0, 6, 0], [0.11, 0.048, 0.76]],
  ] as const) {
    part(name, box({ at, rot, size, material: "pressed" }));
  }
  part("loose_stem", box({
    at: [0.18, 0.07, -0.08],
    rot: [0, -32, 3],
    size: [0.045, 0.04, 0.7],
    material: "dry_grass",
  }));
});
