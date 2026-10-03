//! Validated saved exports. Versions remain opaque; selection always names a channel or release.
use crate::{ApiError, ExportId, ReleaseId, SecretToken, Sha256Digest, SoftwareId, VariantId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Delivery mechanism for an export snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportDestination {
    /// Serve the selected snapshot as a protected repository.
    Hosted,
    /// Produce files for an independently managed repository.
    Download,
}
/// Stable selection policy, with no version ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExportSource {
    /// Follow an explicitly selected channel.
    Channel {
        /// Channel name.
        channel: SoftwareSlug,
    },
    /// Retain one exact release, until the operator changes it.
    Release {
        /// Release identity.
        release: ReleaseId,
    },
}
/// Supported installer transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallerFormat {
    /// Flat macOS installer package.
    Pkg,
    /// Application copied from a disk image.
    DmgApp,
}
impl InstallerFormat {
    /// Safe filename extension.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Pkg => "pkg",
            Self::DmgApp => "dmg",
        }
    }
}
/// Raw installed-state declaration; validated as part of `ExportSettings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InstalledState {
    /// Application bundle installed in /Applications.
    Application {
        /// Filename ending in .app, without path components.
        name: String,
        /// Bundle identifier.
        bundle_id: String,
    },
    /// Package receipt matching the release's exact opaque version.
    Receipt {
        /// Package identifier.
        package_id: String,
    },
}
/// Input representation; not a validation proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawExportSettings {
    /// Installer representation.
    pub format: InstallerFormat,
    /// Installation detection policy.
    pub detection: InstalledState,
    /// Optional presentation override.
    #[serde(default)]
    pub display_name: String,
    /// Human description shown in the destination.
    #[serde(default)]
    pub description: String,
    /// Destination category.
    #[serde(default)]
    pub category: String,
}
/// Validated reusable installation and presentation settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawExportSettings", into = "RawExportSettings")]
pub struct ExportSettings(RawExportSettings);
impl ExportSettings {
    /// Read-only validated settings.
    pub const fn data(&self) -> &RawExportSettings {
        &self.0
    }
}
impl From<ExportSettings> for RawExportSettings {
    fn from(value: ExportSettings) -> Self {
        value.0
    }
}
impl TryFrom<RawExportSettings> for ExportSettings {
    type Error = &'static str;
    fn try_from(value: RawExportSettings) -> Result<Self, Self::Error> {
        let identifier = |s: &str| {
            !s.is_empty()
                && s.len() <= 255
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c))
        };
        match &value.detection {
            InstalledState::Application { name, bundle_id } => {
                if name.len() <= 4
                    || name.len() > 200
                    || !name.to_ascii_lowercase().ends_with(".app")
                    || name.contains(['/', '\\', ':'])
                    || name.chars().any(char::is_control)
                    || !identifier(bundle_id)
                {
                    return Err(
                        "Application detection needs a .app filename and bundle identifier.",
                    );
                }
            }
            InstalledState::Receipt { package_id } => {
                if value.format != InstallerFormat::Pkg || !identifier(package_id) {
                    return Err("Receipt detection needs a PKG installer and package identifier.");
                }
            }
        }
        for (text, max) in [
            (&value.display_name, 200),
            (&value.description, 4000),
            (&value.category, 100),
        ] {
            if text.len() > max || text.chars().any(|c| c.is_control() && c != '\n') {
                return Err("Export presentation text is invalid or too long.");
            }
        }
        Ok(Self(value))
    }
}
/// One selected library application and its reusable destination policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportSelection {
    /// Library identity; never a free-form path.
    pub software: SoftwareId,
    /// Channel-following or exact release selection.
    pub source: ExportSource,
    /// Empty selects all compatible Mac variants; otherwise filters hardware architectures.
    #[serde(default)]
    pub architectures: Vec<Architecture>,
    /// Missing settings are saved as a draft and block publication until reviewed.
    pub settings: Option<ExportSettings>,
}
/// Input representation for a complete saved export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawExportDefinition {
    /// Stable human-friendly identity.
    pub slug: SoftwareSlug,
    /// Display name.
    pub name: String,
    /// Destination mode.
    pub destination: ExportDestination,
    /// Munki catalog name, independent of selected Stabbur channels.
    pub catalog: SoftwareSlug,
    /// Explicit application membership.
    pub selections: Vec<ExportSelection>,
}
/// Bounded, unique saved export definition. Deserialization always validates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawExportDefinition", into = "RawExportDefinition")]
pub struct ExportDefinition(RawExportDefinition);
impl ExportDefinition {
    /// Read-only definition.
    pub const fn data(&self) -> &RawExportDefinition {
        &self.0
    }
}
impl From<ExportDefinition> for RawExportDefinition {
    fn from(value: ExportDefinition) -> Self {
        value.0
    }
}
impl TryFrom<RawExportDefinition> for ExportDefinition {
    type Error = &'static str;
    fn try_from(mut value: RawExportDefinition) -> Result<Self, Self::Error> {
        if value.name.trim().is_empty()
            || value.name.len() > 200
            || value.name.chars().any(char::is_control)
        {
            return Err("Choose a nonempty export name of at most 200 characters.");
        }
        if value.selections.len() > 100 {
            return Err("An export supports at most 100 applications.");
        }
        let mut seen = BTreeSet::new();
        for selection in &mut value.selections {
            if !seen.insert(selection.software.to_string()) {
                return Err("Select each application only once.");
            }
            if selection.architectures.len() > 2
                || selection
                    .architectures
                    .iter()
                    .any(|a| !matches!(a, Architecture::Aarch64 | Architecture::X86_64))
            {
                return Err("Choose Apple silicon, Intel, or both architectures.");
            }
            selection.architectures.sort_by_key(|a| format!("{a:?}"));
            selection.architectures.dedup();
        }
        value.selections.sort_by_key(|s| s.software.to_string());
        Ok(Self(value))
    }
}

