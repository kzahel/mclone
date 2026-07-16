use super::*;
use crate::{
    EMPTY_BLOCK_PALETTE_ENTRIES, EMPTY_HOTBAR_ICONS, FlatHotbarOverlay, GameCollisionMode,
    GameSimulationCadence, GameTouchSettings, GameTravelAssistMode, GameTurnMode, GameXrTurnMode,
    GuiDrawCommand, GuiTextureUv, LoadingProgressCell, LoadingProgressCellStatus,
    StorageProfileBackend, StorageProfileUiState, render_flat_hud, render_loading_progress_overlay,
    render_loading_progress_panel_at,
};
use mclone_input::{InputPromptKind, ResolvedFlatInput, TouchControlsMode};

fn point_in(rect: Rect) -> Point {
    Point {
        x: rect.center_x(),
        y: rect.y + rect.height * 0.5,
    }
}

fn keyboard_mouse_input() -> ResolvedFlatInput {
    ResolvedFlatInput {
        preferred_prompt: Some(InputPromptKind::KeyboardMouse),
        touch_controls_visible: false,
        accepts_keyboard_mouse: true,
        accepts_touch: false,
        accepts_gamepad: false,
        accepts_xr_controller: false,
    }
}

fn loading_progress_overlay(
    target_ready_chunks: usize,
    center_status: LoadingProgressCellStatus,
) -> LoadingProgressOverlay {
    LoadingProgressOverlay::new(
        1,
        target_ready_chunks,
        9,
        target_ready_chunks >= 1,
        [
            LoadingProgressCell::new(-1, -1, LoadingProgressCellStatus::Terrain),
            LoadingProgressCell::new(0, 0, center_status).playable(true),
            LoadingProgressCell::new(1, 1, LoadingProgressCellStatus::TargetReady),
        ],
    )
}

fn touch_input() -> ResolvedFlatInput {
    ResolvedFlatInput {
        preferred_prompt: Some(InputPromptKind::Touch),
        touch_controls_visible: true,
        accepts_keyboard_mouse: true,
        accepts_touch: true,
        accepts_gamepad: false,
        accepts_xr_controller: false,
    }
}

fn gamepad_input() -> ResolvedFlatInput {
    ResolvedFlatInput {
        preferred_prompt: Some(InputPromptKind::Gamepad),
        touch_controls_visible: false,
        accepts_keyboard_mouse: true,
        accepts_touch: true,
        accepts_gamepad: true,
        accepts_xr_controller: false,
    }
}

fn block_palette_state(selected_hotbar_slot: u8) -> GameUiRenderState {
    let brick_icon = GuiTextureUv::new(0.1, 0.2, 0.3, 0.4);
    let log_icon = GuiTextureUv::new(0.4, 0.3, 0.2, 0.1);
    let mut entries = EMPTY_BLOCK_PALETTE_ENTRIES;
    entries[0] = Some(BlockPaletteEntry::new(91, Some(brick_icon), "Bricks"));
    entries[1] = Some(BlockPaletteEntry::new(41, Some(log_icon), "Oak Log"));
    GameUiRenderState {
        block_palette: BlockPaletteOverlay::visible(selected_hotbar_slot, entries),
        ..GameUiRenderState::default()
    }
}

fn world_catalog_state() -> WorldCatalogUiState {
    let first = WorldCatalogUiEntry::new(WorldCatalogUiWorldId(11), "Alpha Base", 123);
    let second = WorldCatalogUiEntry::new(WorldCatalogUiWorldId(12), "Beta Mine", -456);
    WorldCatalogUiState::persistent_local(&[first, second])
}

fn world_catalog_render_state() -> GameUiRenderState {
    GameUiRenderState {
        world_catalog: world_catalog_state(),
        ..GameUiRenderState::default()
    }
}

mod asset_packs;
mod catalog_palette;
mod core_layout;
mod hud;
mod loading;
mod menus_options;
