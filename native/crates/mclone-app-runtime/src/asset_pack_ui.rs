use anyhow::{Context, Result, bail};
use mclone_assets::{
    AssetPackAvailability, AssetPackCatalog, AssetPackDescriptor, AssetPackId, AssetPackOrigin,
    AssetPackSelection, AssetProvenanceReport, AssetProvenanceSummary,
};
use mclone_ui::{
    ASSET_PACK_UI_ROW_CAPACITY, AssetPackUiApplyState, AssetPackUiCoverage, AssetPackUiId,
    AssetPackUiOrigin, AssetPackUiRow, AssetPackUiRowStatus, AssetPacksUiState, GameUiAction,
    WorldCatalogUiText,
};

use crate::prepared_assets::{
    AUTHORED_FIRST_PARTY_PACK_ID, GENERATED_FALLBACK_PACK_ID, MINECRAFT_REFERENCE_PACK_ID,
    PreparedAssetCoverage,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientAssetPackEffect {
    ApplySelection(AssetPackSelection),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientAssetPackController {
    catalog: AssetPackCatalog,
    active: AssetPackSelection,
    staged: AssetPackSelection,
    apply_state: AssetPackUiApplyState,
    message: String,
    provenance: AssetProvenanceSummary,
    coverage: Option<PreparedAssetCoverage>,
}

impl Default for ClientAssetPackController {
    fn default() -> Self {
        let authored = AssetPackDescriptor::new(
            AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
            "Mclone Original Assets",
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
        let fallback = AssetPackDescriptor::new(
            AssetPackId::new(GENERATED_FALLBACK_PACK_ID),
            "Generated Missing Assets",
            AssetPackOrigin::Generated,
            30,
        )
        .expect("built-in fallback descriptor")
        .required();
        let catalog = AssetPackCatalog::new([authored, reference, fallback])
            .expect("built-in asset pack catalog");
        let active = AssetPackSelection::new([AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID)]);
        Self {
            catalog,
            active: active.clone(),
            staged: active,
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
        validate_asset_pack_ui_catalog(&catalog)?;
        catalog
            .source_order(&active)
            .context("active asset selection is invalid for the UI catalog")?;
        self.catalog = catalog;
        self.active = active.clone();
        self.staged = active;
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

    pub fn apply_ui_action(
        &mut self,
        action: GameUiAction,
    ) -> Result<Option<ClientAssetPackEffect>> {
        match action {
            GameUiAction::ToggleAssetPack(ui_id) => {
                if self.apply_state.is_preparing() {
                    return Ok(None);
                }
                let descriptor = self
                    .descriptor_for_ui_id(ui_id)
                    .context("asset pack row no longer exists in the current catalog")?;
                if !descriptor.disableable || !descriptor.availability.is_available() {
                    return Ok(None);
                }
                let id = descriptor.id.clone();
                let mut enabled = self.staged.enabled_ids().cloned().collect::<Vec<_>>();
                if self.staged.is_enabled(&id) {
                    enabled.retain(|enabled_id| enabled_id != &id);
                } else {
                    enabled.push(id);
                }
                self.staged = AssetPackSelection::new(enabled);
                self.apply_state = AssetPackUiApplyState::Idle;
                self.message.clear();
                Ok(None)
            }
            GameUiAction::CancelAssetPacks => {
                if !self.apply_state.is_preparing() {
                    self.staged = self.active.clone();
                    self.apply_state = AssetPackUiApplyState::Idle;
                    self.message.clear();
                }
                Ok(None)
            }
            GameUiAction::ApplyAssetPacks => {
                if self.apply_state.is_preparing() || self.staged == self.active {
                    return Ok(None);
                }
                self.catalog
                    .source_order(&self.staged)
                    .context("staged asset selection is invalid")?;
                self.apply_state = AssetPackUiApplyState::PreparingAssets;
                self.message = "Preparing selected packs".to_owned();
                Ok(Some(ClientAssetPackEffect::ApplySelection(
                    self.staged.clone(),
                )))
            }
            _ => Ok(None),
        }
    }

    pub fn begin_preferred_selection(&mut self, selection: AssetPackSelection) -> Result<()> {
        self.catalog
            .source_order(&selection)
            .context("preferred asset selection is invalid")?;
        if selection == self.active {
            self.staged = selection;
            self.apply_state = AssetPackUiApplyState::Idle;
            self.message.clear();
            return Ok(());
        }
        self.staged = selection;
        self.apply_state = AssetPackUiApplyState::PreparingAssets;
        self.message = "Restoring preferred packs".to_owned();
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
        self.active = selection.clone();
        self.staged = selection;
        self.apply_state = AssetPackUiApplyState::Idle;
        self.message.clear();
        self.provenance = provenance.summary();
        self.coverage = coverage;
    }

    pub fn ui_state(&self) -> AssetPacksUiState {
        let mut descriptors = self.catalog.descriptors().collect::<Vec<_>>();
        descriptors.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut rows = Vec::with_capacity(descriptors.len());
        for descriptor in descriptors {
            let staged = !descriptor.disableable || self.staged.is_enabled(&descriptor.id);
            let active = !descriptor.disableable || self.active.is_enabled(&descriptor.id);
            let available = descriptor.availability.is_available();
            let status = if !available {
                AssetPackUiRowStatus::Unavailable
            } else if self.apply_state.is_preparing() && staged {
                AssetPackUiRowStatus::Preparing
            } else if self.apply_state == AssetPackUiApplyState::Failed && staged != active {
                AssetPackUiRowStatus::Failed
            } else if staged == active && active {
                AssetPackUiRowStatus::Active
            } else if staged {
                AssetPackUiRowStatus::Enabled
            } else {
                AssetPackUiRowStatus::Disabled
            };
            let detail = match &descriptor.availability {
                AssetPackAvailability::Available if !descriptor.disableable => "Always active",
                AssetPackAvailability::Available => "",
                AssetPackAvailability::Unavailable { reason } => reason,
            };
            let mut row = AssetPackUiRow::new(
                asset_pack_ui_id(&descriptor.id),
                descriptor.id.as_str(),
                &descriptor.display_name,
                ui_origin(descriptor.origin),
            );
            row.status = status;
            row.enabled = staged;
            row.active = active;
            row.available = available;
            row.disableable = descriptor.disableable;
            row.detail = WorldCatalogUiText::new(detail);
            rows.push(row);
        }
        let coverage = ui_coverage(self.provenance, self.coverage);
        let mut state = AssetPacksUiState {
            effective_label: WorldCatalogUiText::new(effective_selection_label(&self.staged)),
            coverage,
            dirty: self.staged != self.active,
            apply_state: self.apply_state,
            message: WorldCatalogUiText::new(&self.message),
            ..AssetPacksUiState::empty()
        };
        state.set_rows(&rows);
        state
    }

    fn descriptor_for_ui_id(&self, ui_id: AssetPackUiId) -> Option<&AssetPackDescriptor> {
        self.catalog
            .descriptors()
            .find(|descriptor| asset_pack_ui_id(&descriptor.id) == ui_id)
    }
}

pub fn effective_selection_label(selection: &AssetPackSelection) -> &'static str {
    let authored = selection.is_enabled(&AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID));
    let reference = selection.is_enabled(&AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID));
    match (authored, reference) {
        (true, false) => "Mclone Original",
        (false, true) => "Vanilla Reference",
        (true, true) => "Hybrid Authoring",
        (false, false) => "Generated Fallback Only",
    }
}

fn asset_pack_ui_id(id: &AssetPackId) -> AssetPackUiId {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in id.as_str().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    AssetPackUiId(hash)
}

fn validate_asset_pack_ui_catalog(catalog: &AssetPackCatalog) -> Result<()> {
    let mut ids = std::collections::BTreeMap::new();
    for descriptor in catalog.descriptors() {
        if ids
            .insert(asset_pack_ui_id(&descriptor.id), descriptor.id.clone())
            .is_some()
        {
            bail!("asset pack catalog contains colliding compact UI ids");
        }
    }
    if ids.len() > ASSET_PACK_UI_ROW_CAPACITY {
        bail!(
            "asset pack catalog has {} rows; UI capacity is {}",
            ids.len(),
            ASSET_PACK_UI_ROW_CAPACITY
        );
    }
    Ok(())
}

const fn ui_origin(origin: AssetPackOrigin) -> AssetPackUiOrigin {
    match origin {
        AssetPackOrigin::FirstParty => AssetPackUiOrigin::FirstParty,
        AssetPackOrigin::FirstPartyProvisional
        | AssetPackOrigin::Diagnostic
        | AssetPackOrigin::Generated => AssetPackUiOrigin::Generated,
        AssetPackOrigin::MinecraftReference => AssetPackUiOrigin::MinecraftReference,
        AssetPackOrigin::Unknown => AssetPackUiOrigin::Unknown,
    }
}

fn ui_coverage(
    provenance: AssetProvenanceSummary,
    coverage: Option<PreparedAssetCoverage>,
) -> AssetPackUiCoverage {
    let authored = coverage.map_or(provenance.first_party, |facts| {
        facts.first_party_resolutions
    });
    let generated = coverage.map_or(provenance.generated, |facts| facts.generated_resolutions);
    AssetPackUiCoverage {
        authored,
        required: provenance.first_party
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
        let authored = AssetPackDescriptor::new(
            AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
            "Mclone Original Assets",
            AssetPackOrigin::FirstParty,
            10,
        )
        .unwrap();
        let reference = AssetPackDescriptor::new(
            AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID),
            "Minecraft Reference",
            AssetPackOrigin::MinecraftReference,
            20,
        )
        .unwrap();
        let fallback = AssetPackDescriptor::new(
            AssetPackId::new(GENERATED_FALLBACK_PACK_ID),
            "Generated Missing Assets",
            AssetPackOrigin::Generated,
            30,
        )
        .unwrap()
        .required();
        let catalog = AssetPackCatalog::new([authored, reference, fallback]).unwrap();
        let mut controller = ClientAssetPackController::default();
        controller
            .configure(
                catalog,
                AssetPackSelection::default(),
                &AssetProvenanceReport::new(0, AssetPackSelection::default()),
                None,
            )
            .unwrap();
        controller
    }

    fn row_id(controller: &ClientAssetPackController, pack_id: &str) -> AssetPackUiId {
        controller
            .ui_state()
            .rows
            .iter()
            .flatten()
            .find(|row| row.pack_id.as_str() == pack_id)
            .unwrap()
            .ui_id
    }

    #[test]
    fn stages_and_applies_all_four_well_known_optional_selections() {
        let mut controller = available_controller();
        let authored = row_id(&controller, AUTHORED_FIRST_PARTY_PACK_ID);
        let reference = row_id(&controller, MINECRAFT_REFERENCE_PACK_ID);
        let steps = [
            (authored, "Mclone Original"),
            (reference, "Hybrid Authoring"),
            (authored, "Vanilla Reference"),
            (reference, "Generated Fallback Only"),
        ];

        for (ui_id, label) in steps {
            controller
                .apply_ui_action(GameUiAction::ToggleAssetPack(ui_id))
                .unwrap();
            assert_eq!(controller.ui_state().effective_label.as_str(), label);
            let effect = controller
                .apply_ui_action(GameUiAction::ApplyAssetPacks)
                .unwrap()
                .expect("every step changes the active selection");
            let ClientAssetPackEffect::ApplySelection(selection) = effect;
            let report = AssetProvenanceReport::new(1, selection.clone());
            controller.mark_active(selection, &report, None);
        }
    }

    #[test]
    fn fallback_is_locked_and_cancel_restores_active_selection() {
        let mut controller = available_controller();
        let authored = row_id(&controller, AUTHORED_FIRST_PARTY_PACK_ID);
        let fallback = row_id(&controller, GENERATED_FALLBACK_PACK_ID);
        controller
            .apply_ui_action(GameUiAction::ToggleAssetPack(authored))
            .unwrap();
        controller
            .apply_ui_action(GameUiAction::ToggleAssetPack(fallback))
            .unwrap();
        assert!(controller.ui_state().dirty);
        let fallback_row = *controller.ui_state().row(fallback).unwrap();
        assert!(fallback_row.enabled);
        assert!(!fallback_row.disableable);

        controller
            .apply_ui_action(GameUiAction::CancelAssetPacks)
            .unwrap();
        assert!(!controller.ui_state().dirty);
    }
}
