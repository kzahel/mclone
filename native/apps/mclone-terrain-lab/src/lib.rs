#![forbid(unsafe_code)]

use mclone_terrain_view::{
    CanonicalTerrainStage, TerrainPreviewDrawOptions, TerrainPreviewLayer,
    TerrainPreviewProjectionKind, TerrainPreviewSource, TerrainPreviewSplitLayout,
    TerrainPreviewView,
};
use mclone_worldgen::terrain_preview::TerrainPreviewContentStage;

#[cfg(target_arch = "wasm32")]
mod canonical_coordinator_web;
#[cfg(target_arch = "wasm32")]
mod canonical_mailbox_web;
#[cfg(target_arch = "wasm32")]
mod canonical_web;
#[cfg(target_arch = "wasm32")]
mod canonical_worker_web;
#[cfg(target_arch = "wasm32")]
mod landform_plan_web;
#[cfg(any(target_arch = "wasm32", test))]
mod navigation;
#[cfg(target_arch = "wasm32")]
mod navigation_web;
#[cfg(target_arch = "wasm32")]
mod runtime_exact_worker_web;
#[cfg(target_arch = "wasm32")]
mod runtime_web;
#[cfg(target_arch = "wasm32")]
mod semantic_terrain_sandbox_web;
#[cfg(target_arch = "wasm32")]
mod streamed_plan_atlas_web;
mod visual_assets;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use canonical_web::{CanonicalTerrainLab, mclone_terrain_lab_create_canonical};
#[cfg(target_arch = "wasm32")]
pub use canonical_worker_web::{CanonicalTerrainWorkerActor, CanonicalTerrainWorkerDispatch};
#[cfg(target_arch = "wasm32")]
pub use landform_plan_web::TerrainLabLandformPlanCompiler;
#[cfg(target_arch = "wasm32")]
pub use navigation_web::{TerrainLabNavigationSession, TerrainLabNavigationUpdate};
#[cfg(target_arch = "wasm32")]
pub use runtime_exact_worker_web::{
    TerrainRuntimeExactWorkerActor, TerrainRuntimeExactWorkerDispatch,
};
#[cfg(target_arch = "wasm32")]
pub use runtime_web::{
    TerrainRuntimeCompositionLab, mclone_terrain_lab_create_runtime_composition,
};
#[cfg(target_arch = "wasm32")]
pub use semantic_terrain_sandbox_web::{
    TerrainLabSemanticTerrainSandboxCompiler, semantic_terrain_sandbox_suite_sha256,
};
#[cfg(target_arch = "wasm32")]
pub use streamed_plan_atlas_web::TerrainLabStreamedPlanAtlasCompiler;
#[cfg(target_arch = "wasm32")]
pub use web::{TerrainLab, mclone_terrain_lab_create};

pub fn terrain_preview_options(
    source: &str,
    view: &str,
    layer: &str,
) -> Result<TerrainPreviewDrawOptions, String> {
    Ok(TerrainPreviewDrawOptions {
        source: match source.trim().to_ascii_lowercase().as_str() {
            "gpu" => TerrainPreviewSource::Gpu,
            "reference" | "cpu" => TerrainPreviewSource::Reference,
            "macro" | "fast" => TerrainPreviewSource::Macro,
            "split" | "compare" => TerrainPreviewSource::Split,
            other => {
                return Err(format!(
                    "unsupported terrain preview source {other:?}; expected gpu, reference, macro, \
                     or split"
                ));
            }
        },
        view: match view.trim().to_ascii_lowercase().as_str() {
            "map" | "2d" => TerrainPreviewView::Map,
            "3d" | "terrain" => TerrainPreviewView::ThreeDimensional,
            other => {
                return Err(format!(
                    "unsupported terrain preview view {other:?}; expected map or 3d"
                ));
            }
        },
        layer: match layer.trim().to_ascii_lowercase().as_str() {
            "terrain" | "rendered" => TerrainPreviewLayer::Terrain,
            "height" => TerrainPreviewLayer::Height,
            "error" | "difference" => TerrainPreviewLayer::Error,
            "continentalness" | "continent" => TerrainPreviewLayer::Continentalness,
            "climate" => TerrainPreviewLayer::Climate,
            "rivers" | "river" => TerrainPreviewLayer::Rivers,
            "wetlands" | "wetland" => TerrainPreviewLayer::Wetlands,
            "landforms" | "landform" => TerrainPreviewLayer::Landforms,
            "biomes" | "biome" => TerrainPreviewLayer::Biomes,
            "surface" | "surface-recipe" => TerrainPreviewLayer::SurfaceRecipe,
            "streams" | "planned-streams" => TerrainPreviewLayer::PlannedStreams,
            "forests" | "forest" | "vegetation" => TerrainPreviewLayer::Forests,
            other => {
                return Err(format!(
                    "unsupported terrain preview layer {other:?}; expected terrain, height, error, \
                     continentalness, climate, rivers, wetlands, landforms, biomes, surface, \
                     streams, or forests"
                ));
            }
        },
        split_layout: TerrainPreviewSplitLayout::Columns,
    })
}