/// Hardware architecture in a library variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Architecture {
    /// Apple silicon.
    Aarch64,
    /// Intel.
    X86_64,
    /// Both Mac architectures.
    Universal,
}
/// Validated safe destination name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SoftwareSlug(String);
impl SoftwareSlug {
    /// Validated segment.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for SoftwareSlug {
    type Error = &'static str;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.contains("--")
            || s.is_empty()
            || s.len() > 63
            || !s
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !s.as_bytes()[0].is_ascii_alphanumeric()
            || !s.as_bytes()[s.len() - 1].is_ascii_alphanumeric()
        {
            return Err("Invalid export name.");
        }
        Ok(Self(s))
    }
}
impl From<SoftwareSlug> for String {
    fn from(s: SoftwareSlug) -> Self {
        s.0
    }
}
impl std::fmt::Display for SoftwareSlug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
/// Server-owned saved export and concurrency preconditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRecord {
    /// Identity.
    pub id: ExportId,
    /// Validated saved definition.
    pub definition: ExportDefinition,
    /// Definition revision.
    pub revision: u64,
    /// Published generation, zero before first publication.
    pub generation: u64,
    /// Last edit time.
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
/// Immutable installer in a published batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportItem {
    /// Library identity.
    pub software: SoftwareId,
    /// Munki package name.
    pub slug: SoftwareSlug,
    /// Display name.
    pub name: String,
    /// Exact release.
    pub release: ReleaseId,
    /// Opaque version.
    pub version: String,
    /// Variant identity.
    pub variant: VariantId,
    /// Actual installer architecture.
    pub architecture: Architecture,
    /// Export hardware filter.
    pub architectures: Vec<Architecture>,
    /// Minimum macOS version.
    pub minimum_macos: Option<String>,
    /// Maximum macOS version.
    pub maximum_macos: Option<String>,
    /// Expected content identity.
    pub digest: Sha256Digest,
    /// Expected byte length.
    pub size: u64,
    /// Reusable destination settings.
    pub settings: ExportSettings,
}
impl ExportItem {
    /// Safe installer basename, derived only from validated fields.
    pub fn installer_name(&self) -> String {
        format!(
            "{}.{}",
            self.digest,
            self.settings.data().format.extension()
        )
    }
    /// Safe pkginfo basename.
    pub fn pkginfo_name(&self) -> String {
        format!("{}-{}.plist", self.slug, self.variant)
    }
}
/// Immutable publication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSnapshot {
    /// Export identity.
    pub export: ExportId,
    /// Publication generation.
    pub generation: u64,
    /// Definition revision.
    pub definition_revision: u64,
    /// Definition at publication time.
    pub definition: ExportDefinition,
    /// Selected installers.
    pub items: Vec<ExportItem>,
    /// Publication time.
    pub created_at: chrono::DateTime<chrono::Utc>,
}
/// Snapshot response plus current eligibility and authoritative Munki metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSnapshotView {
    /// Immutable content selection.
    pub snapshot: ExportSnapshot,
    /// Releases that must no longer be exported.
    pub unavailable_releases: Vec<ReleaseId>,
    /// Server-rendered pkginfo, one entry per installer.
    pub pkginfo: Vec<serde_json::Value>,
}
/// Checked materialization input. Mutable eligibility still requires a fresh remote check before publishing.
#[derive(Debug, Clone)]
pub struct MaterializableExport(ExportSnapshotView);
impl TryFrom<ExportSnapshotView> for MaterializableExport {
    type Error = ApiError;
    fn try_from(view: ExportSnapshotView) -> Result<Self, Self::Error> {
        if !view.unavailable_releases.is_empty() {
            return Err(ApiError::InvalidValue {
                kind: "export",
                detail: "a selected release is no longer available",
            });
        }
        if view.snapshot.generation == 0
            || view.snapshot.items.len() > 400
            || view.pkginfo.len() != view.snapshot.items.len()
        {
            return Err(ApiError::Decode);
        }
        let mut seen = BTreeSet::new();
        for (item, info) in view.snapshot.items.iter().zip(&view.pkginfo) {
            if !seen.insert(item.variant.to_string())
                || item.size == 0
                || item.size > 4 * 1024 * 1024 * 1024
                || info["name"] != item.slug.as_str()
                || info["version"] != item.version
                || info["installer_item_hash"] != item.digest.as_str()
                || info["installer_item_location"] != item.installer_name()
            {
                return Err(ApiError::Decode);
            }
        }
        Ok(Self(view))
    }
}
impl MaterializableExport {
    /// Read-only validated snapshot and metadata.
    pub const fn view(&self) -> &ExportSnapshotView {
        &self.0
    }
}
/// Application-level preview change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportChange {
    /// Library identity.
    pub software: SoftwareId,
    /// Display name.
    pub name: String,
    /// Add, update, unchanged, remove, or blocked.
    pub action: String,
    /// Previous opaque versions.
    pub before: Vec<String>,
    /// Selected opaque versions.
    pub after: Vec<String>,
    /// Actionable blocker.
    pub detail: Option<String>,
}
/// Read-only exact batch preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPlan {
    /// Export identity.
    pub export: ExportId,
    /// Observed definition revision.
    pub definition_revision: u64,
    /// Observed published generation.
    pub published_generation: u64,
    /// Exact comparison token for apply.
    pub fingerprint: Sha256Digest,
    /// Whether every selection is publishable.
    pub ready: bool,
    /// Application changes.
    pub changes: Vec<ExportChange>,
    /// Resolved installers.
    pub items: Vec<ExportItem>,
}
/// One-time export-only credential; Debug always redacts the value.
#[derive(Debug, Clone, Deserialize)]
pub struct ExportReader {
    /// Save only to a protected device configuration or credential file.
    pub token: SecretToken,
}
/// Repository protocol namespace, never an arbitrary API path.
#[derive(Debug, Clone, Copy)]
pub enum RepositoryKind {
    /// Munki catalogs.
    Catalogs,
    /// Munki manifests.
    Manifests,
    /// Package metadata.
    Pkgsinfo,
    /// Installer bytes.
    Pkgs,
}
impl RepositoryKind {
    pub(crate) const fn segment(self) -> &'static str {
        match self {
            Self::Catalogs => "catalogs",
            Self::Manifests => "manifests",
            Self::Pkgsinfo => "pkgsinfo",
            Self::Pkgs => "pkgs",
        }
    }
}
