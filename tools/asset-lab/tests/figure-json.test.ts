import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  assertBoxOnlyFigure,
  figure,
  legacyFigure,
  type FigureAsset,
} from "../src/dsl";
import {
  parseFigureAssetJson,
  roundTripFigureAsset,
  serializeFigureAsset,
} from "../src/figure-json";
import { loadFigureJsonDocument } from "../src/load";
import { assetLabRoot } from "../src/vite-figure-path";

test("serializes and reparses the complete semantic figure", () => {
  const source = tinyFigure();
  const document = roundTripFigureAsset(source, "tiny source");

  assert.notEqual(document.asset, source);
  assert.deepEqual(document.asset, source);
  assert.equal(document.json, serializeFigureAsset(source));
  assert.match(document.json, /\n$/);
});

test("round-trips typed creature metadata and rejects invalid tags", () => {
  const classified = figure("classified", ({ box, mat, metadata, part }) => {
    metadata({
      bodyPlans: ["quadruped", "swimmer"],
      disposition: "neutral",
      groups: ["animal"],
      habitats: ["land", "water"],
      scale: "medium",
      themes: ["amphibious"],
    });
    mat("skin", "#667766");
    part("body", box({ at: [0, 0.5, 0], size: [1, 1, 1], material: "skin" }));
  });
  assert.deepEqual(roundTripFigureAsset(classified, "classified source").asset.metadata, {
    bodyPlans: ["quadruped", "swimmer"],
    disposition: "neutral",
    groups: ["animal"],
    habitats: ["land", "water"],
    scale: "medium",
    themes: ["amphibious"],
  });

  const invalid = tinyFigure();
  invalid.metadata = {
    bodyPlans: ["quadruped"],
    disposition: "neutral",
    groups: ["animal"],
    habitats: ["land"],
    scale: "medium",
    themes: ["Not a tag"],
  };
  assert.throws(
    () => serializeFigureAsset(invalid),
    /figure metadata theme 0 must be a lowercase tag/,
  );
});

test("rejects values that cannot cross the JSON contract", () => {
  const source = tinyFigure();
  source.parts[0]!.at = [Number.NaN, 0, 0];

  assert.throws(
    () => roundTripFigureAsset(source, "non-finite source"),
    /part 'body' at\[0\] must be finite/,
  );
});

test("loads a JSON file as a canonical figure document", async () => {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "mclone-asset-lab-json-"));
  try {
    const jsonPath = path.join(tempDir, "tiny.figure.json");
    const expected = serializeFigureAsset(tinyFigure());
    await fs.writeFile(jsonPath, expected, "utf8");

    const document = await loadFigureJsonDocument(jsonPath);

    assert.equal(document.asset.name, "tiny");
    assert.equal(document.json, expected);
  } finally {
    await fs.rm(tempDir, { force: true, recursive: true });
  }
});

test("TypeScript source and checked player JSON resolve identically", async () => {
  const source = await loadFigureJsonDocument(path.join(assetLabRoot, "examples/player/figure.ts"));
  const checked = await loadFigureJsonDocument(
    path.resolve(assetLabRoot, "../../assets/mclone/figures/player.figure.json"),
  );

  assert.deepEqual(source.asset, checked.asset);
  assert.equal(source.json, checked.json);
});

test("canonical figure rejects deprecated curved primitives", () => {
  assert.throws(
    () =>
      figure("curved", ({ part }) => {
        part("orb", { primitive: { kind: "sphere", radius: 1 } });
      }),
    /canonical figures may use only box primitives/,
  );

  const legacy = legacyFigure("curved_legacy", ({ part, sphere }) => {
    part("orb", sphere({ radius: 1 }));
  });
  assert.equal(legacy.parts[0]?.primitive.kind, "sphere");
});

