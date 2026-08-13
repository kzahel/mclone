import { figure } from "../../src/dsl";

// The surrounding bank is real world terrain with one excavated entrance
// cell. This prop supplies only the recessed darkness and irregular threshold;
// it deliberately does not pretend to be a freestanding mound or tunnel.
export default figure("rabbit_burrow", ({ asciiTexture, box, mat, part }) => {
  mat("cavity", "#17130f");
  mat("deep_cavity", "#090807");
  mat("earth", "#745134");
  mat("earth_dark", "#543823");
  mat("dry_soil", "#98704a");
  mat("root", "#aa8861");
  mat("moss", "#65723d");

  asciiTexture("soil_break", {
    palette: { ".": "#745134", "d": "#543823", "l": "#98704a", "m": "#65723d" },
    pixels: [
      "dd..l...dd",
      ".ll...dd..",
      "d...m...ld",
      "..dd...l..",
      "l...dd...m",
      "..m...ll..",
    ],
  });

  // Two nested dark planes make the recess survive oblique viewing while the
  // real rear block remains the authoritative end of the shallow threshold.
  part("deep_back", box({
    at: [0, 0.34, 0.37],
    size: [0.56, 0.5, 0.035],
    material: "deep_cavity",
  }));
  part("cavity_floor", box({
    at: [0, 0.035, 0.08],
    rot: [2, 0, 0],
    size: [0.6, 0.07, 0.66],
    material: "cavity",
  }));

  // An intentionally uneven horseshoe around the entrance. The top/side
  // pieces remain below one block so the world roof and side walls dominate.
  part("rim_left", box({
    at: [-0.36, 0.28, -0.31],
    rot: [0, 7, -8],
    size: [0.2, 0.58, 0.16],
    material: "earth",
    faces: { north: { texture: "soil_break" } },
  }));
  part("rim_right", box({
    at: [0.35, 0.26, -0.3],
    rot: [0, -6, 7],
    size: [0.19, 0.53, 0.17],
    material: "earth_dark",
    faces: { north: { texture: "soil_break" } },
  }));
  part("rim_crown_left", box({
    at: [-0.2, 0.6, -0.29],
    rot: [0, 3, 13],
    size: [0.42, 0.18, 0.17],
    material: "earth",
    faces: { north: { texture: "soil_break" } },
  }));
  part("rim_crown_right", box({
    at: [0.2, 0.61, -0.29],
    rot: [0, -4, -11],
    size: [0.4, 0.17, 0.16],
    material: "earth_dark",
    faces: { north: { texture: "soil_break" } },
  }));
  part("threshold_soil", box({
    at: [0.02, 0.055, -0.35],
    rot: [0, -3, 0],
    size: [0.78, 0.1, 0.25],
    material: "dry_soil",
    faces: { up: { texture: "soil_break" } },
  }));
  part("loose_clod_left", box({
    at: [-0.4, 0.11, -0.46],
    rot: [0, 18, 7],
    size: [0.19, 0.12, 0.24],
    material: "earth_dark",
  }));
  part("loose_clod_right", box({
    at: [0.38, 0.09, -0.49],
    rot: [0, -23, -4],
    size: [0.24, 0.1, 0.18],
    material: "dry_soil",
  }));
  part("root_left", box({
    at: [-0.25, 0.59, -0.405],
    rot: [17, 4, 8],
    size: [0.035, 0.34, 0.035],
    material: "root",
  }));
  part("root_right", box({
    at: [0.24, 0.57, -0.4],
    rot: [-10, -5, -9],
    size: [0.035, 0.25, 0.035],
    material: "root",
  }));
  part("moss_tuft", box({
    at: [-0.31, 0.69, -0.32],
    rot: [0, 9, 4],
    size: [0.27, 0.045, 0.19],
    material: "moss",
  }));
});