pub fn terrain_preview_option_labels(
    options: TerrainPreviewDrawOptions,
) -> (&'static str, &'static str, &'static str) {
    let source = match options.source {
        TerrainPreviewSource::Gpu => "gpu",
        TerrainPreviewSource::Reference => "reference",
        TerrainPreviewSource::Macro => "macro",
        TerrainPreviewSource::Split => "split",
    };
    let view = match options.view {
        TerrainPreviewView::Map => "map",
        TerrainPreviewView::ThreeDimensional => "3d",
    };
    let layer = match options.layer {
        TerrainPreviewLayer::Terrain => "terrain",
        TerrainPreviewLayer::Height => "height",
        TerrainPreviewLayer::Error => "error",
        TerrainPreviewLayer::Continentalness => "continentalness",
        TerrainPreviewLayer::Climate => "climate",
        TerrainPreviewLayer::Rivers => "rivers",
        TerrainPreviewLayer::Wetlands => "wetlands",
        TerrainPreviewLayer::Landforms => "landforms",
        TerrainPreviewLayer::Biomes => "biomes",
        TerrainPreviewLayer::SurfaceRecipe => "surface",
        TerrainPreviewLayer::PlannedStreams => "streams",
        TerrainPreviewLayer::Forests => "forests",
    };
    (source, view, layer)
}

pub fn terrain_preview_content_stage(value: &str) -> Result<TerrainPreviewContentStage, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "base" => Ok(TerrainPreviewContentStage::Base),
        "hydrology" | "water" => Ok(TerrainPreviewContentStage::Hydrology),
        "structured" | "streams" => Ok(TerrainPreviewContentStage::Structured),
        "surface" => Ok(TerrainPreviewContentStage::Surface),
        "cover" | "vegetation" => Ok(TerrainPreviewContentStage::Cover),
        other => Err(format!(
            "unsupported LOD content stage {other:?}; expected base, hydrology, structured, \
             surface, or cover"
        )),
    }
}

pub fn terrain_preview_projection_kind(
    value: &str,
) -> Result<TerrainPreviewProjectionKind, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "orthographic" | "ortho" => Ok(TerrainPreviewProjectionKind::Orthographic),
        "perspective" => Ok(TerrainPreviewProjectionKind::Perspective),
        other => Err(format!(
            "unsupported terrain preview projection {other:?}; expected orthographic or perspective"
        )),
    }
}

pub fn terrain_preview_split_layout(value: &str) -> Result<TerrainPreviewSplitLayout, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "columns" | "side-by-side" => Ok(TerrainPreviewSplitLayout::Columns),
        "rows" | "stacked" => Ok(TerrainPreviewSplitLayout::Rows),
        other => Err(format!(
            "unsupported Terrain Lab split layout {other:?}; expected columns or rows"
        )),
    }
}

pub fn canonical_terrain_stage(value: &str) -> Result<CanonicalTerrainStage, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "surface" => Ok(CanonicalTerrainStage::Surface),
        "final" | "features" | "final-features" => Ok(CanonicalTerrainStage::FinalFeatures),
        other => Err(format!(
            "unsupported canonical terrain stage {other:?}; expected surface or final"
        )),
    }
}

