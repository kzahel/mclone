import {
  figure,
  type ClipKey,
  type CycleTrack,
  type LocomotionContactSpec,
} from "../../src/dsl";

// A giant neutral banyan carried by six stilt roots. Its asymmetric canopy
// follows the root stride, then compresses as the tree replants itself.
export default figure("walking_banyan", ({
  asciiTexture,
  bob,
  box,
  clip,
  contactSwing,
  defaultClip,
  mat,
  metadata,
  part,
  swing,
  walkCycle,
}) => {
  metadata({
    bodyPlans: ["crawler", "rooted"],
    disposition: "neutral",
    groups: ["plant"],
    habitats: ["land"],
    scale: "giant",
    themes: ["ancient", "living-growth", "mobile", "tree", "woodland"],
  });

  mat("bark", "#66513a");
  mat("bark_light", "#8a704b");
  mat("bark_dark", "#3d3428");
  mat("root", "#51432f");
  mat("leaf", { color: "#3e713d", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("leaf_light", { color: "#62934e", alphaMode: "mask", alphaCutoff: 0.1 });
  mat("moss", "#71834a");
  mat("eye", "#c5d879");

  asciiTexture("banyan_face", {
    palette: {
      ".": "#66513a",
      "l": "#8a704b",
      "d": "#3d3428",
      "e": "#c5d879",
      "m": "#71834a",
    },
    pixels: [
      "ddddlllllllldddd",
      "dd............dd",
      "d...ee....ee...d",
      "....ee....ee....",
      "......dddd......",
      "....dd....dd....",
      "..mm........mm..",
      "dddd........dddd",
    ],
  });
  asciiTexture("leaf_holes", {
    palette: {
      ".": "transparent",
      "l": "#62934e",
      "g": "#3e713d",
      "d": "#294f30",
      "m": "#71834a",
    },
    pixels: [
      "..ggggllllgg..",
      ".ggllllgggggg.",
      "ggglggggllgggg",
      "gllllggggggllg",
      "gggggmmggggggg",
      "lgggggggglllgg",
      ".ggllgggggggg.",
      "..ggg..gggg...",
    ],
  });
  asciiTexture("root_rings", {
    palette: { ".": "#51432f", "l": "#66513a", "d": "#3d3428", "m": "#71834a" },
    pixels: [
      "dddddddd",
      "d.ll.l.d",
      ".l....l.",
      "..m..m..",
      "dddddddd",
    ],
  });

  part("root_crown", box({
    at: [0, 0.88, 0],
    size: [1.08, 0.5, 0.92],
    material: "root",
    faces: { east: { texture: "root_rings" }, west: { texture: "root_rings" } },
  }));
  part("trunk", box({
    parent: "root_crown",
    at: [0, 0.78, 0],
    rot: [0, 0, -2],
    size: [0.84, 1.22, 0.74],
    material: "bark",
    faces: { north: { texture: "banyan_face" } },
    joint: { pivot: [0, -0.55, 0], axis: [0, 0, 1] },
  }));
  part("trunk_moss", box({
    parent: "trunk",
    at: [-0.32, -0.08, -0.39],
    rot: [0, 0, -5],
    size: [0.18, 0.54, 0.1],
    material: "moss",
  }));
  part("branch_bar", box({
    parent: "trunk",
    at: [0, 0.56, 0.02],
    rot: [0, 0, 3],
    size: [1.64, 0.24, 0.38],
    material: "bark_dark",
  }));
  part("branch_front", box({
    parent: "trunk",
    at: [-0.12, 0.48, -0.46],
    rot: [8, 0, -5],
    size: [0.36, 0.22, 0.9],
    material: "bark_light",
  }));
  part("branch_back", box({
    parent: "trunk",
    at: [0.15, 0.44, 0.43],
    rot: [-7, 0, 6],
    size: [0.34, 0.2, 0.82],
    material: "bark_light",
  }));

  const leafFaces = {
    north: { texture: "leaf_holes" },
    south: { texture: "leaf_holes" },
    east: { texture: "leaf_holes" },
    west: { texture: "leaf_holes" },
    up: { texture: "leaf_holes" },
    down: { texture: "leaf_holes" },
  } as const;
  part("canopy_center", box({
    parent: "branch_bar",
    at: [0, 0.42, 0],
    size: [1.34, 0.68, 1.08],
    material: "leaf",
    faces: leafFaces,
  }));
  part("canopy_l", box({
    parent: "branch_bar",
    at: [-0.78, 0.31, -0.04],
    rot: [0, -4, -3],
    size: [1.02, 0.58, 0.9],
    material: "leaf_light",
    faces: leafFaces,
  }));
  part("canopy_r", box({
    parent: "branch_bar",
    at: [0.82, 0.38, 0.05],
    rot: [0, 5, 4],
    size: [1.12, 0.64, 0.94],
    material: "leaf",
    faces: leafFaces,
  }));
  part("canopy_front", box({
    parent: "branch_front",
    at: [-0.1, 0.28, -0.5],
    rot: [0, -5, -2],
    size: [0.94, 0.56, 0.92],
    material: "leaf_light",
    faces: leafFaces,
  }));
  part("canopy_back", box({
    parent: "branch_back",
    at: [0.08, 0.3, 0.46],
    rot: [0, 4, 3],
    size: [0.88, 0.52, 0.86],
    material: "leaf",
    faces: leafFaces,
  }));

  const roots = [
    { name: "fl", x: -0.4, z: -0.3, phase: 0 },
    { name: "fr", x: 0.4, z: -0.3, phase: 0.5 },
    { name: "ml", x: -0.46, z: 0, phase: 0.5 },
    { name: "mr", x: 0.46, z: 0, phase: 0 },
    { name: "bl", x: -0.38, z: 0.31, phase: 0 },
    { name: "br", x: 0.38, z: 0.31, phase: 0.5 },
  ] as const;
  for (const root of roots) {
    const sign = root.x < 0 ? -1 : 1;
    part(`root_leg_${root.name}`, box({
      parent: "root_crown",
      at: [root.x, -0.46, root.z],
      rot: [root.z * -8, 0, sign * -5],
      size: [0.24, 0.58, 0.26],
      material: "root",
      joint: { pivot: [0, 0.27, 0], axis: [1, 0, 0] },
    }));
    part(`root_pad_${root.name}`, box({
      parent: `root_leg_${root.name}`,
      at: [sign * 0.08, -0.34, root.z < 0 ? -0.06 : 0.04],
      rot: [0, sign * -4, 0],
      size: [0.38, 0.12, 0.42],
      material: "bark_dark",
      faces: { up: { texture: "root_rings" } },
    }));
  }

  const stanceRatio = 0.72;
  const contacts: LocomotionContactSpec[] = [];
  const tracks: CycleTrack[] = [
    bob("root_crown", { axis: "y", amount: 0.014, center: 0.016, phase: 0.5 }),
    swing("trunk", { axis: "z", degrees: 1.8, phase: 0.25 }),
    swing("canopy_l", { axis: "z", degrees: 2.2, phase: 0.16 }),
    swing("canopy_r", { axis: "z", degrees: -2.4, phase: 0.64 }),
    swing("canopy_front", { axis: "x", degrees: 2, phase: 0.38 }),
  ];
  for (const root of roots) {
    contacts.push({
      part: `root_pad_${root.name}`,
      phaseStart: root.phase,
      phaseEnd: (root.phase + stanceRatio) % 1,
      role: root.name,
      stanceRatio,
    });
    tracks.push(contactSwing(`root_leg_${root.name}`, {
      axis: "x",
      degrees: 13,
      phase: root.phase,
      stanceRatio,
    }));
  }

  walkCycle("root_stride", {
    label: "Root stride",
    role: "locomotion",
    fps: 20,
    duration: 1.46,
    loop: true,
    samples: 31,
    locomotion: {
      kind: "quadruped-walk",
      cycleDistance: 0.62,
      direction: [0, 0, -1],
      units: "figure",
      contacts,
    },
    tracks,
  });

  const replantKeys: ClipKey[] = [
    ["root_crown", 0, { at: [0, 0, 0] }],
    ["root_crown", 0.32, { at: [0, 0.1, 0] }],
    ["root_crown", 0.58, { at: [0, -0.018, 0] }],
    ["root_crown", 0.82, { at: [0, 0.025, 0] }],
    ["root_crown", 1.24, { at: [0, 0, 0] }],
    ["trunk", 0, { rot: [0, 0, 0] }],
    ["trunk", 0.32, { rot: [-3, 0, 4] }],
    ["trunk", 0.58, { rot: [4, 0, -3] }],
    ["trunk", 1.24, { rot: [0, 0, 0] }],
    ["canopy_center", 0, { scale: [1, 1, 1] }],
    ["canopy_center", 0.58, { scale: [1.05, 0.88, 1.05] }],
    ["canopy_center", 0.82, { scale: [0.98, 1.05, 0.98] }],
    ["canopy_center", 1.24, { scale: [1, 1, 1] }],
  ];
  for (const root of roots) {
    const sign = root.x < 0 ? -1 : 1;
    replantKeys.push(
      [`root_leg_${root.name}`, 0, { rot: [0, 0, 0] }],
      [`root_leg_${root.name}`, 0.32, { rot: [-7, 0, sign * -5] }],
      [`root_leg_${root.name}`, 0.58, { rot: [5, 0, sign * 12] }],
      [`root_leg_${root.name}`, 1.24, { rot: [0, 0, 0] }],
    );
  }
  clip("replant", {
    label: "Replant",
    role: "action",
    nextClip: "root_stride",
    fps: 30,
    loop: false,
    keys: replantKeys,
  });
  defaultClip("root_stride");
});
