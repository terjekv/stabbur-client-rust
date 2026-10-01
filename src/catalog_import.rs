//! Reviewable imports from immutable worker or repository discovery snapshots.

use crate::{
    ApiError, AutoPkgArtifactSelector, AutoPkgOutputSelectors, AutoPkgVariantSelector,
    AutoPkgVerificationSelector, BuildTargetSchedule, CatalogManifest, CatalogRecipe,
    CatalogSoftware, CatalogTarget, NewAutoPkgRevision, NewRecipeRevision, PinnedSource,
    RecipeCatalogManifest, ValidatedCatalogManifest, ValidatedSourcePin,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Operator choices that cannot safely be inferred from a recipe identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeImportSelection {
    /// Exact identifier from the selected immutable discovery snapshot.
    pub identifier: String,
    /// New software slug; also used as the recipe and disabled target name.
    pub slug: String,
    /// Operator-facing software name.
    pub name: String,
    /// Explicit artifact architecture (`aarch64`, `x86_64`, or `universal`).
    pub architecture: String,
    /// Optional reviewed minimum macOS version.
    pub minimum_macos: Option<String>,
    /// AutoPkg output variable containing the opaque version, typically `version`.
    pub version_variable: String,
    /// AutoPkg output variable containing the installer, typically `pathname` or `pkg_path`.
    pub artifact_variable: String,
    /// Installer media type, for example `application/octet-stream`.
    pub media_type: String,
}

/// Produces disabled, manual build targets and pinned definitions for separate plan/review/apply.
/// Discovery diagnostics and source provenance remain on the immutable snapshot.
pub fn prepare_recipe_import(
    snapshot: &RecipeCatalogManifest,
    selections: &[RecipeImportSelection],
) -> Result<ValidatedCatalogManifest, ApiError> {
    if snapshot.producer != "autopkg"
        || snapshot.schema_version != 1
        || selections.is_empty()
        || selections.len() > 100
    {
        return Err(invalid(
            "select between one and 100 recipes from an AutoPkg snapshot",
        ));
    }
    let mut desired = CatalogManifest {
        schema_version: crate::CATALOG_SCHEMA_VERSION,
        targets: vec![],
        software: vec![],
        recipes: vec![],
    };
    for selection in selections {
        let mut matches = snapshot
            .recipes
            .iter()
            .filter(|entry| entry.identifier == selection.identifier);
        let entry = matches
            .next()
            .ok_or_else(|| invalid("selected recipe is absent from this snapshot"))?;
        if matches.next().is_some()
            || entry.builder != "autopkg"
            || snapshot.diagnostics.iter().any(|diagnostic| {
                (diagnostic.severity == "error" || diagnostic.code == "parent_trust_required")
                    && (diagnostic.identifier.is_none()
                        || diagnostic.identifier.as_deref() == Some(&entry.identifier))
            })
        {
            return Err(invalid(
                "resolve discovery errors and parent trust requirements before importing this recipe",
            ));
        }
        if !artifact_workflow(entry) {
            return Err(invalid(
                "refresh discovery and choose an artifact recipe; installation, publication and unknown-purpose recipes require a different workflow",
            ));
        }
        let pins = entry.import_sources.as_ref().filter(|sources| !sources.is_empty() && sources.len() <= 16).ok_or_else(|| invalid("this snapshot has no complete pinned source closure; refresh discovery or supply a reviewed catalog manifest"))?;
        let sources = pins
            .iter()
            .map(|source| {
                ValidatedSourcePin::try_from(PinnedSource {
                    url: source.locator.clone(),
                    commit: source.revision.clone(),
                })
                .map(Into::into)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let definition = NewAutoPkgRevision {
            sources,
            entrypoint: entry.identifier.clone(),
            inputs: BTreeMap::new(),
            required_capabilities: entry.required_capabilities.clone(),
            output: AutoPkgOutputSelectors {
                version_pointer: output_pointer(&selection.version_variable)?,
                recipe_trust_pointer: "/stabbur/recipe_trust_succeeded".into(),
                variants: vec![AutoPkgVariantSelector {
                    platform: "mac_os".into(),
                    architecture: selection.architecture.clone(),
                    minimum_macos: selection.minimum_macos.clone(),
                    maximum_macos: None,
                    resolution_priority: 0,
                    artifacts: vec![AutoPkgArtifactSelector {
                        path_pointer: output_pointer(&selection.artifact_variable)?,
                        media_type: selection.media_type.clone(),
                        role: "primary_installer".into(),
                    }],
                }],
                verification: vec![AutoPkgVerificationSelector {
                    name: "recipe_trust".into(),
                    pointer: "/stabbur/recipe_trust_succeeded".into(),
                    required: true,
                }],
            },
        };
        desired.software.push(CatalogSoftware {
            slug: selection.slug.clone(),
            name: selection.name.clone(),
            installation: None,
        });
        desired.recipes.push(CatalogRecipe {
            name: selection.slug.clone(),
            revision: NewRecipeRevision::autopkg(&definition),
        });
        desired.targets.push(CatalogTarget {
            name: selection.slug.clone(),
            software: selection.slug.clone(),
            recipe: selection.slug.clone(),
            parameters: BTreeMap::new(),
            schedule: BuildTargetSchedule::Manual,
            enabled: false,
        });
    }
    ValidatedCatalogManifest::new(desired)
}

fn artifact_workflow(entry: &crate::RecipeCatalogEntry) -> bool {
    entry.guidance.as_ref().is_some_and(|guidance| {
        matches!(
            guidance.purpose,
            crate::RecipePurpose::FetchArtifact | crate::RecipePurpose::BuildArtifact
        )
    })
}

fn output_pointer(variable: &str) -> Result<String, ApiError> {
    if variable.is_empty()
        || variable.len() > 128
        || !variable
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || value == b'_')
    {
        return Err(invalid(
            "output variable names must contain only letters, digits, or underscores",
        ));
    }
    Ok(format!("/stabbur/outputs/{variable}"))
}

fn invalid(reason: &'static str) -> ApiError {
    ApiError::InvalidValue {
        kind: "recipe import",
        detail: reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> RecipeCatalogManifest {
        serde_json::from_value(serde_json::json!({"schema_version":1,"producer":"autopkg","source":{"locator":"stabbur-worker:fixture:autopkg","revision":"fixture"},"recipes":[{"identifier":"example.download.App","guidance":{"name":"App","purpose":"fetch_artifact"},"builder":"autopkg","parents":[],"required_capabilities":["builder.autopkg","os.macos"],"import_sources":[{"locator":"https://example.test/recipes.git","revision":"a".repeat(40)}]}],"diagnostics":[]})).unwrap()
    }
    fn selection() -> RecipeImportSelection {
        RecipeImportSelection {
            identifier: "example.download.App".into(),
            slug: "app".into(),
            name: "App".into(),
            architecture: "aarch64".into(),
            minimum_macos: Some("13".into()),
            version_variable: "version".into(),
            artifact_variable: "pathname".into(),
            media_type: "application/octet-stream".into(),
        }
    }
    #[test]
    fn imports_are_pinned_disabled_and_manual_with_no_input_overrides() {
        let imported = prepare_recipe_import(&snapshot(), &[selection()]).unwrap();
        let desired = imported.as_manifest();
        assert!(!desired.targets[0].enabled);
        assert_eq!(desired.targets[0].schedule, BuildTargetSchedule::Manual);
        let definition = &desired.recipes[0].revision.definition;
        assert_eq!(definition["sources"][0]["commit"], "a".repeat(40));
        assert_eq!(definition["entrypoint"], "example.download.App");
        assert_eq!(definition["inputs"], serde_json::json!({}));
        assert_eq!(
            definition["output"]["version_pointer"],
            "/stabbur/outputs/version"
        );
    }
    #[test]
    fn missing_pins_errors_unpinned_sources_and_duplicate_names_are_rejected() {
        let mut source = snapshot();
        source.recipes[0].import_sources = None;
        assert!(prepare_recipe_import(&source, &[selection()]).is_err());
        source = snapshot();
        source.recipes[0].import_sources.as_mut().unwrap()[0].revision = "main".into();
        assert!(prepare_recipe_import(&source, &[selection()]).is_err());
        source = snapshot();
        source.diagnostics.push(crate::RecipeCatalogDiagnostic {
            identifier: None,
            code: "broken".into(),
            severity: "error".into(),
            detail: "fixture".into(),
        });
        assert!(prepare_recipe_import(&source, &[selection()]).is_err());
        assert!(prepare_recipe_import(&snapshot(), &[selection(), selection()]).is_err());
        let mut invalid = selection();
        invalid.architecture = "guess".into();
        assert!(prepare_recipe_import(&snapshot(), &[invalid]).is_err());
    }
    #[test]
    fn guided_import_rejects_side_effects_legacy_snapshots_and_missing_trust() {
        for purpose in [
            crate::RecipePurpose::Install,
            crate::RecipePurpose::Publish,
            crate::RecipePurpose::Unknown,
        ] {
            let mut source = snapshot();
            source.recipes[0].guidance.as_mut().unwrap().purpose = purpose;
            assert!(prepare_recipe_import(&source, &[selection()]).is_err());
        }
        let mut source = snapshot();
        source.recipes[0].guidance = None;
        assert!(prepare_recipe_import(&source, &[selection()]).is_err());
        source = snapshot();
        source.diagnostics.push(crate::RecipeCatalogDiagnostic {
            identifier: Some(selection().identifier),
            code: "parent_trust_required".into(),
            severity: "warning".into(),
            detail: "Review the parent chain.".into(),
        });
        assert!(prepare_recipe_import(&source, &[selection()]).is_err());
    }
}
