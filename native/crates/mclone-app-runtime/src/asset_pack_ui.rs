use anyhow::{Context, Result, bail};
use mclone_assets::{
    AUTHORED_FIRST_PARTY_PACK_ID, AssetPackAvailability, AssetPackCatalog, AssetPackDescriptor,
    AssetPackId, AssetPackOrigin, AssetPackSelection, AssetProvenanceReport,
    AssetProvenanceSummary, DIAGNOSTIC_MISSING_PACK_ID, MINECRAFT_REFERENCE_PACK_ID,
    PROVISIONAL_FIRST_PARTY_PACK_ID, TexturePresentation, TextureVisualProfile,
};
use mclone_ui::{
    ASSET_PACK_UI_ROW_CAPACITY, AssetPackUiApplyState, AssetPackUiCoverage, AssetPackUiId,
    AssetPackUiOrigin, AssetPackUiRow, AssetPackUiRowStatus, AssetPacksUiState, GameUiAction,
    WorldCatalogUiText,
};

use crate::prepared_assets::PreparedAssetCoverage;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientAssetPackEffect {
    ApplySelection {
        selection: AssetPackSelection,
        presentation: TexturePresentation,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientAssetPackController {
    catalog: AssetPackCatalog,
    active: AssetPackSelection,
    staged: AssetPackSelection,
    active_presentation: TexturePresentation,
    staged_presentation: TexturePresentation,
    apply_state: AssetPackUiApplyState,
    message: String,
    provenance: AssetProvenanceSummary,
    coverage: Option<PreparedAssetCoverage>,
}

impl Default for ClientAssetPackController {
    fn default() -> Self {
        let authored = AssetPackDescriptor::new(
            AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
            "Mclone Curated Textures",
            AssetPackOrigin::FirstParty,
            10,
        )
        .expect("built-in authored descriptor")
        .unavailable("Not installed by this platform");
        let reference = AssetPackDescriptor::new(
            AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID),
            "Minecraft 1.17.1 Reference",
            AssetPackOrigin::MinecraftReference,
            20,
        )
        .expect("built-in reference descriptor");
        let provisional = AssetPackDescriptor::new(
            AssetPackId::new(PROVISIONAL_FIRST_PARTY_PACK_ID),
            "Mclone Provisional Textures",
            AssetPackOrigin::FirstPartyProvisional,
            30,
        )
        .expect("built-in provisional descriptor")
        .unavailable("Not installed by this platform");
        let diagnostic = AssetPackDescriptor::new(
            AssetPackId::new(DIAGNOSTIC_MISSING_PACK_ID),
            "Numbered Missing Diagnostics",
            AssetPackOrigin::Diagnostic,
            40,
        )
        .expect("built-in diagnostic descriptor")
        .unavailable("Not installed by this platform");
        let catalog = AssetPackCatalog::new([authored, reference, provisional, diagnostic])
            .expect("built-in asset pack catalog");
        let active = TextureVisualProfile::MinecraftReference.selection();
        Self {
            catalog,
            active: active.clone(),
            staged: active,
            active_presentation: TexturePresentation::Textured,
            staged_presentation: TexturePresentation::Textured,
            apply_state: AssetPackUiApplyState::Idle,
            message: String::new(),
            provenance: AssetProvenanceSummary {
                minecraft_reference: 1,
                ..AssetProvenanceSummary::default()
            },
            coverage: None,
        }
    }
}

impl ClientAssetPackController {
    pub fn configure(
        &mut self,
        catalog: AssetPackCatalog,
        active: AssetPackSelection,
        provenance: &AssetProvenanceReport,
        coverage: Option<PreparedAssetCoverage>,
    ) -> Result<()> {
        self.configure_with_presentation(
            catalog,
            active,
            TexturePresentation::Textured,
            provenance,
            coverage,
        )
    }

    pub fn configure_with_presentation(
        &mut self,
        catalog: AssetPackCatalog,
        active: AssetPackSelection,
        presentation: TexturePresentation,
        provenance: &AssetProvenanceReport,
        coverage: Option<PreparedAssetCoverage>,
    ) -> Result<()> {
        validate_asset_pack_ui_catalog(&catalog)?;
        let active = TextureVisualProfile::normalize_legacy_selection(&active);
        catalog
            .source_order(&active)
            .context("active visual profile is invalid for the UI catalog")?;
        TextureVisualProfile::from_selection(&active)
            .context("active asset selection is not a named visual profile")?;
        self.catalog = catalog;
        self.active = active.clone();
        self.staged = active;
        self.active_presentation = presentation;
        self.staged_presentation = presentation;
        self.apply_state = AssetPackUiApplyState::Idle;
        self.message.clear();
        self.provenance = provenance.summary();
        self.coverage = coverage;
        Ok(())
    }

    pub fn catalog(&self) -> &AssetPackCatalog {
        &self.catalog
    }

    pub fn active_selection(&self) -> &AssetPackSelection {
        &self.active
    }

    pub fn staged_selection(&self) -> &AssetPackSelection {
        &self.staged
    }

    pub const fn active_presentation(&self) -> TexturePresentation {
        self.active_presentation
    }

    pub const fn staged_presentation(&self) -> TexturePresentation {
        self.staged_presentation
    }

    pub fn apply_ui_action(
        &mut self,
        action: GameUiAction,
    ) -> Result<Option<ClientAssetPackEffect>> {
        match action {
            GameUiAction::ToggleAssetPack(ui_id) => {
                if self.apply_state.is_preparing() {
                    return Ok(None);
                }
                let profile =
                    profile_for_ui_id(ui_id).context("visual profile row no longer exists")?;
                self.catalog
                    .source_order(&profile.selection())
                    .context("selected visual profile is unavailable")?;
                self.staged = profile.selection();
                if profile == TextureVisualProfile::FirstPartyCoverage {
                    self.staged_presentation = TexturePresentation::Textured;
                }
                self.apply_state = AssetPackUiApplyState::Idle;
                self.message.clear();
                Ok(None)
            }
            GameUiAction::CycleTexturePresentation => {
                if self.apply_state.is_preparing()
                    || TextureVisualProfile::from_selection(&self.staged)
                        == Some(TextureVisualProfile::FirstPartyCoverage)
                {
                    return Ok(None);
                }
                self.staged_presentation = match self.staged_presentation {
                    TexturePresentation::Textured => TexturePresentation::FlatColors,
                    TexturePresentation::FlatColors => TexturePresentation::Textured,
                };
                self.apply_state = AssetPackUiApplyState::Idle;
                self.message.clear();
                Ok(None)
            }
            GameUiAction::CancelAssetPacks => {
                if !self.apply_state.is_preparing() {
                    self.staged = self.active.clone();
                    self.staged_presentation = self.active_presentation;
                    self.apply_state = AssetPackUiApplyState::Idle;
                    self.message.clear();
                }
                Ok(None)
            }
            GameUiAction::ApplyAssetPacks => {
                if self.apply_state.is_preparing() || !self.is_dirty() {
                    return Ok(None);
                }
                self.catalog
                    .source_order(&self.staged)
                    .context("staged visual profile is invalid")?;
                self.apply_state = AssetPackUiApplyState::PreparingAssets;
                self.message = "Preparing visual profile".to_owned();
                Ok(Some(ClientAssetPackEffect::ApplySelection {
                    selection: self.staged.clone(),
                    presentation: self.staged_presentation,
                }))
            }
            _ => Ok(None),
        }
    }

    pub fn begin_preferred_selection(&mut self, selection: AssetPackSelection) -> Result<()> {
        self.begin_preferred_profile(selection, self.active_presentation)
    }

    pub fn begin_preferred_profile(
        &mut self,
        selection: AssetPackSelection,
        presentation: TexturePresentation,
    ) -> Result<()> {
        let selection = TextureVisualProfile::normalize_legacy_selection(&selection);
        self.catalog
            .source_order(&selection)
            .context("preferred visual profile is invalid")?;
        TextureVisualProfile::from_selection(&selection)
            .context("preferred asset selection is not a named visual profile")?;
        if selection == self.active && presentation == self.active_presentation {
            self.staged = selection;
            self.staged_presentation = presentation;
            self.apply_state = AssetPackUiApplyState::Idle;
            self.message.clear();
            return Ok(());
        }
        self.staged = selection;
        self.staged_presentation = presentation;
        self.apply_state = AssetPackUiApplyState::PreparingAssets;
        self.message = "Restoring visual profile".to_owned();
        Ok(())
    }

    pub fn mark_preparing_meshes(&mut self) {
        self.apply_state = AssetPackUiApplyState::PreparingMeshes;
        self.message = "Preparing visible meshes".to_owned();
    }

    pub fn mark_failed(&mut self, message: impl Into<String>) {
        self.apply_state = AssetPackUiApplyState::Failed;
        self.message = message.into();
    }

    pub fn mark_active(
        &mut self,
        selection: AssetPackSelection,
        provenance: &AssetProvenanceReport,
        coverage: Option<PreparedAssetCoverage>,
    ) {
        self.mark_active_with_presentation(
            selection,
            TexturePresentation::Textured,
            provenance,
            coverage,
        );
    }

    pub fn mark_active_with_presentation(
        &mut self,
        selection: AssetPackSelection,
        presentation: TexturePresentation,
        provenance: &AssetProvenanceReport,
        coverage: Option<PreparedAssetCoverage>,
    ) {
        self.active = selection.clone();
        self.staged = selection;
        self.active_presentation = presentation;
        self.staged_presentation = presentation;
        self.apply_state = AssetPackUiApplyState::Idle;
        self.message.clear();
        self.provenance = provenance.summary();
        self.coverage = coverage;
    }

    pub fn ui_state(&self) -> AssetPacksUiState {
        let staged_profile = TextureVisualProfile::from_selection(&self.staged);
        let active_profile = TextureVisualProfile::from_selection(&self.active);
        let mut rows = Vec::with_capacity(TextureVisualProfile::ALL.len());
        for profile in TextureVisualProfile::ALL {
            let selection = profile.selection();
            let availability = profile_availability(&self.catalog, profile);
            let available = availability.is_none();
            let staged = staged_profile == Some(profile);
            let active = active_profile == Some(profile);
            let status = if !available {
                AssetPackUiRowStatus::Unavailable
            } else if self.apply_state.is_preparing() && staged {
                AssetPackUiRowStatus::Preparing
            } else if self.apply_state == AssetPackUiApplyState::Failed && staged != active {
                AssetPackUiRowStatus::Failed
            } else if staged && active && !self.is_dirty() {
                AssetPackUiRowStatus::Active
            } else if staged {
                AssetPackUiRowStatus::Enabled
            } else {
                AssetPackUiRowStatus::Disabled
            };
            let mut row = AssetPackUiRow::new(
                profile_ui_id(profile),
                profile.id(),
                profile.label(),
                profile_origin(profile),
            );
            row.status = status;
            row.enabled = staged;
            row.active = active;
            row.available = available;
            row.disableable = true;
            row.detail = WorldCatalogUiText::new(if status == AssetPackUiRowStatus::Failed {
                self.message.as_str()
            } else {
                availability
                    .as_deref()
                    .unwrap_or_else(|| profile_detail(profile))
            });
            debug_assert_eq!(selection, profile.selection());
            rows.push(row);
        }
        let coverage = ui_coverage(self.provenance, self.coverage);
        let mut state = AssetPacksUiState {
            effective_label: WorldCatalogUiText::new(effective_selection_label(&self.staged)),
            presentation_label: WorldCatalogUiText::new(self.staged_presentation.label()),
            coverage,
            dirty: self.is_dirty(),
            apply_state: self.apply_state,
            message: WorldCatalogUiText::new(&self.message),
            ..AssetPacksUiState::empty()
        };
        state.set_rows(&rows);
        state
    }

    fn is_dirty(&self) -> bool {
        self.staged != self.active || self.staged_presentation != self.active_presentation
    }
}

pub fn effective_selection_label(selection: &AssetPackSelection) -> &'static str {
    TextureVisualProfile::from_selection(selection)
        .map(TextureVisualProfile::label)
        .unwrap_or("Custom asset selection")
}

