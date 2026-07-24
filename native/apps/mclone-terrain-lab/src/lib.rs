#![forbid(unsafe_code)]

use mclone_terrain_view::{
    CanonicalTerrainStage, TerrainPreviewDrawOptions, TerrainPreviewLayer, TerrainPreviewSource,
    TerrainPreviewView,
};
use mclone_worldgen::terrain_preview::TerrainPreviewContentStage;

#[cfg(target_arch = "wasm32")]
mod canonical_web;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use canonical_web::{CanonicalTerrainLab, mclone_terrain_lab_create_canonical};
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
            "split" | "compare" => TerrainPreviewSource::Split,
            other => {
                return Err(format!(
                    "unsupported terrain preview source {other:?}; expected gpu, reference, or split"
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
            "biomes" | "biome" => TerrainPreviewLayer::Biomes,
            "surface" | "surface-recipe" => TerrainPreviewLayer::SurfaceRecipe,
            "streams" | "planned-streams" => TerrainPreviewLayer::PlannedStreams,
            other => {
                return Err(format!(
                    "unsupported terrain preview layer {other:?}; expected terrain, height, error, \
                     continentalness, climate, rivers, wetlands, biomes, surface, or streams"
                ));
            }
        },
    })
}

pub fn terrain_preview_option_labels(
    options: TerrainPreviewDrawOptions,
) -> (&'static str, &'static str, &'static str) {
    let source = match options.source {
        TerrainPreviewSource::Gpu => "gpu",
        TerrainPreviewSource::Reference => "reference",
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
        TerrainPreviewLayer::Biomes => "biomes",
        TerrainPreviewLayer::SurfaceRecipe => "surface",
        TerrainPreviewLayer::PlannedStreams => "streams",
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
            }
        );
    }

    #[test]
    fn rejects_unknown_browser_options() {
        assert!(terrain_preview_options("both-ish", "3d", "terrain").is_err());
        assert!(terrain_preview_options("gpu", "perspective", "terrain").is_err());
        assert!(terrain_preview_options("gpu", "3d", "geology").is_err());
        assert!(terrain_preview_content_stage("lighting").is_err());
        assert_eq!(
            terrain_preview_content_stage("streams").unwrap(),
            TerrainPreviewContentStage::Structured
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
}