test("canonical geometry requires reasoned disconnected-component exceptions", () => {
  assert.throws(
    () => figure("detached", ({ box, mat, part }) => {
      mat("skin", "#667766");
      part("body", box({ size: [1, 1, 1], material: "skin" }));
      part("halo", box({ at: [0, 1.5, 0], size: [0.2, 0.2, 0.2], material: "skin" }));
    }),
    /disconnected-component.*geometryException/s,
  );

  const acknowledged = figure("acknowledged_detached", ({
    box,
    geometryException,
    mat,
    part,
  }) => {
    mat("skin", "#667766");
    part("body", box({ size: [1, 1, 1], material: "skin" }));
    part("halo", box({ at: [0, 1.5, 0], size: [0.2, 0.2, 0.2], material: "skin" }));
    geometryException({
      rule: "disconnected-component",
      parts: ["halo"],
      reason: "A magical halo intentionally floats above the body.",
    });
  });
  assert.deepEqual(acknowledged.geometryExceptions, [{
    rule: "disconnected-component",
    parts: ["halo"],
    reason: "A magical halo intentionally floats above the body.",
  }]);
  assert.deepEqual(
    roundTripFigureAsset(acknowledged, "acknowledged source").asset,
    acknowledged,
  );

  assert.throws(
    () => figure("stale_geometry_exception", ({
      box,
      geometryException,
      mat,
      part,
    }) => {
      mat("skin", "#667766");
      part("body", box({ size: [1, 1, 1], material: "skin" }));
      part("halo", box({ at: [0, 0.55, 0], size: [0.2, 0.2, 0.2], material: "skin" }));
      geometryException({
        rule: "disconnected-component",
        parts: ["halo"],
        reason: "This exception should become stale after attachment.",
      });
    }),
    /stale geometry exception/,
  );

  assert.throws(
    () => figure("missing_geometry_reason", ({
      box,
      geometryException,
      mat,
      part,
    }) => {
      mat("skin", "#667766");
      part("body", box({ size: [1, 1, 1], material: "skin" }));
      part("halo", box({ at: [0, 1.5, 0], size: [0.2, 0.2, 0.2], material: "skin" }));
      geometryException({
        rule: "disconnected-component",
        parts: ["halo"],
        reason: "",
      });
    }),
    /requires a nonempty reason/,
  );
});

