use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    ApiError, InstallationMetadata, NewAutoPkgRevision, NewRecipeRevision, Recipe, RecipeRevision,
    Software, client::validate_recipe_revision,
};

/// Manifest schema understood by this client release.
pub const CATALOG_SCHEMA_VERSION: u32 = 2;

/// Versioned, reviewed catalog desired state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogManifest {
    /// Schema 2 desired execution policy. Unlisted targets remain unmanaged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<CatalogTarget>,
    /// Manifest schema version. Version 1 manages software metadata and immutable recipe revisions.
    pub schema_version: u32,
    /// Software identities managed by the manifest.
    #[serde(default)]
    pub software: Vec<CatalogSoftware>,
    /// Recipe identities and their desired latest immutable revisions.
    #[serde(default)]
    pub recipes: Vec<CatalogRecipe>,
}

impl CatalogManifest {
    /// Validates and returns a deterministically ordered, normalized manifest.
    pub fn canonicalized(&self) -> Result<Self, ApiError> {
        if !matches!(self.schema_version, 1 | 2)
            || (self.schema_version == 1 && !self.targets.is_empty())
        {
            return Err(invalid(
                "catalog schema version",
                "schema 1 supports software/recipes; schema 2 additionally supports targets",
            ));
        }
        if self.software.len() > 1_000 || self.recipes.len() > 1_000 {
            return Err(invalid(
                "catalog manifest",
                "software and recipes are each limited to 1000 entries",
            ));
        }

        let mut software = self.software.clone();
        for entry in &software {
            validate_software(entry)?;
        }
        software.sort_by(|left, right| left.slug.cmp(&right.slug));
        if software
            .windows(2)
            .any(|values| values[0].slug == values[1].slug)
        {
            return Err(invalid("catalog software", "software slugs must be unique"));
        }

        let mut recipes = self.recipes.clone();
        for entry in &mut recipes {
            validate_recipe_name(&entry.name)?;
            entry.revision = canonical_revision(&entry.revision)?;
        }
        recipes.sort_by(|left, right| left.name.cmp(&right.name));
        if recipes
            .windows(2)
            .any(|values| values[0].name == values[1].name)
        {
            return Err(invalid("catalog recipes", "recipe names must be unique"));
        }

        let mut targets = self.targets.clone();
        if targets.len() > 1000 {
            return Err(invalid(
                "catalog targets",
                "at most 1000 targets are allowed",
            ));
        }
        for target in &targets {
            validate_recipe_name(&target.name)?;
            if !software.iter().any(|value| value.slug == target.software)
                || !recipes.iter().any(|value| value.name == target.recipe)
            {
                return Err(invalid(
                    "catalog target reference",
                    "targets must reference software and recipes in this manifest",
                ));
            }
            if matches!(target.schedule, crate::BuildTargetSchedule::Interval { every_seconds } if !(60..=31_536_000).contains(&every_seconds))
            {
                return Err(invalid(
                    "target interval",
                    "expected 60 through 31536000 seconds",
                ));
            }
            crate::client::validate_build_parameters(&target.parameters)?;
        }
        targets.sort_by(|left, right| left.name.cmp(&right.name));
        if targets.windows(2).any(|pair| pair[0].name == pair[1].name) {
            return Err(invalid("catalog targets", "target names must be unique"));
        }
        Ok(Self {
            schema_version: self.schema_version,
            software,
            recipes,
            targets,
        })
    }
}

/// Desired software metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogSoftware {
    /// Unique lowercase URL-safe software slug.
    pub slug: String,
    /// Human-readable display name.
    pub name: String,
    /// Managed installation metadata. Omission leaves existing metadata unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation: Option<InstallationMetadata>,
}

/// Desired recipe metadata and latest immutable revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogRecipe {
    /// Unique operator-facing recipe name.
    pub name: String,
    /// Desired latest builder-neutral revision.
    pub revision: NewRecipeRevision,
}