fn profile_availability(
    catalog: &AssetPackCatalog,
    profile: TextureVisualProfile,
) -> Option<String> {
    for id in profile.selection().enabled_ids() {
        match catalog.get(id) {
            None => return Some(format!("{} is not discovered", id.as_str())),
            Some(descriptor) => match &descriptor.availability {
                AssetPackAvailability::Available => {}
                AssetPackAvailability::Unavailable { reason } => return Some(reason.clone()),
            },
        }
    }
    None
}

const fn profile_detail(profile: TextureVisualProfile) -> &'static str {
    match profile {
        TextureVisualProfile::McloneOriginal => "Curated → provisional",
        TextureVisualProfile::MinecraftReference => "Minecraft → provisional gaps",
        TextureVisualProfile::HybridAuthoring => "Curated → Minecraft → provisional",
        TextureVisualProfile::FirstPartyCoverage => "Curated → numbered missing",
        TextureVisualProfile::ProvisionalAudit => "Provisional only",
    }
}

const fn profile_origin(profile: TextureVisualProfile) -> AssetPackUiOrigin {
    match profile {
        TextureVisualProfile::McloneOriginal => AssetPackUiOrigin::FirstParty,
        TextureVisualProfile::MinecraftReference | TextureVisualProfile::HybridAuthoring => {
            AssetPackUiOrigin::MinecraftReference
        }
        TextureVisualProfile::FirstPartyCoverage | TextureVisualProfile::ProvisionalAudit => {
            AssetPackUiOrigin::Generated
        }
    }
}