test("land figures reject sampled non-contact ground penetration", () => {
  assert.throws(
    () => figure("grounded_tail_fixture", ({ box, clip, mat, part }) => {
      mat("skin", "#667766");
      part("body", box({ at: [0, 0.5, 0], size: [1, 1, 1], material: "skin" }));
      part("tail", box({
        parent: "body",
        at: [0, -0.36, 0],
        size: [0.2, 0.24, 0.2],
        material: "skin",
      }));
      clip("walk", {
        role: "locomotion",
        locomotion: {
          kind: "quadruped-walk",
          cycleDistance: 0.5,
          direction: [0, 0, -1],
          units: "figure",
        },
        keys: [
          ["tail", 0, { at: [0, 0, 0] }],
          ["tail", 1, { at: [0, -0.08, 0] }],
        ],
      });
    }),
    /ground-penetration part 'tail'.*clip 'walk' at 1\.000s.*geometryException/s,
  );

  const acknowledged = figure("acknowledged_ground_fixture", ({
    box,
    clip,
    geometryException,
    mat,
    part,
  }) => {
    mat("skin", "#667766");
    part("body", box({ at: [0, 0.5, 0], size: [1, 1, 1], material: "skin" }));
    part("tail", box({
      parent: "body",
      at: [0, -0.36, 0],
      size: [0.2, 0.24, 0.2],
      material: "skin",
    }));
    clip("walk", {
      role: "locomotion",
      locomotion: {
        kind: "quadruped-walk",
        cycleDistance: 0.5,
        direction: [0, 0, -1],
        units: "figure",
      },
      keys: [
        ["tail", 0, { at: [0, 0, 0] }],
        ["tail", 1, { at: [0, -0.08, 0] }],
      ],
    });
    geometryException({
      rule: "ground-penetration",
      parts: ["tail"],
      reason: "This fixture intentionally proves a reasoned ground exception.",
    });
  });
  assert.deepEqual(
    roundTripFigureAsset(acknowledged, "acknowledged ground source").asset,
    acknowledged,
  );

  assert.throws(
    () => figure("stale_ground_fixture", ({
      box,
      clip,
      geometryException,
      mat,
      part,
    }) => {
      mat("skin", "#667766");
      part("body", box({ at: [0, 0.5, 0], size: [1, 1, 1], material: "skin" }));
      part("tail", box({
        parent: "body",
        at: [0, -0.26, 0],
        size: [0.2, 0.24, 0.2],
        material: "skin",
      }));
      clip("walk", {
        role: "locomotion",
        locomotion: {
          kind: "quadruped-walk",
          cycleDistance: 0.5,
          direction: [0, 0, -1],
          units: "figure",
        },
        keys: [["tail", 0, { at: [0, 0, 0] }]],
      });
      geometryException({
        rule: "ground-penetration",
        parts: ["tail"],
        reason: "This should become stale after the tail is raised.",
      });
    }),
    /stale ground exception/,
  );

  assert.doesNotThrow(
    () => figure("declared_contact_fixture", ({ box, clip, mat, part }) => {
      mat("skin", "#667766");
      part("body", box({ at: [0, 0.5, 0], size: [1, 1, 1], material: "skin" }));
      part("foot", box({
        parent: "body",
        at: [0, -0.44, 0],
        size: [0.2, 0.24, 0.2],
        material: "skin",
      }));
      clip("walk", {
        role: "locomotion",
        locomotion: {
          kind: "quadruped-walk",
          cycleDistance: 0.5,
          direction: [0, 0, -1],
          units: "figure",
          contacts: [{
            part: "foot",
            phaseStart: 0,
            phaseEnd: 0.6,
            role: "front-left",
            stanceRatio: 0.6,
          }],
        },
        keys: [["foot", 0, { rot: [0, 0, 0] }]],
      });
    }),
  );

  assert.doesNotThrow(
    () => figure("swimming_tail_fixture", ({ box, clip, mat, part }) => {
      mat("skin", "#667766");
      part("body", box({ at: [0, 0.5, 0], size: [1, 1, 1], material: "skin" }));
      part("tail", box({
        parent: "body",
        at: [0, -0.44, 0],
        size: [0.2, 0.24, 0.2],
        material: "skin",
      }));
      clip("swim", {
        role: "locomotion",
        locomotion: {
          kind: "swim",
          cycleDistance: 0.8,
          direction: [0, 0, -1],
          units: "figure",
        },
        keys: [["tail", 0, { rot: [0, 0, 0] }]],
      });
    }),
  );
});