/// One deterministic catalog reconciliation operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum CatalogAction {
    /// Create desired execution policy after resolving the reviewed immutable revision.
    CreateTarget {
        /// Complete desired target policy.
        target: CatalogTarget,
        /// Exact existing revision or revision created earlier in this plan.
        revision: PlannedRecipeRevision,
    },
    /// Update a managed target using its observed concurrency revision.
    UpdateTarget {
        /// Complete desired target policy.
        target: CatalogTarget,
        /// Exact existing revision or revision created earlier in this plan.
        revision: PlannedRecipeRevision,
        /// Observed target revision.
        expected_revision: u64,
    },
    /// Create missing software.
    CreateSoftware {
        /// Desired software resource.
        software: CatalogSoftware,
    },
    /// Replace a software display name.
    UpdateSoftwareName {
        /// Stable software slug.
        slug: String,
        /// Desired display name.
        name: String,
        /// Current optimistic-concurrency revision.
        expected_revision: u64,
    },
    /// Replace managed installation metadata.
    UpdateSoftwareInstallation {
        /// Stable software slug.
        slug: String,
        /// Desired installation metadata.
        installation: InstallationMetadata,
        /// Current optimistic-concurrency revision.
        expected_revision: u64,
    },
    /// Create missing recipe metadata.
    CreateRecipe {
        /// Stable recipe name.
        name: String,
    },
    /// Append a changed or initially missing immutable recipe revision.
    CreateRecipeRevision {
        /// Stable recipe name.
        recipe: String,
        /// Normalized desired revision.
        revision: NewRecipeRevision,
        /// Sequence expected for the appended revision.
        expected_sequence: u64,
    },
}

/// Deterministic difference between a manifest and the current server catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogPlan {
    /// Planned manifest schema version.
    pub schema_version: u32,
    /// Ordered operations. An empty list means the catalog has converged.
    pub actions: Vec<CatalogAction>,
}

impl CatalogPlan {
    /// Whether the manifest already matches all resources it manages.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

/// Result of applying a catalog plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogSyncReport {
    /// Applied manifest schema version.
    pub schema_version: u32,
    /// Operations successfully applied in order.
    pub applied: Vec<CatalogAction>,
}

