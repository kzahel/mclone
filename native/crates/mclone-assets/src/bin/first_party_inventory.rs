#![forbid(unsafe_code)]

use std::path::PathBuf;

use mclone_assets::{
    AssetConsumerKind, AssetRequirementPolicy, FirstPartyVisualClass,
    canonical_first_party_asset_inventory,
};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = parse_output(std::env::args_os().skip(1))?;
    let inventory = canonical_first_party_asset_inventory();
    let payload = json!({
        "schema_version": 1,
        "asset_schema": "mclone-visuals-v1",
        "block_visuals": inventory.block_visuals.iter().map(|visual| json!({
            "state_id": visual.state.id.0,
            "state": visual.state.canonical_key(),
            "class": visual_class_name(visual.class),
            "material": visual.material.as_ref().map(ToString::to_string),
        })).collect::<Vec<_>>(),
        "materials": inventory.materials.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "direct_assets": inventory.direct_assets.iter().map(|asset| json!({
            "path": asset.path.as_str(),
            "consumer": consumer_name(asset.consumer),
            "policy": policy_name(asset.policy),
        })).collect::<Vec<_>>(),
    });
    let bytes = serde_json::to_vec_pretty(&payload)?;

    if let Some(output) = output {
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&output, bytes)?;
        println!("wrote {}", output.display());
    } else {
        println!("{}", String::from_utf8(bytes)?);
    }
    Ok(())
}

fn parse_output(
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<Option<PathBuf>, String> {
    let mut arguments = arguments.into_iter();
    let mut output = None;
    while let Some(argument) = arguments.next() {
        if argument == "--output" {
            let path = arguments
                .next()
                .ok_or_else(|| "--output requires a path".to_owned())?;
            output = Some(PathBuf::from(path));
        } else {
            return Err(format!("unknown argument `{}`", argument.to_string_lossy()));
        }
    }
    Ok(output)
}

const fn visual_class_name(class: FirstPartyVisualClass) -> &'static str {
    match class {
        FirstPartyVisualClass::Empty => "empty",
        FirstPartyVisualClass::Solid => "solid",
        FirstPartyVisualClass::Slab => "slab",
        FirstPartyVisualClass::Stair => "stair",
        FirstPartyVisualClass::CrossedPlane => "crossed_plane",
        FirstPartyVisualClass::Flat => "flat",
        FirstPartyVisualClass::Fluid => "fluid",
    }
}

const fn consumer_name(consumer: AssetConsumerKind) -> &'static str {
    match consumer {
        AssetConsumerKind::FarLod => "far_lod",
        AssetConsumerKind::ActorTexture => "actor_texture",
        AssetConsumerKind::ActorFigure => "actor_figure",
        AssetConsumerKind::ScreenEffect => "screen_effect",
        AssetConsumerKind::TerrainColorMap => "terrain_color_map",
        AssetConsumerKind::Audio => "audio",
    }
}

const fn policy_name(policy: AssetRequirementPolicy) -> &'static str {
    match policy {
        AssetRequirementPolicy::Required => "required",
        AssetRequirementPolicy::Optional => "optional",
        AssetRequirementPolicy::Suppressible => "suppressible",
    }
}