test("canonical surfaces require reasoned exact-face exceptions", () => {
  assert.throws(
    () => figure("coplanar_fixture", ({ box, mat, part }) => {
      mat("skin", "#667766");
      mat("accent", "#aa6655");
      part("neck", box({ size: [1, 1, 1], material: "skin" }));
      part("head", box({
        parent: "neck",
        at: [0, 0.75, 0],
        size: [1, 1, 0.8],
        material: "accent",
        faces: { west: { material: "skin" } },
      }));
    }),
    /coplanar-overlap head\.east \/ neck\.east.*surfaceException/s,
  );

  const acknowledged = figure("acknowledged_coplanar_fixture", ({
    box,
    mat,
    part,
    surfaceException,
  }) => {
    mat("skin", "#667766");
    mat("accent", "#aa6655");
    part("neck", box({ size: [1, 1, 1], material: "skin" }));
    part("head", box({
      parent: "neck",
      at: [0, 0.75, 0],
      size: [1, 1, 0.8],
      material: "accent",
      faces: { west: { material: "skin" } },
    }));
    surfaceException({
      rule: "coplanar-overlap",
      faces: [
        { part: "neck", face: "east" },
        { part: "head", face: "east" },
      ],
      reason: "The contrasting inset is intentionally flush on this side.",
    });
  });
  assert.equal(acknowledged.surfaceExceptions?.[0]?.reason.includes("intentionally"), true);
  assert.deepEqual(
    roundTripFigureAsset(acknowledged, "acknowledged surface source").asset,
    acknowledged,
  );

  assert.throws(
    () => figure("stale_surface_exception", ({
      box,
      mat,
      part,
      surfaceException,
    }) => {
      mat("skin", "#667766");
      mat("accent", "#aa6655");
      part("neck", box({ size: [1, 1, 1], material: "skin" }));
      part("head", box({
        parent: "neck",
        at: [0, 0.75, 0],
        size: [0.9, 1, 0.8],
        material: "accent",
      }));
      surfaceException({
        rule: "coplanar-overlap",
        faces: [
          { part: "neck", face: "east" },
          { part: "head", face: "east" },
        ],
        reason: "This exception should become stale after adding a side step.",
      });
    }),
    /stale surface exception/,
  );

  assert.throws(
    () => figure("missing_surface_reason", ({
      box,
      mat,
      part,
      surfaceException,
    }) => {
      mat("skin", "#667766");
      mat("accent", "#aa6655");
      part("neck", box({ size: [1, 1, 1], material: "skin" }));
      part("head", box({
        parent: "neck",
        at: [0, 0.75, 0],
        size: [1, 1, 0.8],
        material: "accent",
      }));
      surfaceException({
        rule: "coplanar-overlap",
        faces: [
          { part: "neck", face: "east" },
          { part: "head", face: "east" },
        ],
        reason: "",
      });
    }),
    /surface exception 0 requires a nonempty reason/,
  );

  assert.throws(
    () => figure("animated_coplanar_fixture", ({ box, clip, mat, part }) => {
      mat("skin", "#667766");
      mat("accent", "#aa6655");
      part("neck", box({ size: [1, 1, 1], material: "skin" }));
      part("head", box({
        parent: "neck",
        at: [0.1, 0.75, 0],
        size: [1, 1, 0.8],
        material: "accent",
        faces: { west: { material: "skin" } },
      }));
      clip("settle", {
        loop: false,
        keys: [
          ["head", 0, { at: [0, 0, 0] }],
          ["head", 1, { at: [-0.1, 0, 0] }],
        ],
      });
    }),
    /clip 'settle' at 1\.000s.*surfaceException/s,
  );
});

test("animated head sockets require a stable lateral surface margin", () => {
  assert.throws(
    () => figure("narrow_head_socket", ({ box, clip, mat, part }) => {
      mat("neck", "#665544");
      mat("head", "#aa8866");
      part("neck", box({ size: [1, 1, 1], material: "neck" }));
      part("head", box({
        parent: "neck",
        at: [0, 0.75, 0],
        size: [1.02, 1, 0.8],
        material: "head",
      }));
      clip("nod", {
        keys: [
          ["head", 0, { rot: [0, 0, 0] }],
          ["head", 1, { rot: [8, 0, 0] }],
        ],
      });
    }),
    /articulated-seam-margin head\.east \/ neck\.east.*margin 0\.010, required 0\.020/s,
  );

  assert.doesNotThrow(
    () => figure("safe_head_socket", ({ box, clip, mat, part }) => {
      mat("neck", "#665544");
      mat("head", "#aa8866");
      part("neck", box({ size: [1, 1, 1], material: "neck" }));
      part("head", box({
        parent: "neck",
        at: [0, 0.75, 0],
        size: [1.08, 1, 0.8],
        material: "head",
      }));
      clip("nod", {
        keys: [
          ["head", 0, { rot: [0, 0, 0] }],
          ["head", 1, { rot: [8, 0, 0] }],
        ],
      });
    }),
  );

  const acknowledged = figure("acknowledged_head_socket", ({
    box,
    clip,
    mat,
    part,
    surfaceException,
  }) => {
    mat("neck", "#665544");
    mat("head", "#aa8866");
    part("neck", box({ size: [1, 1, 1], material: "neck" }));
    part("head", box({
      parent: "neck",
      at: [0, 0.75, 0],
      size: [1.02, 1, 0.8],
      material: "head",
    }));
    clip("nod", {
      keys: [
        ["head", 0, { rot: [0, 0, 0] }],
        ["head", 1, { rot: [8, 0, 0] }],
      ],
    });
    for (const face of ["east", "west"] as const) {
      surfaceException({
        rule: "articulated-seam-margin",
        faces: [
          { part: "head", face },
          { part: "neck", face },
        ],
        reason: "This test fixture deliberately exercises a narrow animated socket.",
      });
    }
  });
  assert.equal(acknowledged.surfaceExceptions?.length, 2);
  assert.deepEqual(
    roundTripFigureAsset(acknowledged, "acknowledged head socket").asset,
    acknowledged,
  );
});