pub(crate) fn build_plan(
    manifest: &ValidatedCatalogManifest,
    software: &[Software],
    recipes: &[(Recipe, Vec<RecipeRevision>)],
    targets: &[crate::BuildTarget],
) -> Result<CatalogPlan, ApiError> {
    let manifest = manifest.as_manifest();
    let current_software = software
        .iter()
        .map(|entry| (entry.slug.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let current_recipes = recipes
        .iter()
        .map(|entry| (entry.0.name.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut actions = Vec::new();

    for desired in &manifest.software {
        let Some(current) = current_software.get(desired.slug.as_str()) else {
            actions.push(CatalogAction::CreateSoftware {
                software: desired.clone(),
            });
            continue;
        };
        let mut expected_revision = current.revision;
        if current.name != desired.name {
            actions.push(CatalogAction::UpdateSoftwareName {
                slug: desired.slug.clone(),
                name: desired.name.clone(),
                expected_revision,
            });
            expected_revision += 1;
        }
        if let Some(installation) = &desired.installation
            && current.installation.as_ref() != Some(installation)
        {
            actions.push(CatalogAction::UpdateSoftwareInstallation {
                slug: desired.slug.clone(),
                installation: installation.clone(),
                expected_revision,
            });
        }
    }

    for desired in &manifest.recipes {
        let Some((_, revisions)) = current_recipes.get(desired.name.as_str()) else {
            actions.push(CatalogAction::CreateRecipe {
                name: desired.name.clone(),
            });
            actions.push(CatalogAction::CreateRecipeRevision {
                recipe: desired.name.clone(),
                revision: desired.revision.clone(),
                expected_sequence: 1,
            });
            continue;
        };
        let latest = revisions.iter().max_by_key(|revision| revision.sequence);
        if latest.is_none_or(|revision| !revision_matches(revision, &desired.revision)) {
            actions.push(CatalogAction::CreateRecipeRevision {
                recipe: desired.name.clone(),
                revision: desired.revision.clone(),
                expected_sequence: latest.map_or(1, |revision| revision.sequence + 1),
            });
        }
    }

    plan_targets(
        manifest,
        &current_software,
        &current_recipes,
        targets,
        &mut actions,
    )?;
    Ok(CatalogPlan {
        schema_version: manifest.schema_version,
        actions,
    })
}

fn plan_targets(
    manifest: &CatalogManifest,
    current_software: &BTreeMap<&str, &Software>,
    current_recipes: &BTreeMap<&str, &(Recipe, Vec<RecipeRevision>)>,
    targets: &[crate::BuildTarget],
    actions: &mut Vec<CatalogAction>,
) -> Result<(), ApiError> {
    for target in &manifest.targets {
        let desired = manifest
            .recipes
            .iter()
            .find(|recipe| recipe.name == target.recipe)
            .expect("validated recipe reference");
        let existing = current_recipes
            .get(target.recipe.as_str())
            .and_then(|(_, revisions)| revisions.iter().max_by_key(|value| value.sequence));
        let revision = match existing.filter(|value| revision_matches(value, &desired.revision)) {
            Some(value) => PlannedRecipeRevision::Existing { id: value.id },
            None => PlannedRecipeRevision::Created {
                recipe: target.recipe.clone(),
                sequence: existing.map_or(1, |value| value.sequence + 1),
            },
        };
        if let Some(current) = targets.iter().find(|value| value.name == target.name) {
            if current_software
                .get(target.software.as_str())
                .is_none_or(|software| software.id != current.software_id)
            {
                return Err(invalid(
                    "target software",
                    "an existing target cannot be rebound to different software",
                ));
            }
            if !matches!(revision, PlannedRecipeRevision::Existing { id } if id == current.recipe_revision_id)
                || current.enabled != target.enabled
                || current.schedule != target.schedule
                || current.parameters != serde_json::json!(target.parameters)
            {
                actions.push(CatalogAction::UpdateTarget {
                    target: target.clone(),
                    revision,
                    expected_revision: current.revision,
                });
            }
        } else {
            actions.push(CatalogAction::CreateTarget {
                target: target.clone(),
                revision,
            });
        }
    }
    Ok(())
}

fn canonical_revision(value: &NewRecipeRevision) -> Result<NewRecipeRevision, ApiError> {
    validate_recipe_revision(value)?;
    let definition = if value.builder == "autopkg" {
        let definition = serde_json::from_value::<NewAutoPkgRevision>(value.definition.clone())
            .map_err(|_| {
                invalid(
                    "AutoPkg recipe definition",
                    "expected the released AutoPkg definition fields",
                )
            })?;
        serde_json::json!({
            "sources": definition.sources,
            "entrypoint": definition.entrypoint,
            "inputs": definition.inputs,
            "output": definition.output,
        })
    } else {
        value.definition.clone()
    };
    let mut required_capabilities = value.required_capabilities.clone();
    match value.builder.as_str() {
        "autopkg" => required_capabilities.extend(["builder.autopkg".into(), "os.macos".into()]),
        "fake" => {
            required_capabilities.extend(["builder.fake".into(), "runtime.portable".into()]);
        }
        _ => unreachable!("builder validation precedes normalization"),
    }
    required_capabilities.sort();
    required_capabilities.dedup();
    Ok(NewRecipeRevision {
        builder: value.builder.clone(),
        definition,
        required_capabilities,
    })
}

fn revision_matches(current: &RecipeRevision, desired: &NewRecipeRevision) -> bool {
    current.builder == desired.builder
        && current.definition == desired.definition
        && current.required_capabilities == desired.required_capabilities
}

fn validate_software(value: &CatalogSoftware) -> Result<(), ApiError> {
    let slug = value.slug.as_bytes();
    let valid_slug = (1..=63).contains(&slug.len())
        && slug
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && slug.first().zip(slug.last()).is_some_and(|(first, last)| {
            first.is_ascii_alphanumeric() && last.is_ascii_alphanumeric()
        })
        && !value.slug.contains("--");
    if !valid_slug {
        return Err(invalid(
            "catalog software slug",
            "expected a 1-63 byte lowercase URL-safe slug",
        ));
    }
    if value.name.trim() != value.name || !(1..=255).contains(&value.name.len()) {
        return Err(invalid(
            "catalog software name",
            "expected 1-255 bytes without leading or trailing whitespace",
        ));
    }
    if let Some(installation) = &value.installation {
        let encoded = serde_json::to_vec(installation).map_err(|_| {
            invalid(
                "catalog installation metadata",
                "installation metadata could not be encoded",
            )
        })?;
        if !installation.install.is_object()
            || !installation.detection.is_object()
            || encoded.len() > 256 * 1024
            || contains_secret_key(&installation.install)
            || contains_secret_key(&installation.detection)
        {
            return Err(invalid(
                "catalog installation metadata",
                "expected two JSON objects totaling at most 256 KiB without credential-like keys",
            ));
        }
    }
    Ok(())
}

fn validate_recipe_name(value: &str) -> Result<(), ApiError> {
    if value.trim() != value || !(1..=128).contains(&value.len()) {
        return Err(invalid(
            "catalog recipe name",
            "expected 1-128 bytes without leading or trailing whitespace",
        ));
    }
    Ok(())
}

fn contains_secret_key(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(values) => values.iter().any(|(key, value)| {
            let key = key.to_ascii_lowercase();
            key.contains("password")
                || key.contains("token")
                || key.contains("secret")
                || key.contains("credential")
                || contains_secret_key(value)
        }),
        serde_json::Value::Array(values) => values.iter().any(contains_secret_key),
        _ => false,
    }
}

fn invalid(kind: &'static str, detail: &'static str) -> ApiError {
    ApiError::InvalidValue { kind, detail }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::{RecipeId, RecipeRevisionId, SoftwareId};

    fn manifest() -> CatalogManifest {
        CatalogManifest {
            targets: vec![],
            schema_version: 1,
            software: vec![CatalogSoftware {
                slug: "firefox".into(),
                name: "Firefox".into(),
                installation: None,
            }],
            recipes: vec![CatalogRecipe {
                name: "firefox".into(),
                revision: NewRecipeRevision::fake(vec!["os.macos".into(), "os.macos".into()]),
            }],
        }
    }

    #[test]
    fn canonicalization_normalizes_capabilities_and_rejects_duplicates() {
        let canonical = manifest().canonicalized().unwrap();
        assert_eq!(
            canonical.recipes[0].revision.required_capabilities,
            ["builder.fake", "os.macos", "runtime.portable"]
        );

        let mut duplicate = manifest();
        duplicate.software.push(duplicate.software[0].clone());
        assert!(duplicate.canonicalized().is_err());
    }

    #[test]
    fn plan_creates_missing_resources_in_dependency_order() {
        let plan = build_plan(
            &ValidatedCatalogManifest::new(manifest()).unwrap(),
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(matches!(
            plan.actions.as_slice(),
            [
                CatalogAction::CreateSoftware { .. },
                CatalogAction::CreateRecipe { .. },
                CatalogAction::CreateRecipeRevision {
                    expected_sequence: 1,
                    ..
                }
            ]
        ));
    }

    #[test]
    fn plan_is_empty_after_convergence() {
        let desired = manifest().canonicalized().unwrap();
        let software = Software {
            id: "01900000-0000-7000-8000-000000000001"
                .parse::<SoftwareId>()
                .unwrap(),
            slug: "firefox".into(),
            name: "Firefox".into(),
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            revision: 1,
            installation: None,
        };
        let recipe = Recipe {
            id: "01900000-0000-7000-8000-000000000002"
                .parse::<RecipeId>()
                .unwrap(),
            name: "firefox".into(),
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            revision: 1,
        };
        let revision = RecipeRevision {
            id: "01900000-0000-7000-8000-000000000003"
                .parse::<RecipeRevisionId>()
                .unwrap(),
            recipe_id: recipe.id,
            sequence: 1,
            builder: desired.recipes[0].revision.builder.clone(),
            definition: desired.recipes[0].revision.definition.clone(),
            required_capabilities: desired.recipes[0].revision.required_capabilities.clone(),
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        };

        let plan = build_plan(
            &ValidatedCatalogManifest::new(desired.clone()).unwrap(),
            &[software],
            &[(recipe, vec![revision])],
            &[],
        )
        .unwrap();
        assert!(plan.is_empty());
    }

    #[test]
    fn software_updates_chain_revisions_and_omission_is_unmanaged() {
        let mut desired = manifest();
        desired.software[0].name = "Firefox ESR".into();
        desired.software[0].installation = Some(InstallationMetadata {
            install: serde_json::json!({"kind": "pkg"}),
            detection: serde_json::json!({"bundle_id": "org.mozilla.firefox"}),
        });
        let software = Software {
            id: "01900000-0000-7000-8000-000000000004"
                .parse::<SoftwareId>()
                .unwrap(),
            slug: "firefox".into(),
            name: "Firefox".into(),
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            revision: 4,
            installation: None,
        };

        let plan = build_plan(
            &ValidatedCatalogManifest::new(desired.clone()).unwrap(),
            std::slice::from_ref(&software),
            &[],
            &[],
        )
        .unwrap();
        assert!(matches!(
            plan.actions.as_slice(),
            [
                CatalogAction::UpdateSoftwareName {
                    expected_revision: 4,
                    ..
                },
                CatalogAction::UpdateSoftwareInstallation {
                    expected_revision: 5,
                    ..
                },
                CatalogAction::CreateRecipe { .. },
                CatalogAction::CreateRecipeRevision { .. }
            ]
        ));

        desired.software[0].name = "Firefox".into();
        desired.software[0].installation = None;
        let plan = build_plan(
            &ValidatedCatalogManifest::new(desired.clone()).unwrap(),
            &[software],
            &[],
            &[],
        )
        .unwrap();
        assert!(matches!(
            plan.actions.as_slice(),
            [
                CatalogAction::CreateRecipe { .. },
                CatalogAction::CreateRecipeRevision { .. }
            ]
        ));
    }

    #[test]
    fn autopkg_definition_defaults_are_canonicalized_before_comparison() {
        let manifest = CatalogManifest {
            targets: vec![],
            schema_version: 1,
            software: vec![],
            recipes: vec![CatalogRecipe {
                name: "autopkg-firefox".into(),
                revision: NewRecipeRevision {
                    builder: "autopkg".into(),
                    definition: serde_json::json!({
                        "sources": [{
                            "url": "https://example.test/recipes.git",
                            "commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        }],
                        "entrypoint": "Firefox.pkg.recipe",
                        "output": {
                            "version_pointer": "/version",
                            "recipe_trust_pointer": "/stabbur/recipe_trust_succeeded",
                            "variants": [{
                                "platform": "mac_os",
                                "architecture": "aarch64",
                                "minimum_macos": null,
                                "maximum_macos": null,
                                "resolution_priority": 0,
                                "artifacts": [{
                                    "path_pointer": "/pkg_path",
                                    "media_type": "application/vnd.apple.installer+xml",
                                    "role": "primary_installer"
                                }]
                            }],
                            "verification": [{
                                "name": "signature",
                                "pointer": "/verified",
                                "required": true
                            }]
                        }
                    }),
                    required_capabilities: vec![],
                },
            }],
        };

        let canonical = manifest.canonicalized().unwrap();
        let revision = &canonical.recipes[0].revision;
        assert_eq!(revision.definition["inputs"], serde_json::json!({}));
        assert_eq!(
            revision.required_capabilities,
            ["builder.autopkg", "os.macos"]
        );

        let mut invalid = manifest.clone();
        invalid.recipes[0].revision.definition["unexpected"] = serde_json::Value::Null;
        assert!(invalid.canonicalized().is_err());

        let mut sensitive = manifest;
        sensitive.recipes[0].revision.definition["inputs"] =
            serde_json::json!({"api_token": "must-not-enter-a-plan"});
        assert!(sensitive.canonicalized().is_err());
    }
}

/// Reviewed desired execution policy declared by catalog schema 2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogTarget {
    /// Stable target name.
    pub name: String,
    /// Software slug declared in this manifest.
    pub software: String,
    /// Recipe name declared in this manifest; resolves to its exact desired revision.
    pub recipe: String,
    /// Non-secret parameters.
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    /// Manual or interval policy.
    pub schedule: crate::BuildTargetSchedule,
    /// Explicit execution eligibility.
    pub enabled: bool,
}
/// A revision reference cannot be simultaneously existing and pending creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlannedRecipeRevision {
    /// Use an existing immutable revision selected during planning.
    Existing {
        /// Exact revision.
        id: crate::RecipeRevisionId,
    },
    /// Use a revision created by an earlier action in this same plan.
    Created {
        /// Recipe owning the revision.
        recipe: String,
        /// Exact expected sequence enforced transactionally by the server.
        sequence: u64,
    },
}
impl PlannedRecipeRevision {
    pub(crate) fn resolve(
        &self,
        created: &BTreeMap<(String, u64), crate::RecipeRevisionId>,
    ) -> Result<crate::RecipeRevisionId, ApiError> {
        match self {
            Self::Existing { id } => Ok(*id),
            Self::Created { recipe, sequence } => created
                .get(&(recipe.clone(), *sequence))
                .copied()
                .ok_or_else(|| {
                    invalid(
                        "planned recipe revision",
                        "a required earlier action did not produce its expected revision",
                    )
                }),
        }
    }
}
/// Canonical, fully validated catalog desired state. Fields cannot be mutated or deserialized unchecked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CatalogManifest", into = "CatalogManifest")]
pub struct ValidatedCatalogManifest(CatalogManifest);
impl ValidatedCatalogManifest {
    /// Validates and canonicalizes all entries and references once.
    #[allow(clippy::needless_pass_by_value)] // Consume the raw DTO when establishing the immutable proof.
    pub fn new(value: CatalogManifest) -> Result<Self, ApiError> {
        Ok(Self(value.canonicalized()?))
    }
    /// Read-only view of the canonical manifest.
    pub fn as_manifest(&self) -> &CatalogManifest {
        &self.0
    }
}
impl TryFrom<CatalogManifest> for ValidatedCatalogManifest {
    type Error = ApiError;
    fn try_from(value: CatalogManifest) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<ValidatedCatalogManifest> for CatalogManifest {
    fn from(value: ValidatedCatalogManifest) -> Self {
        value.0
    }
}

/// A validated, immutable replacement pin. Parsing cannot bypass source URL or commit checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "crate::PinnedSource", into = "crate::PinnedSource")]
pub struct ValidatedSourcePin(crate::PinnedSource);
impl TryFrom<crate::PinnedSource> for ValidatedSourcePin {
    type Error = ApiError;
    fn try_from(value: crate::PinnedSource) -> Result<Self, Self::Error> {
        crate::client::validate_pinned_source(&value)?;
        Ok(Self(value))
    }
}
impl From<ValidatedSourcePin> for crate::PinnedSource {
    fn from(value: ValidatedSourcePin) -> Self {
        value.0
    }
}
/// Reviewable exact source change; no upstream ordering or trust acceptance is inferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourcePinChange {
    /// Affected recipe name.
    pub recipe: String,
    /// Credential-free source URL.
    pub url: String,
    /// Previous exact Git commit.
    pub previous_commit: String,
    /// Proposed exact Git commit.
    pub proposed_commit: String,
}
/// Proposed catalog and its exact source diff. Construction preserves catalog validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceUpdateProposal {
    manifest: ValidatedCatalogManifest,
    changes: Vec<SourcePinChange>,
}
impl SourceUpdateProposal {
    /// Validated proposed desired state, ready for a separate plan/review/apply cycle.
    #[must_use]
    pub fn manifest(&self) -> &ValidatedCatalogManifest {
        &self.manifest
    }
    /// All affected recipe source pins.
    #[must_use]
    pub fn changes(&self) -> &[SourcePinChange] {
        &self.changes
    }
}
impl ValidatedCatalogManifest {
    /// Proposes a caller-selected exact source pin without executing recipes or accepting trust changes.
    pub fn propose_source_update(
        &self,
        replacement: &ValidatedSourcePin,
    ) -> Result<SourceUpdateProposal, ApiError> {
        let mut manifest = self.0.clone();
        let mut changes = Vec::new();
        for recipe in &mut manifest.recipes {
            if recipe.revision.builder != "autopkg" {
                continue;
            }
            let mut definition: NewAutoPkgRevision =
                serde_json::from_value(recipe.revision.definition.clone())
                    .map_err(|_| ApiError::Decode)?;
            for source in &mut definition.sources {
                if source.url == replacement.0.url && source.commit != replacement.0.commit {
                    changes.push(SourcePinChange {
                        recipe: recipe.name.clone(),
                        url: source.url.clone(),
                        previous_commit: source.commit.clone(),
                        proposed_commit: replacement.0.commit.clone(),
                    });
                    source.commit.clone_from(&replacement.0.commit);
                }
            }
            recipe.revision.definition =
                serde_json::to_value(definition).map_err(|_| ApiError::Decode)?;
        }
        if changes.is_empty() {
            return Err(invalid(
                "source update",
                "no managed recipe uses a different pin for this exact source URL",
            ));
        }
        Ok(SourceUpdateProposal {
            manifest: Self::new(manifest)?,
            changes,
        })
    }
}