pub fn canonical_terrain_stage_label(stage: CanonicalTerrainStage) -> &'static str {
    match stage {
        CanonicalTerrainStage::Surface => "surface",
        CanonicalTerrainStage::FinalFeatures => "final",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_browser_option_vocabulary() {
        let options = terrain_preview_options("split", "3d", "terrain").unwrap();
        assert_eq!(
            terrain_preview_option_labels(options),
            ("split", "3d", "terrain")
        );
        assert_eq!(
            terrain_preview_option_labels(TerrainPreviewDrawOptions::default()),
            ("reference", "3d", "terrain")
        );
        assert_eq!(
            terrain_preview_options("CPU", "2d", "difference").unwrap(),
            TerrainPreviewDrawOptions {
                source: TerrainPreviewSource::Reference,
                view: TerrainPreviewView::Map,
                layer: TerrainPreviewLayer::Error,
                split_layout: TerrainPreviewSplitLayout::Columns,
            }
        );
        assert_eq!(
            terrain_preview_option_labels(
                terrain_preview_options("gpu", "map", "landform").unwrap()
            ),
            ("gpu", "map", "landforms")
        );
        assert_eq!(
            terrain_preview_option_labels(
                terrain_preview_options("gpu", "map", "vegetation").unwrap()
            ),
            ("gpu", "map", "forests")
        );
    }

    #[test]
    fn rejects_unknown_browser_options() {
        assert!(terrain_preview_options("both-ish", "3d", "terrain").is_err());
        assert!(terrain_preview_options("gpu", "perspective", "terrain").is_err());
        assert!(terrain_preview_options("gpu", "3d", "geology").is_err());
        assert!(terrain_preview_content_stage("lighting").is_err());
        assert!(terrain_preview_projection_kind("isometric").is_err());
        assert!(terrain_preview_split_layout("diagonal").is_err());
        assert_eq!(
            terrain_preview_content_stage("streams").unwrap(),
            TerrainPreviewContentStage::Structured
        );
        assert_eq!(
            terrain_preview_projection_kind("ortho").unwrap(),
            TerrainPreviewProjectionKind::Orthographic
        );
        assert_eq!(
            terrain_preview_projection_kind("perspective").unwrap(),
            TerrainPreviewProjectionKind::Perspective
        );
        assert_eq!(
            terrain_preview_split_layout("stacked").unwrap(),
            TerrainPreviewSplitLayout::Rows
        );
    }

    #[test]
    fn parses_canonical_stage_vocabulary() {
        assert_eq!(
            canonical_terrain_stage("surface").unwrap(),
            CanonicalTerrainStage::Surface
        );
        assert_eq!(
            canonical_terrain_stage("Final Features").unwrap_err(),
            "unsupported canonical terrain stage \"final features\"; expected surface or final"
        );
        assert_eq!(
            canonical_terrain_stage_label(canonical_terrain_stage("features").unwrap()),
            "final"
        );
    }

    #[test]
    fn exact_and_lod_landmarks_share_projection_and_rotation() {
        let target_y = 73.0;
        for kind in [
            TerrainPreviewProjectionKind::Orthographic,
            TerrainPreviewProjectionKind::Perspective,
        ] {
            let camera = mclone_terrain_view::TerrainPreviewCamera::new(0.83, 0.57, kind).unwrap();
            let projection = mclone_terrain_view::terrain_preview_projection(
                64,
                TerrainPreviewView::ThreeDimensional,
                camera,
                341,
                529,
                target_y,
            );
            let chunk_camera = mclone_render::chunk::ChunkCamera {
                eye: [
                    0.5 + projection.eye_offset[0],
                    projection.target_y + projection.eye_offset[1],
                    0.5 + projection.eye_offset[2],
                ],
                target: [0.5, projection.target_y, 0.5],
                up: projection.up,
                fov_y_radians: projection.fov_y_radians,
                z_near: projection.z_near,
                z_far: projection.z_far,
            };
            let render_view = match kind {
                TerrainPreviewProjectionKind::Orthographic => chunk_camera
                    .render_orthographic_view(341, 529, projection.vertical_half_extent * 2.0),
                TerrainPreviewProjectionKind::Perspective => chunk_camera.render_view(341, 529),
            };

            for relative in [[12.0, 88.0, -8.0], [-19.0, 61.0, 23.0]] {
                let exact = project_matrix(
                    render_view.uniform_matrix(),
                    [relative[0] + 0.5, relative[1], relative[2] + 0.5, 1.0],
                );
                let lod = project_preview(projection, relative);
                assert!(
                    (exact[0] - lod[0]).abs() < 1.0e-5 && (exact[1] - lod[1]).abs() < 1.0e-5,
                    "{kind:?} landmark drifted: exact={exact:?}, lod={lod:?}"
                );
            }
        }
    }

    fn project_matrix(matrix: [[f32; 4]; 4], point: [f32; 4]) -> [f32; 2] {
        let clip = [0, 1, 2, 3].map(|row| {
            (0..4)
                .map(|column| matrix[column][row] * point[column])
                .sum::<f32>()
        });
        [clip[0] / clip[3], clip[1] / clip[3]]
    }

    fn project_preview(
        projection: mclone_terrain_view::TerrainPreviewProjection,
        point: [f32; 3],
    ) -> [f32; 2] {
        let eye = [
            projection.eye_offset[0],
            projection.target_y + projection.eye_offset[1],
            projection.eye_offset[2],
        ];
        let target = [0.0, projection.target_y, 0.0];
        let forward = normalize(subtract(target, eye));
        let right = normalize(cross(forward, projection.up));
        let up = normalize(cross(right, forward));
        let from_eye = subtract(point, eye);
        let depth = dot(from_eye, forward).max(projection.z_near);
        let half_height = match projection.kind {
            TerrainPreviewProjectionKind::Orthographic => projection.vertical_half_extent,
            TerrainPreviewProjectionKind::Perspective => {
                depth * (projection.fov_y_radians * 0.5).tan()
            }
        };
        [
            dot(from_eye, right) / (half_height * projection.aspect),
            dot(from_eye, up) / half_height,
        ]
    }

    fn subtract(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
        [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
    }

    fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
        [
            left[1] * right[2] - left[2] * right[1],
            left[2] * right[0] - left[0] * right[2],
            left[0] * right[1] - left[1] * right[0],
        ]
    }

    fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
        left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
    }

    fn normalize(vector: [f32; 3]) -> [f32; 3] {
        let length = dot(vector, vector).sqrt();
        [vector[0] / length, vector[1] / length, vector[2] / length]
    }
}