test("preserves named action presentation and default-clip metadata", () => {
  const asset = figure("action_figure", ({ box, clip, defaultClip, mat, part }) => {
    mat("shell", "#667766");
    part("body", box({ size: [1, 1, 1], material: "shell" }));
    clip("roll_up", {
      label: "Roll up",
      role: "action",
      loop: false,
      keys: [
        ["body", 0, { rot: [0, 0, 0] }],
        ["body", 0.5, { rot: [90, 0, 0] }],
      ],
    });
    clip("unroll", {
      label: "Unroll",
      role: "action",
      nextClip: "roll_up",
      loop: false,
      keys: [
        ["body", 0, { rot: [90, 0, 0] }],
        ["body", 0.5, { rot: [0, 0, 0] }],
      ],
    });
    defaultClip("roll_up");
  });

  const parsed = roundTripFigureAsset(asset, "action source").asset;
  assert.equal(parsed.defaultClip, "roll_up");
  assert.equal(parsed.clips.roll_up?.label, "Roll up");
  assert.equal(parsed.clips.roll_up?.role, "action");
  assert.equal(parsed.clips.unroll?.nextClip, "roll_up");
});

test("procedural cycle tracks clamp their baked scalar values", () => {
  const asset = figure("constrained_cycle", ({
    bob,
    box,
    followThrough,
    mat,
    part,
    swing,
    walkCycle,
  }) => {
    mat("skin", "#667766");
    part("body", box({ size: [1, 1, 1], material: "skin" }));
    part("head", box({ parent: "body", size: [0.5, 0.5, 0.5], material: "skin" }));
    part("tail", box({ parent: "body", size: [0.2, 0.2, 0.8], material: "skin" }));
    walkCycle("hop", {
      duration: 1,
      samples: 9,
      tracks: [
        bob("body", { axis: "y", amount: 0.2, center: 0, phase: 0.5, min: 0 }),
        swing("head", { axis: "x", degrees: 20, min: -5, max: 5 }),
        followThrough("tail", {
          source: "body",
          sourceChannel: "pos",
          sourceAxis: "y",
          axis: "x",
          degrees: 12,
          min: -3,
          max: 3,
        }),
      ],
    });
  });

  const clip = asset.clips.hop;
  assert.ok(clip);
  const bodyLift = clip.keys
    .filter(([part]) => part === "body")
    .map(([, , transform]) => transform.at?.[1]);
  assert.ok(bodyLift.every((value) => value !== undefined && value >= 0));
  assert.ok(bodyLift.filter((value) => value === 0).length >= 5);
  assert.equal(Math.max(...bodyLift.filter((value): value is number => value !== undefined)), 0.2);

  const headSwing = clip.keys
    .filter(([part]) => part === "head")
    .map(([, , transform]) => transform.rot?.[0]);
  assert.ok(headSwing.every((value) => value !== undefined && value >= -5 && value <= 5));

  const tailFollow = clip.keys
    .filter(([part]) => part === "tail")
    .map(([, , transform]) => transform.rot?.[0]);
  assert.ok(tailFollow.every((value) => value !== undefined && value >= -3 && value <= 3));

  assert.throws(
    () => figure("invalid_constraint", ({ bob, box, mat, part, walkCycle }) => {
      mat("skin", "#667766");
      part("body", box({ size: [1, 1, 1], material: "skin" }));
      walkCycle("hop", {
        tracks: [bob("body", { amount: 0.2, min: 1, max: 0 })],
      });
    }),
    /min must not exceed max/,
  );
});

