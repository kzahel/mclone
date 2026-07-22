import { figure, type ClipKey } from "../../src/dsl";

// A low land tortoise whose old shell has become naturally mottled with
// lichen. The biological silhouette stays primary: domed shell, thick pads,
// blunt head, short tail, and a withdrawal that lowers its plastron to ground.
export default figure("mossback_tortoise", ({
  asciiTexture,
  box,
  clip,
  defaultClip,
  followThrough,
  mat,
  metadata,
  part,
  quadrupedWalk,
}) => {
  metadata({
    bodyPlans: ["quadruped"],
    disposition: "passive",
    groups: ["animal"],
    habitats: ["land"],
    scale: "medium",
    themes: ["lichen", "mossback", "symbiotic", "tortoise", "woodland"],
  });

  mat("skin", "#6f7750");
  mat("skin_light", "#92986b");
  mat("skin_dark", "#4c543b");
  mat("shell", "#6e5734");
  mat("shell_light", "#977545");
  mat("shell_dark", "#453a29");
  mat("lichen", "#708452");
  mat("eye", "#1c2118");

  asciiTexture("tortoise_face", {
    palette: { ".": "#6f7750", "l": "#92986b", "d": "#4c543b", "e": "#1c2118" },
    pixels: [
      "dd......dd",
      "d..ee.ee.d",
      "...ee.ee..",
      "....ll....",
      "..d....d..",
      ".d......d.",
      "....dd....",
      "dd......dd",
    ],
  });
  asciiTexture("lichen_scutes", {
    palette: {
      ".": "#6e5734",
      "l": "#977545",
      "d": "#453a29",
      "m": "#708452",
      "g": "#94a568",
    },
    pixels: [
      "dddllllllddd",
      "dlmmmllgglld",
      "dmmldddlggld",
      "lmldllllldgl",
      "lmmldgglddll",
      "lddlgggmldll",
      "lglddddmlggl",
      "dllggmmmmlld",
      "dddllllllddd",
    ],
  });
  asciiTexture("shell_band", {
    palette: { ".": "#6e5734", "l": "#977545", "d": "#453a29", "m": "#708452" },
    pixels: [
      "dddddddddddd",
      "dll..mm..lld",
      "d..llllmm..d",
      "dmm..ll....d",
      "dddddddddddd",
    ],
  });
  asciiTexture("pad_scales", {
    palette: { ".": "#6f7750", "l": "#92986b", "d": "#4c543b" },
    pixels: ["dldldldl", "ldldldld", "dldldldl", "llllllll"],
  });

  part("body", box({
    at: [0, 0.32, 0.06],
    size: [0.7, 0.32, 1.0],
    material: "skin_dark",
  }));
  part("shell_rim", box({
    parent: "body",
    at: [0, 0.12, 0.04],
    size: [1.12, 0.18, 1.2],
    material: "shell_dark",
    faces: { east: { texture: "shell_band" }, west: { texture: "shell_band" } },
  }));
  part("shell", box({
    parent: "body",
    at: [0, 0.25, 0.04],
    size: [1.04, 0.42, 1.14],
    material: "shell",
    faces: {
      north: { texture: "shell_band" },
      south: { texture: "shell_band" },
      east: { texture: "lichen_scutes" },
      west: { texture: "lichen_scutes" },
      up: { texture: "lichen_scutes" },
    },
  }));
  part("shell_crown", box({
    parent: "shell",
    at: [0, 0.28, 0],
    size: [0.84, 0.24, 0.9],
    material: "shell_light",
    faces: { up: { texture: "lichen_scutes" }, east: { texture: "lichen_scutes" }, west: { texture: "lichen_scutes" } },
  }));

  part("neck", box({
    parent: "body",
    at: [0, 0, -0.53],
    size: [0.42, 0.24, 0.3],
    material: "skin_light",
  }));
  part("head", box({
    parent: "neck",
    at: [0, 0.02, -0.28],
    size: [0.48, 0.34, 0.38],
    material: "skin",
    faces: { north: { texture: "tortoise_face" } },
    joint: { pivot: [0, 0, 0.14], axis: [1, 0, 0] },
  }));
  part("beak", box({
    parent: "head",
    at: [0, -0.07, -0.23],
    size: [0.34, 0.16, 0.12],
    material: "skin_light",
  }));

  const limbs = [
    { suffix: "fl", x: -0.42, z: -0.29 },
    { suffix: "fr", x: 0.42, z: -0.29 },
    { suffix: "bl", x: -0.42, z: 0.29 },
    { suffix: "br", x: 0.42, z: 0.29 },
  ] as const;
  for (const limb of limbs) {
    part(`leg_${limb.suffix}`, box({
      parent: "body",
      at: [limb.x, -0.18, limb.z],
      rot: [0, 0, limb.x < 0 ? -4 : 4],
      size: [0.3, 0.2, 0.38],
      material: limb.z < 0 ? "skin_light" : "skin",
      faces: { north: { texture: "pad_scales" }, south: { texture: "pad_scales" } },
      joint: { pivot: [limb.x < 0 ? 0.12 : -0.12, 0.08, 0], axis: [1, 0, 0] },
    }));
    part(`pad_${limb.suffix}`, box({
      parent: `leg_${limb.suffix}`,
      at: [limb.x < 0 ? -0.02 : 0.02, -0.1, -0.025],
      size: [0.32, 0.08, 0.42],
      material: "skin_dark",
      faces: { up: { texture: "pad_scales" } },
    }));
  }

  part("tail", box({
    parent: "body",
    at: [0, -0.02, 0.56],
    rot: [17, 0, 0],
    size: [0.2, 0.18, 0.36],
    material: "skin_dark",
    joint: { pivot: [0, 0, -0.15], axis: [1, 0, 0] },
  }));

  quadrupedWalk("lichen_crawl", {
    label: "Lichen crawl",
    fps: 18,
    duration: 1.44,
    cycleDistance: 0.34,
    gait: "walk",
    loop: true,
    samples: 27,
    contactParts: {
      frontLeft: "pad_fl",
      frontRight: "pad_fr",
      backLeft: "pad_bl",
      backRight: "pad_br",
    },
    body: "body",
    bodyBob: 0.006,
    bodyBobCenter: 0.007,
    head: "head",
    headSwingDegrees: 2,
    legs: {
      frontLeft: "leg_fl",
      frontRight: "leg_fr",
      backLeft: "leg_bl",
      backRight: "leg_br",
    },
    stanceRatio: 0.78,
    swingDegrees: 11,
    tail: "tail",
    tailSwingDegrees: 3,
    tracks: [
      followThrough("neck", { source: "body", sourceChannel: "pos", sourceAxis: "y", axis: "x", degrees: 2.5, overshoot: 0.35, lag: 0.11 }),
    ],
  });

  const withdrawKeys: ClipKey[] = [
    ["body", 0, { at: [0, 0, 0] }],
    ["body", 0.34, { at: [0, -0.14, 0] }],
    ["body", 0.72, { at: [0, -0.14, 0] }],
    ["body", 1.22, { at: [0, 0, 0] }],
    ["neck", 0, { at: [0, 0, 0], scale: [1, 1, 1] }],
    ["neck", 0.34, { at: [0, 0.06, 0.28], scale: [0.7, 0.6, 0.22] }],
    ["neck", 0.72, { at: [0, 0.06, 0.28], scale: [0.7, 0.6, 0.22] }],
    ["neck", 1.22, { at: [0, 0, 0], scale: [1, 1, 1] }],
    ["head", 0, { at: [0, 0, 0], scale: [1, 1, 1] }],
    ["head", 0.34, { at: [0, 0.04, 0.23], scale: [1, 0.45, 0.45] }],
    ["head", 0.72, { at: [0, 0.04, 0.23], scale: [1, 0.45, 0.45] }],
    ["head", 1.22, { at: [0, 0, 0], scale: [1, 1, 1] }],
    ["tail", 0, { at: [0, 0, 0], scale: [1, 1, 1] }],
    ["tail", 0.34, { at: [0, 0.05, -0.2], scale: [0.3, 0.3, 0.3] }],
    ["tail", 0.72, { at: [0, 0.05, -0.2], scale: [0.3, 0.3, 0.3] }],
    ["tail", 1.22, { at: [0, 0, 0], scale: [1, 1, 1] }],
  ];
  for (const limb of limbs) {
    const inward = limb.x < 0 ? 0.1 : -0.1;
    const towardBody = limb.z < 0 ? 0.12 : -0.12;
    withdrawKeys.push(
      [`leg_${limb.suffix}`, 0, { at: [0, 0, 0], scale: [1, 1, 1] }],
      [`leg_${limb.suffix}`, 0.34, { at: [inward, 0.18, towardBody], scale: [0.25, 0.25, 0.25] }],
      [`leg_${limb.suffix}`, 0.72, { at: [inward, 0.18, towardBody], scale: [0.25, 0.25, 0.25] }],
      [`leg_${limb.suffix}`, 1.22, { at: [0, 0, 0], scale: [1, 1, 1] }],
    );
  }
  clip("shell_withdraw", {
    label: "Shell withdraw",
    role: "action",
    nextClip: "lichen_crawl",
    fps: 30,
    loop: false,
    keys: withdrawKeys,
  });
  defaultClip("lichen_crawl");
});
