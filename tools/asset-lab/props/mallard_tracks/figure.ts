import { figure } from "../../src/dsl";

// Two staggered webbed prints authored as a surface trace. Each toe meets its
// rear pad; the two footprints are intentionally separate physical evidence.
export default figure("mallard_tracks", ({ box, geometryException, mat, part, surfaceException }) => {
  mat("track_deep", "#594126");
  mat("track", "#765937");
  mat("track_soft", "#92734b");

  addFootprint("left", -0.17, -0.12, -7);
  addFootprint("right", 0.2, 0.2, 8);

  geometryException({
    rule: "disconnected-component",
    parts: ["right_pad", "right_toe_center", "right_toe_inner", "right_toe_outer"],
    reason: "A track pair records two intentionally separate webbed footprints.",
  });

  for (const [first, second] of [
    ["left_pad", "left_toe_center"],
    ["left_toe_center", "left_toe_inner"],
    ["left_toe_inner", "left_toe_outer"],
    ["right_pad", "right_toe_center"],
    ["right_toe_center", "right_toe_inner"],
    ["right_toe_inner", "right_toe_outer"],
  ] as const) {
    surfaceException({
      rule: "coplanar-overlap",
      faces: [
        { part: first, face: "down" },
        { part: second, face: "down" },
      ],
      reason: "Joined pieces of one webbed imprint intentionally share its flat surface-contact plane.",
    });
  }

  function addFootprint(prefix: string, x: number, z: number, yaw: number): void {
    part(`${prefix}_pad`, box({
      at: [x, 0.007, z + 0.1],
      rot: [0, yaw, 0],
      size: [0.13, 0.014, 0.1],
      material: "track_deep",
    }));
    part(`${prefix}_toe_center`, box({
      at: [x, 0.009, z - 0.055],
      rot: [0, yaw, 0],
      size: [0.058, 0.018, 0.25],
      material: "track",
    }));
    part(`${prefix}_toe_inner`, box({
      at: [x - 0.065, 0.008, z - 0.04],
      rot: [0, yaw - 27, 0],
      size: [0.054, 0.016, 0.23],
      material: "track_soft",
    }));
    part(`${prefix}_toe_outer`, box({
      at: [x + 0.065, 0.01, z - 0.04],
      rot: [0, yaw + 27, 0],
      size: [0.054, 0.02, 0.23],
      material: "track",
    }));
  }
});