fn profile_ui_id(profile: TextureVisualProfile) -> AssetPackUiId {
    compact_ui_id(profile.id())
}

fn profile_for_ui_id(ui_id: AssetPackUiId) -> Option<TextureVisualProfile> {
    TextureVisualProfile::ALL
        .into_iter()
        .find(|profile| profile_ui_id(*profile) == ui_id)
}

fn compact_ui_id(value: &str) -> AssetPackUiId {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    AssetPackUiId(hash)
}

fn validate_asset_pack_ui_catalog(catalog: &AssetPackCatalog) -> Result<()> {
    let mut ids = std::collections::BTreeSet::new();
    for profile in TextureVisualProfile::ALL {
        if !ids.insert(profile_ui_id(profile)) {
            bail!("visual profile catalog contains colliding compact UI ids");
        }
    }
    if ids.len() > ASSET_PACK_UI_ROW_CAPACITY {
        bail!(
            "visual profile catalog has {} rows; UI capacity is {}",
            ids.len(),
            ASSET_PACK_UI_ROW_CAPACITY
        );
    }
    if catalog.descriptors().count() == 0 {
        bail!("asset pack catalog is empty");
    }
    Ok(())
}

fn ui_coverage(
    provenance: AssetProvenanceSummary,
    coverage: Option<PreparedAssetCoverage>,
) -> AssetPackUiCoverage {
    let authored = coverage.map_or(provenance.first_party, |facts| {
        facts.first_party_resolutions
    });
    let generated = coverage.map_or(
        provenance.generated + provenance.provisional + provenance.diagnostic,
        |facts| facts.generated_resolutions,
    );
    AssetPackUiCoverage {
        authored,
        required: provenance.first_party
            + provenance.provisional
            + provenance.diagnostic
            + provenance.generated
            + provenance.minecraft_reference
            + provenance.unknown,
        generated,
        minecraft: provenance.minecraft_reference,
        unknown: provenance.unknown,
        suppressed: provenance.suppressed,
        missing: provenance.missing,
        proprietary_free: provenance.minecraft_reference == 0 && provenance.unknown == 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available_controller() -> ClientAssetPackController {
        let catalog = AssetPackCatalog::new([
            AssetPackDescriptor::new(
                AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
                "Curated",
                AssetPackOrigin::FirstParty,
                10,
            )
            .unwrap(),
            AssetPackDescriptor::new(
                AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID),
                "Minecraft",
                AssetPackOrigin::MinecraftReference,
                20,
            )
            .unwrap(),
            AssetPackDescriptor::new(
                AssetPackId::new(PROVISIONAL_FIRST_PARTY_PACK_ID),
                "Provisional",
                AssetPackOrigin::FirstPartyProvisional,
                30,
            )
            .unwrap(),
            AssetPackDescriptor::new(
                AssetPackId::new(DIAGNOSTIC_MISSING_PACK_ID),
                "Diagnostic",
                AssetPackOrigin::Diagnostic,
                40,
            )
            .unwrap(),
        ])
        .unwrap();
        let active = TextureVisualProfile::McloneOriginal.selection();
        let mut controller = ClientAssetPackController::default();
        controller
            .configure(
                catalog,
                active.clone(),
                &AssetProvenanceReport::new(0, active),
                None,
            )
            .unwrap();
        controller
    }

    fn row_id(
        controller: &ClientAssetPackController,
        profile: TextureVisualProfile,
    ) -> AssetPackUiId {
        controller
            .ui_state()
            .rows
            .iter()
            .flatten()
            .find(|row| row.pack_id.as_str() == profile.id())
            .unwrap()
            .ui_id
    }

    #[test]
    fn selects_and_applies_each_named_visual_profile() {
        let mut controller = available_controller();
        for profile in TextureVisualProfile::ALL {
            controller
                .apply_ui_action(GameUiAction::ToggleAssetPack(row_id(&controller, profile)))
                .unwrap();
            assert_eq!(
                controller.ui_state().effective_label.as_str(),
                profile.label()
            );
            if controller.ui_state().dirty {
                let effect = controller
                    .apply_ui_action(GameUiAction::ApplyAssetPacks)
                    .unwrap()
                    .expect("changed profile emits apply");
                let ClientAssetPackEffect::ApplySelection {
                    selection,
                    presentation,
                } = effect;
                let report = AssetProvenanceReport::new(1, selection.clone());
                controller.mark_active_with_presentation(selection, presentation, &report, None);
            }
        }
    }

    #[test]
    fn flat_colors_are_orthogonal_but_coverage_stays_legible() {
        let mut controller = available_controller();
        controller
            .apply_ui_action(GameUiAction::CycleTexturePresentation)
            .unwrap();
        assert_eq!(
            controller.staged_presentation(),
            TexturePresentation::FlatColors
        );
        controller
            .apply_ui_action(GameUiAction::ToggleAssetPack(row_id(
                &controller,
                TextureVisualProfile::FirstPartyCoverage,
            )))
            .unwrap();
        assert_eq!(
            controller.staged_presentation(),
            TexturePresentation::Textured
        );
        controller
            .apply_ui_action(GameUiAction::CycleTexturePresentation)
            .unwrap();
        assert_eq!(
            controller.staged_presentation(),
            TexturePresentation::Textured
        );
    }

    #[test]
    fn unavailable_reference_disables_reference_profiles() {
        let mut controller = available_controller();
        let mut descriptors = controller
            .catalog
            .descriptors()
            .cloned()
            .collect::<Vec<_>>();
        let reference = descriptors
            .iter_mut()
            .find(|descriptor| descriptor.id.as_str() == MINECRAFT_REFERENCE_PACK_ID)
            .unwrap();
        reference.availability = AssetPackAvailability::Unavailable {
            reason: "Local Minecraft assets not installed".to_owned(),
        };
        let catalog = AssetPackCatalog::new(descriptors).unwrap();
        let active = TextureVisualProfile::McloneOriginal.selection();
        controller
            .configure(
                catalog,
                active.clone(),
                &AssetProvenanceReport::new(0, active),
                None,
            )
            .unwrap();
        let state = controller.ui_state();
        assert!(
            !state
                .row(row_id(
                    &controller,
                    TextureVisualProfile::MinecraftReference
                ))
                .unwrap()
                .available
        );
        assert!(
            state
                .row(row_id(&controller, TextureVisualProfile::McloneOriginal))
                .unwrap()
                .available
        );
    }
}