test("procedural ground contacts project a foot through its ancestor hinge", () => {
  const asset = figure("projected_contact", ({ bob, box, mat, part, walkCycle }) => {
    mat("skin", "#667766");
    part("body", box({ at: [0, 0.75, 0], size: [0.4, 0.4, 0.4], material: "skin" }));
    part("leg", box({
      parent: "body",
      at: [0, -0.45, 0],
      size: [0.2, 0.5, 0.2],
      material: "skin",
      joint: { pivot: [0, 0.25, 0], axis: [1, 0, 0] },
    }));
    part("foot", box({
      parent: "leg",
      at: [0, -0.2, -0.08],
      size: [0.3, 0.2, 0.4],
      material: "skin",
    }));
    walkCycle("bounce", {
      duration: 1,
      samples: 17,
      groundContacts: [{
        contactPart: "foot",
        solvePart: "leg",
        axis: "x",
        minCorrectionDegrees: -65,
        maxCorrectionDegrees: 65,
      }],
      tracks: [bob("body", { axis: "y", amount: 0.06, phase: 0.5 })],
    });
  });

  const clip = asset.clips.bounce;
  assert.ok(clip);
  const bodyLift = clip.keys
    .filter(([part]) => part === "body")
    .map(([, , transform]) => transform.at?.[1]);
  assert.ok(bodyLift.some((value) => value !== undefined && value < 0));

  const hingeAngles = clip.keys
    .filter(([part]) => part === "leg")
    .map(([, , transform]) => transform.rot?.[0]);
  assert.equal(hingeAngles.length, 17);
  assert.ok(hingeAngles.some((value) => value !== undefined && Math.abs(value) > 0.1));
  assert.ok(hingeAngles.some((value) => value === 0));

  const footAngles = clip.keys
    .filter(([part]) => part === "foot")
    .map(([, , transform]) => transform.rot?.[0]);
  assert.equal(footAngles.length, 17);
  for (let index = 0; index < hingeAngles.length; index += 1) {
    assert.ok(Math.abs((hingeAngles[index] ?? 0) + (footAngles[index] ?? 0)) < 1e-9);
  }
});

test("swim macro exports ordinary body tail fin keys and locomotion", () => {
  const asset = figure("swimmer", ({ mat, part, box, swim }) => {
    mat("skin", "#447799");
    part("body", box({ size: [1, 1, 1], material: "skin" }));
    part("tail", box({ parent: "body", size: [0.2, 0.6, 0.6], material: "skin" }));
    part("tail_tip", box({ parent: "tail", size: [0.1, 0.8, 0.5], material: "skin" }));
    part("fin_l", box({ parent: "body", size: [0.4, 0.1, 0.3], material: "skin" }));
    part("fin_r", box({ parent: "body", size: [0.4, 0.1, 0.3], material: "skin" }));
    swim("swim", {
      duration: 1,
      samples: 5,
      body: "body",
      bodyBob: 0.02,
      bodySwayDegrees: 3,
      cycleDistance: 1.4,
      leftFin: "fin_l",
      rightFin: "fin_r",
      tail: "tail",
      tailTip: "tail_tip",
    });
  });

  const clip = asset.clips.swim;
  assert.ok(clip);
  assert.equal(clip.locomotion?.kind, "swim");
  assert.equal(clip.locomotion?.cycleDistance, 1.4);
  assert.deepEqual(clip.locomotion?.direction, [0, 0, -1]);
  assert.equal(clip.locomotion?.contacts, undefined);
  assert.deepEqual(
    new Set(clip.keys.map(([part]) => part)),
    new Set(["body", "tail", "tail_tip", "fin_l", "fin_r"]),
  );

  const finKeys = clip.keys.filter(([, time]) => time === 0.25);
  const leftFin = finKeys.find(([part]) => part === "fin_l")?.[2].rot;
  const rightFin = finKeys.find(([part]) => part === "fin_r")?.[2].rot;
  assert.ok(leftFin);
  assert.ok(rightFin);
  assert.equal(leftFin[2], -rightFin[2]);
});

