use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{AssetError, AssetPath, AssetResult};

pub const AUTHORED_FIRST_PARTY_PACK_ID: &str = "mclone-authored";
pub const PROVISIONAL_FIRST_PARTY_PACK_ID: &str = "mclone-generated-fallback";
pub const DIAGNOSTIC_MISSING_PACK_ID: &str = "mclone-diagnostic-missing";
pub const MINECRAFT_REFERENCE_PACK_ID: &str = "minecraft-1.17.1-reference";

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetPackId(String);

impl AssetPackId {
    pub fn new(value: impl Into<String>) -> Self {
        Self::try_new(value).expect("invalid asset pack id")
    }

    pub fn try_new(value: impl Into<String>) -> AssetResult<Self> {
        let value = value.into();
        if value.is_empty()
            || !value
                .chars()
                .all(|character| matches!(character, 'a'..='z' | '0'..='9' | '-' | '_' | '.'))
        {
            return Err(AssetError::InvalidAssetPack(format!(
                "invalid asset pack id `{value}`"
            )));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetPackId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum AssetPackOrigin {
    FirstParty,
    FirstPartyProvisional,
    Diagnostic,
    Generated,
    MinecraftReference,
    #[default]
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum AssetPackRole {
    AuthoredOverride,
    ProvisionalBase,
    DiagnosticFallback,
    ReferenceBase,
    GeneratedFallback,
    RenderContent,
    AudioContent,
    Metadata,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextureVisualProfile {
    #[default]
    McloneOriginal,
    MinecraftReference,
    HybridAuthoring,
    FirstPartyCoverage,
    ProvisionalAudit,
}

impl TextureVisualProfile {
    pub const ALL: [Self; 5] = [
        Self::McloneOriginal,
        Self::MinecraftReference,
        Self::HybridAuthoring,
        Self::FirstPartyCoverage,
        Self::ProvisionalAudit,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::McloneOriginal => "mclone-original",
            Self::MinecraftReference => "minecraft-reference",
            Self::HybridAuthoring => "hybrid-authoring",
            Self::FirstPartyCoverage => "first-party-coverage",
            Self::ProvisionalAudit => "provisional-audit",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::McloneOriginal => "Mclone Original",
            Self::MinecraftReference => "Minecraft Reference",
            Self::HybridAuthoring => "Hybrid Authoring",
            Self::FirstPartyCoverage => "First-party Coverage",
            Self::ProvisionalAudit => "Provisional Audit",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|profile| profile.id() == value)
    }

    pub fn selection(self) -> AssetPackSelection {
        let ids: &[&str] = match self {
            Self::McloneOriginal => &[
                AUTHORED_FIRST_PARTY_PACK_ID,
                PROVISIONAL_FIRST_PARTY_PACK_ID,
            ],
            Self::MinecraftReference => {
                &[MINECRAFT_REFERENCE_PACK_ID, PROVISIONAL_FIRST_PARTY_PACK_ID]
            }
            Self::HybridAuthoring => &[
                AUTHORED_FIRST_PARTY_PACK_ID,
                MINECRAFT_REFERENCE_PACK_ID,
                PROVISIONAL_FIRST_PARTY_PACK_ID,
            ],
            Self::FirstPartyCoverage => &[AUTHORED_FIRST_PARTY_PACK_ID, DIAGNOSTIC_MISSING_PACK_ID],
            Self::ProvisionalAudit => &[PROVISIONAL_FIRST_PARTY_PACK_ID],
        };
        AssetPackSelection::new(ids.iter().map(|id| AssetPackId::new(*id)))
    }

    pub fn from_selection(selection: &AssetPackSelection) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.selection() == *selection)
    }

    pub fn normalize_legacy_selection(selection: &AssetPackSelection) -> AssetPackSelection {
        if Self::from_selection(selection).is_some() {
            return selection.clone();
        }
        let authored = selection.is_enabled(&AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID));
        let reference = selection.is_enabled(&AssetPackId::new(MINECRAFT_REFERENCE_PACK_ID));
        let only_legacy_ids = selection.enabled_ids().all(|id| {
            matches!(
                id.as_str(),
                AUTHORED_FIRST_PARTY_PACK_ID | MINECRAFT_REFERENCE_PACK_ID
            )
        });
        if !only_legacy_ids {
            return selection.clone();
        }
        match (authored, reference) {
            (true, false) => Self::McloneOriginal.selection(),
            (false, true) => Self::MinecraftReference.selection(),
            (true, true) => Self::HybridAuthoring.selection(),
            (false, false) => Self::ProvisionalAudit.selection(),
        }
    }

    pub const fn is_diagnostic(self) -> bool {
        matches!(self, Self::FirstPartyCoverage | Self::ProvisionalAudit)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TexturePresentation {
    #[default]
    Textured,
    FlatColors,
}

impl TexturePresentation {
    pub const ALL: [Self; 2] = [Self::Textured, Self::FlatColors];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Textured => "textured",
            Self::FlatColors => "flat-colors",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Textured => "Textured",
            Self::FlatColors => "Flat colors",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|presentation| presentation.id() == value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetPackAvailability {
    Available,
    Unavailable { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetPackDiscovery {
    pub fallback_id: AssetPackId,
    pub fallback_display_name: String,
    pub trusted_origin: Option<AssetPackOrigin>,
    pub trusted_roles: BTreeSet<AssetPackRole>,
}

impl AssetPackDiscovery {
    pub fn untrusted(fallback_id: AssetPackId, fallback_display_name: impl Into<String>) -> Self {
        Self {
            fallback_id,
            fallback_display_name: fallback_display_name.into(),
            trusted_origin: None,
            trusted_roles: BTreeSet::new(),
        }
    }

    pub fn trusted(
        fallback_id: AssetPackId,
        fallback_display_name: impl Into<String>,
        origin: AssetPackOrigin,
        roles: impl IntoIterator<Item = AssetPackRole>,
    ) -> Self {
        Self {
            fallback_id,
            fallback_display_name: fallback_display_name.into(),
            trusted_origin: Some(origin),
            trusted_roles: roles.into_iter().collect(),
        }
    }
}

impl AssetPackAvailability {
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetPackDescriptor {
    pub id: AssetPackId,
    pub display_name: String,
    pub origin: AssetPackOrigin,
    pub roles: BTreeSet<AssetPackRole>,
    pub asset_schema: Option<String>,
    pub content_fingerprint: Option<String>,
    pub availability: AssetPackAvailability,
    pub priority: u16,
    pub disableable: bool,
}

impl AssetPackDescriptor {
    pub fn new(
        id: AssetPackId,
        display_name: impl Into<String>,
        origin: AssetPackOrigin,
        priority: u16,
    ) -> AssetResult<Self> {
        let display_name = display_name.into();
        if display_name.trim().is_empty() {
            return Err(AssetError::InvalidAssetPack(format!(
                "asset pack {id} has an empty display name"
            )));
        }
        Ok(Self {
            id,
            display_name,
            origin,
            roles: BTreeSet::new(),
            asset_schema: None,
            content_fingerprint: None,
            availability: AssetPackAvailability::Available,
            priority,
            disableable: true,
        })
    }

    pub fn with_roles(mut self, roles: impl IntoIterator<Item = AssetPackRole>) -> Self {
        self.roles = roles.into_iter().collect();
        self
    }

    pub fn with_asset_schema(mut self, asset_schema: impl Into<String>) -> Self {
        self.asset_schema = Some(asset_schema.into());
        self
    }

    pub fn with_content_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.content_fingerprint = Some(fingerprint.into());
        self
    }

    pub fn unavailable(mut self, reason: impl Into<String>) -> Self {
        self.availability = AssetPackAvailability::Unavailable {
            reason: reason.into(),
        };
        self
    }

    pub const fn required(mut self) -> Self {
        self.disableable = false;
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssetPackCatalog {
    descriptors: BTreeMap<AssetPackId, AssetPackDescriptor>,
}

impl AssetPackCatalog {
    pub fn new(descriptors: impl IntoIterator<Item = AssetPackDescriptor>) -> AssetResult<Self> {
        let mut catalog = Self::default();
        for descriptor in descriptors {
            let id = descriptor.id.clone();
            if catalog.descriptors.insert(id.clone(), descriptor).is_some() {
                return Err(AssetError::InvalidAssetPack(format!(
                    "duplicate asset pack id `{id}`"
                )));
            }
        }
        Ok(catalog)
    }

    pub fn get(&self, id: &AssetPackId) -> Option<&AssetPackDescriptor> {
        self.descriptors.get(id)
    }

    pub fn descriptors(&self) -> impl Iterator<Item = &AssetPackDescriptor> {
        self.descriptors.values()
    }

    pub fn source_order(
        &self,
        selection: &AssetPackSelection,
    ) -> AssetResult<Vec<&AssetPackDescriptor>> {
        for id in selection.enabled_ids() {
            let Some(descriptor) = self.get(id) else {
                return Err(AssetError::InvalidAssetPack(format!(
                    "selection references undiscovered asset pack `{id}`"
                )));
            };
            if let AssetPackAvailability::Unavailable { reason } = &descriptor.availability {
                return Err(AssetError::InvalidAssetPack(format!(
                    "selected asset pack `{id}` is unavailable: {reason}"
                )));
            }
        }

        let mut descriptors = self
            .descriptors()
            .filter(|descriptor| !descriptor.disableable || selection.is_enabled(&descriptor.id))
            .collect::<Vec<_>>();
        for descriptor in &descriptors {
            if let AssetPackAvailability::Unavailable { reason } = &descriptor.availability {
                return Err(AssetError::InvalidAssetPack(format!(
                    "required asset pack `{}` is unavailable: {reason}",
                    descriptor.id
                )));
            }
        }
        descriptors.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(descriptors)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssetPackSelection {
    enabled: BTreeSet<AssetPackId>,
}

impl AssetPackSelection {
    pub fn new(enabled: impl IntoIterator<Item = AssetPackId>) -> Self {
        Self {
            enabled: enabled.into_iter().collect(),
        }
    }

    pub fn is_enabled(&self, id: &AssetPackId) -> bool {
        self.enabled.contains(id)
    }

    pub fn enabled_ids(&self) -> impl Iterator<Item = &AssetPackId> {
        self.enabled.iter()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetResolutionOrigin {
    pub pack_id: Option<AssetPackId>,
    pub origin: AssetPackOrigin,
}

impl AssetResolutionOrigin {
    pub fn named(pack_id: AssetPackId, origin: AssetPackOrigin) -> Self {
        Self {
            pack_id: Some(pack_id),
            origin,
        }
    }

    pub const fn anonymous() -> Self {
        Self {
            pack_id: None,
            origin: AssetPackOrigin::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetResolutionOutcome {
    Resolved,
    Suppressed,
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetProvenanceEntry {
    pub path: AssetPath,
    pub source: AssetResolutionOrigin,
    pub outcome: AssetResolutionOutcome,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AssetProvenanceSummary {
    pub first_party: usize,
    pub provisional: usize,
    pub diagnostic: usize,
    pub generated: usize,
    pub minecraft_reference: usize,
    pub unknown: usize,
    pub suppressed: usize,
    pub missing: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetProvenanceReport {
    pub epoch: u64,
    pub selection: AssetPackSelection,
    entries: Vec<AssetProvenanceEntry>,
}

impl AssetProvenanceReport {
    pub fn new(epoch: u64, selection: AssetPackSelection) -> Self {
        Self {
            epoch,
            selection,
            entries: Vec::new(),
        }
    }

    pub fn record(&mut self, entry: AssetProvenanceEntry) {
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[AssetProvenanceEntry] {
        &self.entries
    }

    pub fn summary(&self) -> AssetProvenanceSummary {
        let mut summary = AssetProvenanceSummary::default();
        for entry in &self.entries {
            match entry.outcome {
                AssetResolutionOutcome::Suppressed => summary.suppressed += 1,
                AssetResolutionOutcome::Missing => summary.missing += 1,
                AssetResolutionOutcome::Resolved => match entry.source.origin {
                    AssetPackOrigin::FirstParty => summary.first_party += 1,
                    AssetPackOrigin::FirstPartyProvisional => summary.provisional += 1,
                    AssetPackOrigin::Diagnostic => summary.diagnostic += 1,
                    AssetPackOrigin::Generated => summary.generated += 1,
                    AssetPackOrigin::MinecraftReference => summary.minecraft_reference += 1,
                    AssetPackOrigin::Unknown => summary.unknown += 1,
                },
            }
        }
        summary
    }

    pub fn allows_proprietary_free_claim(&self) -> bool {
        let summary = self.summary();
        summary.minecraft_reference == 0 && summary.unknown == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(id: &str, origin: AssetPackOrigin, priority: u16) -> AssetPackDescriptor {
        AssetPackDescriptor::new(AssetPackId::new(id), id, origin, priority).unwrap()
    }

    #[test]
    fn catalog_orders_enabled_packs_and_always_includes_required_fallback() {
        let authored = descriptor("mclone-authored", AssetPackOrigin::FirstParty, 10);
        let reference = descriptor(
            "minecraft-1.17.1-reference",
            AssetPackOrigin::MinecraftReference,
            20,
        );
        let fallback = descriptor("mclone-generated", AssetPackOrigin::Generated, 30).required();
        let catalog = AssetPackCatalog::new([fallback, reference, authored]).unwrap();
        let selection = AssetPackSelection::new([
            AssetPackId::new("minecraft-1.17.1-reference"),
            AssetPackId::new("mclone-authored"),
        ]);

        let order = catalog
            .source_order(&selection)
            .unwrap()
            .into_iter()
            .map(|descriptor| descriptor.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            order,
            [
                "mclone-authored",
                "minecraft-1.17.1-reference",
                "mclone-generated"
            ]
        );
    }

    #[test]
    fn named_texture_profiles_have_exact_source_families() {
        assert_eq!(
            TextureVisualProfile::McloneOriginal.selection(),
            AssetPackSelection::new([
                AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
                AssetPackId::new(PROVISIONAL_FIRST_PARTY_PACK_ID),
            ])
        );
        assert_eq!(
            TextureVisualProfile::FirstPartyCoverage.selection(),
            AssetPackSelection::new([
                AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
                AssetPackId::new(DIAGNOSTIC_MISSING_PACK_ID),
            ])
        );
        for profile in TextureVisualProfile::ALL {
            assert_eq!(
                TextureVisualProfile::parse(profile.id()),
                Some(profile),
                "{} did not round-trip",
                profile.id()
            );
            assert_eq!(
                TextureVisualProfile::from_selection(&profile.selection()),
                Some(profile)
            );
        }
    }

    #[test]
    fn texture_presentation_ids_round_trip() {
        for presentation in TexturePresentation::ALL {
            assert_eq!(
                TexturePresentation::parse(presentation.id()),
                Some(presentation)
            );
        }
    }

    #[test]
    fn legacy_optional_pack_selections_gain_their_explicit_base() {
        assert_eq!(
            TextureVisualProfile::normalize_legacy_selection(&AssetPackSelection::new([
                AssetPackId::new(AUTHORED_FIRST_PARTY_PACK_ID),
            ])),
            TextureVisualProfile::McloneOriginal.selection()
        );
        assert_eq!(
            TextureVisualProfile::normalize_legacy_selection(&AssetPackSelection::default()),
            TextureVisualProfile::ProvisionalAudit.selection()
        );
        let custom = AssetPackSelection::new([AssetPackId::new("community-pack")]);
        assert_eq!(
            TextureVisualProfile::normalize_legacy_selection(&custom),
            custom
        );
    }

    #[test]
    fn catalog_represents_unavailable_pack_but_rejects_selecting_it() {
        let reference = descriptor(
            "minecraft-1.17.1-reference",
            AssetPackOrigin::MinecraftReference,
            20,
        )
        .unavailable("not installed");
        let catalog = AssetPackCatalog::new([reference]).unwrap();
        let selection = AssetPackSelection::new([AssetPackId::new("minecraft-1.17.1-reference")]);

        assert!(catalog.source_order(&selection).is_err());
        assert_eq!(catalog.descriptors().count(), 1);
    }

    #[test]
    fn unknown_resolutions_suppress_proprietary_free_claims() {
        let mut report = AssetProvenanceReport::new(7, AssetPackSelection::default());
        report.record(AssetProvenanceEntry {
            path: AssetPath::new("assets/mclone/known.png"),
            source: AssetResolutionOrigin::named(
                AssetPackId::new("mclone-authored"),
                AssetPackOrigin::FirstParty,
            ),
            outcome: AssetResolutionOutcome::Resolved,
        });
        assert!(report.allows_proprietary_free_claim());

        report.record(AssetProvenanceEntry {
            path: AssetPath::new("assets/custom/unmarked.png"),
            source: AssetResolutionOrigin::anonymous(),
            outcome: AssetResolutionOutcome::Resolved,
        });

        assert!(!report.allows_proprietary_free_claim());
        assert_eq!(report.summary().unknown, 1);
    }
}
