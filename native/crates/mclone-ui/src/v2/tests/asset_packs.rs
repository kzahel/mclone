use super::*;
use crate::{
    AssetPackUiApplyState, AssetPackUiCoverage, AssetPackUiId, AssetPackUiOrigin, AssetPackUiRow,
    AssetPackUiRowStatus, AssetPacksUiState, WorldCatalogUiText,
};

const AUTHORED: AssetPackUiId = AssetPackUiId(11);
const REFERENCE: AssetPackUiId = AssetPackUiId(12);
const FALLBACK: AssetPackUiId = AssetPackUiId(13);

fn asset_pack_state(dirty: bool) -> AssetPacksUiState {
    let mut authored = AssetPackUiRow::new(
        AUTHORED,
        "mclone-authored",
        "Mclone Original Assets",
        AssetPackUiOrigin::FirstParty,
    );
    authored.enabled = dirty;
    authored.status = if dirty {
        AssetPackUiRowStatus::Enabled
    } else {
        AssetPackUiRowStatus::Disabled
    };

    let mut reference = AssetPackUiRow::new(
        REFERENCE,
        "minecraft-1.17.1-reference",
        "Minecraft 1.17.1 Reference",
        AssetPackUiOrigin::MinecraftReference,
    );
    reference.available = false;
    reference.status = AssetPackUiRowStatus::Unavailable;
    reference.detail = WorldCatalogUiText::new("Not installed");

    let mut fallback = AssetPackUiRow::new(
        FALLBACK,
        "mclone-generated-fallback",
        "Generated Missing Assets",
        AssetPackUiOrigin::Generated,
    );
    fallback.enabled = true;
    fallback.active = true;
    fallback.disableable = false;
    fallback.status = AssetPackUiRowStatus::Active;
    fallback.detail = WorldCatalogUiText::new("Always active");

    let mut state = AssetPacksUiState {
        effective_label: WorldCatalogUiText::new(if dirty {
            "Mclone Original"
        } else {
            "Generated Fallback Only"
        }),
        coverage: AssetPackUiCoverage {
            authored: 11,
            required: 146,
            generated: 135,
            proprietary_free: true,
            ..AssetPackUiCoverage::default()
        },
        dirty,
        ..AssetPacksUiState::empty()
    };
    state.set_rows(&[authored, reference, fallback]);
    state
}

fn surface_with_state(state: AssetPacksUiState) -> UiSurface {
    let mut surface = UiSurface::new();
    surface.set_screen(Some(UiScreenId::AssetPacks {
        parent: GameOptionsParent::Pause,
    }));
    surface.set_scale(GuiScale::from_pixels(960, 540));
    surface.set_render_state(GameUiRenderState {
        asset_packs: state,
        ..GameUiRenderState::default()
    });
    surface
}

#[test]
fn options_hub_opens_asset_packs_from_title_and_pause() {
    for parent in [GameOptionsParent::Title, GameOptionsParent::Pause] {
        let mut surface = UiSurface::new();
        surface.set_screen(Some(UiScreenId::Options { parent }));
        surface.set_scale(GuiScale::from_pixels(960, 540));
        let rect = surface
            .layout()
            .widget(UI_V2_OPTIONS_ASSET_PACKS)
            .expect("Visual Profiles options entry")
            .rect;
        let point = point_in(rect);
        assert!(surface.pointer_down(point, GameUiRenderState::default()));
        assert_eq!(
            surface.pointer_up(point, GameUiRenderState::default()).1,
            Some(GameUiAction::OpenAssetPacks(parent))
        );
    }
}

#[test]
fn pack_rows_emit_copyable_actions_and_locked_rows_do_not_toggle() {
    let mut surface = surface_with_state(asset_pack_state(true));
    let widgets = surface.layout().widgets().to_vec();
    let authored = widgets
        .iter()
        .find(|widget| widget.label.contains("mclone-authored"))
        .unwrap();
    let point = point_in(authored.rect);
    assert!(surface.pointer_down(point, surface.render_state));
    assert_eq!(
        surface.pointer_up(point, surface.render_state).1,
        Some(GameUiAction::ToggleAssetPack(AUTHORED))
    );

    for pack_id in ["minecraft-1.17.1-reference", "mclone-generated-fallback"] {
        let widget = widgets
            .iter()
            .find(|widget| widget.label.contains(pack_id))
            .unwrap();
        let point = point_in(widget.rect);
        assert!(surface.pointer_down(point, surface.render_state));
        assert_eq!(surface.pointer_up(point, surface.render_state).1, None);
    }
}

#[test]
fn apply_cancel_progress_and_keyboard_back_use_shared_contracts() {
    let mut surface = surface_with_state(asset_pack_state(true));
    let apply = surface
        .layout()
        .widget(UI_V2_ASSET_PACK_APPLY)
        .expect("Apply")
        .clone();
    assert!(apply.enabled);
    let point = point_in(apply.rect);
    assert!(surface.pointer_down(point, surface.render_state));
    assert_eq!(
        surface.pointer_up(point, surface.render_state).1,
        Some(GameUiAction::ApplyAssetPacks)
    );
    assert_eq!(
        surface.key_pressed(GuiKey::Escape),
        (true, Some(GameUiAction::CancelAssetPacks))
    );

    let mut preparing = asset_pack_state(true);
    preparing.apply_state = AssetPackUiApplyState::PreparingMeshes;
    let mut preparing_surface = surface_with_state(preparing);
    assert!(
        !preparing_surface
            .layout()
            .widget(UI_V2_ASSET_PACK_APPLY)
            .unwrap()
            .enabled
    );
    assert!(
        !preparing_surface
            .layout()
            .widget(UI_V2_ASSET_PACK_CANCEL)
            .unwrap()
            .enabled
    );
    assert_eq!(preparing_surface.key_pressed(GuiKey::Escape), (true, None));
}

#[test]
fn rendered_screen_contains_profiles_presentation_and_failure_text() {
    let mut state = asset_pack_state(true);
    state.apply_state = AssetPackUiApplyState::Failed;
    state.message = WorldCatalogUiText::new("Pack decode failed");
    let failed_row = state.rows[0].as_mut().expect("staged profile row");
    failed_row.status = AssetPackUiRowStatus::Failed;
    failed_row.detail = state.message;
    let mut surface = surface_with_state(state);
    let draw = surface.render_draw_list(GameUiRenderState {
        asset_packs: state,
        ..GameUiRenderState::default()
    });
    let text = draw
        .commands()
        .iter()
        .filter_map(|command| match command {
            GuiDrawCommand::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("VISUAL PROFILES"));
    assert!(text.contains("mclone-authored"));
    assert!(text.contains("Local only / proprietary"));
    assert!(text.contains("Generated Fallback Only") || text.contains("Mclone Original"));
    assert!(text.contains("Presentation:"));
    assert!(text.contains("Pack decode failed"));
}