test("slither macro exports a phased segment wave and locomotion", () => {
  const asset = figure("slitherer", ({ mat, part, box, slither }) => {
    mat("skin", "#447744");
    part("body", box({ at: [0, 0.15, 0], size: [0.4, 0.3, 0.7], material: "skin" }));
    part("middle", box({ parent: "body", size: [0.3, 0.25, 0.6], material: "skin" }));
    part("tail", box({ parent: "middle", size: [0.2, 0.2, 0.5], material: "skin" }));
    slither("slither", {
      duration: 1,
      samples: 5,
      body: "body",
      bodyBob: 0.01,
      cycleDistance: 0.8,
      degrees: 10,
      phaseStep: 0.125,
      segments: ["body", "middle", "tail"],
    });
  });

  const clip = asset.clips.slither;
  assert.ok(clip);
  assert.equal(clip.locomotion?.kind, "slither");
  assert.equal(clip.locomotion?.cycleDistance, 0.8);
  assert.deepEqual(clip.locomotion?.direction, [0, 0, -1]);
  assert.equal(clip.locomotion?.contacts, undefined);
  assert.deepEqual(
    new Set(clip.keys.map(([part]) => part)),
    new Set(["body", "middle", "tail"]),
  );

  const quarterKeys = clip.keys.filter(([, time]) => time === 0.25);
  const body = quarterKeys.find(([part]) => part === "body")?.[2];
  const middle = quarterKeys.find(([part]) => part === "middle")?.[2];
  assert.ok(body?.rot);
  assert.ok(body.at);
  assert.ok(middle?.rot);
  assert.notEqual(body.rot[1], middle.rot[1]);
});

test("canonical and legacy examples cross the canonical JSON boundary", async () => {
  const canonicalSources = await discoverFigureSources("examples");
  const legacySources = await discoverFigureSources("legacy-examples");
  assert.ok(canonicalSources.length > 0);
  assert.ok(legacySources.length > 0);

  const names = new Set<string>();
  for (const sourcePath of [...canonicalSources, ...legacySources]) {
    const document = await loadFigureJsonDocument(sourcePath);
    assert.equal(serializeFigureAsset(document.asset), document.json);
    assert.equal(parseFigureAssetJson(document.json, sourcePath).name, document.asset.name);
    assert.equal(names.has(document.asset.name), false, `duplicate figure name '${document.asset.name}'`);
    names.add(document.asset.name);
  }

  for (const sourcePath of canonicalSources) {
    const document = await loadFigureJsonDocument(sourcePath);
    assert.doesNotThrow(() => assertBoxOnlyFigure(document.asset), sourcePath);
  }
});

test("reports invalid direct JSON before it reaches Three.js", () => {
  assert.throws(
    () => parseFigureAssetJson("{\"schemaVersion\":1}", "broken.figure.json"),
    /Expected 'broken\.figure\.json' to contain a schema-v1 FigureAsset/,
  );
});

function tinyFigure(): FigureAsset {
  return {
    schemaVersion: 1,
    name: "tiny",
    materials: {
      white: { color: "#ffffff" },
    },
    textures: {},
    parts: [
      {
        name: "body",
        at: [0, 0, 0],
        material: "white",
        primitive: { kind: "box", size: [1, 1, 1] },
      },
    ],
    clips: {},
  };
}

async function discoverFigureSources(directory: string): Promise<string[]> {
  const root = path.join(assetLabRoot, directory);
  const entries = await fs.readdir(root, { withFileTypes: true });
  return entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(root, entry.name, "figure.ts"))
    .sort();
}
