mod catalog_ops;
use std::fmt;

use crate::SecretToken;

#[cfg(feature = "async")]
/// Asynchronous client and resource handles.
pub mod r#async;
#[cfg(feature = "blocking")]
/// Blocking client and resource handles.
pub mod blocking;

/// Typestate for a client without credentials.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unauthenticated;

/// Typestate for an authenticated client.
#[derive(Clone)]
pub struct Authenticated {
    pub(crate) token: SecretToken,
    pub(crate) expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Authenticated {
    pub(crate) fn with_expiry(
        token: SecretToken,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            token,
            expires_at: Some(expires_at),
        }
    }
    pub(crate) fn new(token: SecretToken) -> Self {
        Self {
            token,
            expires_at: None,
        }
    }
}

impl fmt::Debug for Authenticated {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Authenticated")
            .field("token", &"[REDACTED]")
            .finish()
    }
}

pub(crate) fn validate_page(limit: u32) -> Result<(), crate::ApiError> {
    if !(1..=200).contains(&limit) {
        return Err(crate::ApiError::InvalidValue {
            kind: "page limit",
            detail: "expected a value from 1 through 200",
        });
    }
    Ok(())
}

pub(crate) fn validate_autopkg_revision(
    value: &crate::NewAutoPkgRevision,
) -> Result<(), crate::ApiError> {
    validate_autopkg_source_and_inputs(value)?;
    validate_autopkg_output(&value.output)
}

fn validate_autopkg_source_and_inputs(
    value: &crate::NewAutoPkgRevision,
) -> Result<(), crate::ApiError> {
    if value.sources.is_empty()
        || value.sources.len() > 16
        || value
            .sources
            .iter()
            .any(|source| !valid_pinned_source(source))
    {
        return Err(invalid_autopkg(
            "sources must be 1-16 credential-free HTTPS URLs pinned to full lowercase commits",
        ));
    }
    if value.entrypoint.trim() != value.entrypoint
        || !(1..=255).contains(&value.entrypoint.len())
        || !value
            .entrypoint
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || value.entrypoint.starts_with('-')
    {
        return Err(invalid_autopkg(
            "entrypoint is not a safe AutoPkg recipe identifier",
        ));
    }
    if value.inputs.len() > 128
        || value.inputs.iter().any(|(key, value)| {
            let lower = key.to_ascii_lowercase();
            key.trim() != key
                || !(1..=128).contains(&key.len())
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
                || key.starts_with('-')
                || lower.contains("password")
                || lower.contains("token")
                || lower.contains("secret")
                || value.len() > 8 * 1_024
                || value.chars().any(char::is_control)
        })
    {
        return Err(invalid_autopkg(
            "inputs must be bounded non-secret names and non-control values",
        ));
    }
    Ok(())
}

fn valid_pinned_source(source: &crate::PinnedSource) -> bool {
    let url = url::Url::parse(&source.url).ok();
    source.url.len() <= 2_048
        && url.as_ref().is_some_and(|url| {
            url.scheme() == "https"
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
        })
        && source.commit.len() == 40
        && source
            .commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn validate_pinned_source(source: &crate::PinnedSource) -> Result<(), crate::ApiError> {
    if valid_pinned_source(source) {
        Ok(())
    } else {
        Err(crate::ApiError::InvalidValue {
            kind: "AutoPkg catalog source",
            detail: "expected a credential-free HTTPS URL pinned to a full lowercase commit",
        })
    }
}

fn validate_autopkg_output(output: &crate::AutoPkgOutputSelectors) -> Result<(), crate::ApiError> {
    if !valid_pointer(&output.version_pointer)
        || output.recipe_trust_pointer != "/stabbur/recipe_trust_succeeded"
        || output.variants.is_empty()
        || output.variants.len() > 64
        || output.verification.is_empty()
        || output.verification.len() > 128
        || !output.verification.iter().any(|check| check.required)
    {
        return Err(invalid_autopkg(
            "output selectors are incomplete or out of bounds",
        ));
    }
    for variant in &output.variants {
        let minimum = variant
            .minimum_macos
            .as_deref()
            .map(parse_macos)
            .transpose()?;
        let maximum = variant
            .maximum_macos
            .as_deref()
            .map(parse_macos)
            .transpose()?;
        let valid_platform = matches!(variant.platform.as_str(), "mac_os" | "linux" | "windows");
        let valid_architecture = matches!(
            variant.architecture.as_str(),
            "x86_64" | "aarch64" | "universal"
        );
        if !valid_platform
            || !valid_architecture
            || (variant.platform != "mac_os" && (minimum.is_some() || maximum.is_some()))
            || minimum
                .as_ref()
                .zip(maximum.as_ref())
                .is_some_and(|(minimum, maximum)| minimum > maximum)
            || variant.artifacts.is_empty()
            || variant.artifacts.len() > 16
            || variant
                .artifacts
                .iter()
                .filter(|artifact| artifact.role == "primary_installer")
                .count()
                != 1
        {
            return Err(invalid_autopkg(
                "variant selectors are invalid or do not select exactly one primary installer",
            ));
        }
        for artifact in &variant.artifacts {
            if !valid_pointer(&artifact.path_pointer)
                || artifact.media_type.trim() != artifact.media_type
                || !(1..=255).contains(&artifact.media_type.len())
                || artifact
                    .media_type
                    .bytes()
                    .any(|byte| byte.is_ascii_control())
                || !matches!(
                    artifact.role.as_str(),
                    "primary_installer" | "signature" | "sbom" | "debug_symbols" | "metadata"
                )
            {
                return Err(invalid_autopkg("artifact selectors are invalid"));
            }
        }
    }
    let mut verification_names = std::collections::BTreeSet::new();
    if output.verification.iter().any(|check| {
        check.name.trim() != check.name
            || !(1..=128).contains(&check.name.len())
            || !valid_pointer(&check.pointer)
            || !verification_names.insert(&check.name)
    }) {
        return Err(invalid_autopkg(
            "verification selectors must have unique bounded names and JSON pointers",
        ));
    }
    Ok(())
}

pub(crate) fn validate_recipe_revision(
    value: &crate::NewRecipeRevision,
) -> Result<(), crate::ApiError> {
    if !value.definition.is_object() {
        return Err(crate::ApiError::InvalidValue {
            kind: "recipe definition",
            detail: "expected a JSON object",
        });
    }
    if value.required_capabilities.iter().any(|capability| {
        !(1..=127).contains(&capability.len())
            || !capability.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'-' | b'_')
            })
            || !capability
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            || !capability
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
    }) {
        return Err(crate::ApiError::InvalidValue {
            kind: "recipe capabilities",
            detail: "expected normalized lowercase capability names",
        });
    }
    match value.builder.as_str() {
        "fake" if value.definition == serde_json::json!({}) => Ok(()),
        "fake" => Err(crate::ApiError::InvalidValue {
            kind: "fake recipe definition",
            detail: "expected an empty JSON object",
        }),
        "autopkg" => {
            let fields = value.definition.as_object().expect("object checked above");
            if fields.keys().any(|field| {
                !matches!(
                    field.as_str(),
                    "sources" | "entrypoint" | "inputs" | "output"
                )
            }) {
                return Err(crate::ApiError::InvalidValue {
                    kind: "AutoPkg recipe definition",
                    detail: "definition contains an unknown field",
                });
            }
            let definition =
                serde_json::from_value::<crate::NewAutoPkgRevision>(value.definition.clone())
                    .map_err(|_| crate::ApiError::InvalidValue {
                        kind: "AutoPkg recipe definition",
                        detail: "expected the released AutoPkg definition fields",
                    })?;
            validate_autopkg_revision(&definition)
        }
        _ => Err(crate::ApiError::InvalidValue {
            kind: "recipe builder",
            detail: "supported builders are `autopkg` and `fake`",
        }),
    }
}

fn valid_pointer(value: &str) -> bool {
    value.starts_with('/') && value.len() <= 1_024
}

fn parse_macos(value: &str) -> Result<Vec<u32>, crate::ApiError> {
    let components = value
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            invalid_autopkg("macOS versions must contain one to four numeric components")
        })?;
    if value.is_empty() || !(1..=4).contains(&components.len()) {
        return Err(invalid_autopkg(
            "macOS versions must contain one to four numeric components",
        ));
    }
    let mut normalized = components;
    normalized.resize(4, 0);
    Ok(normalized)
}

fn invalid_autopkg(detail: &'static str) -> crate::ApiError {
    crate::ApiError::InvalidValue {
        kind: "AutoPkg recipe definition",
        detail,
    }
}

#[cfg(all(test, feature = "async", feature = "blocking"))]
mod parity {
    use super::{Authenticated, Unauthenticated, r#async, blocking};

    #[allow(dead_code, clippy::too_many_lines)]
    fn public_surface_compiles() {
        let _ = r#async::Client::<Unauthenticated>::from_url;
        let _ = blocking::Client::<Unauthenticated>::from_url;
        let _ = r#async::Client::<Unauthenticated>::health;
        let _ = blocking::Client::<Unauthenticated>::health;
        let _ = r#async::Client::<Unauthenticated>::openapi_document;
        let _ = blocking::Client::<Unauthenticated>::openapi_document;
        let _ = r#async::Client::<Unauthenticated>::bootstrap;
        let _ = blocking::Client::<Unauthenticated>::bootstrap;
        let _ = r#async::Client::<Unauthenticated>::login;
        let _ = blocking::Client::<Unauthenticated>::login;
        let _ = r#async::Client::<Unauthenticated>::authenticate;
        let _ = blocking::Client::<Unauthenticated>::authenticate;
        let _ = r#async::Client::<Authenticated>::me;
        let _ = blocking::Client::<Authenticated>::me;
        let _ = r#async::Client::<Authenticated>::change_password;
        let _ = blocking::Client::<Authenticated>::change_password;
        let _ = r#async::Client::<Authenticated>::token;
        let _ = blocking::Client::<Authenticated>::token;
        let _ = r#async::Client::<Authenticated>::identity;
        let _ = blocking::Client::<Authenticated>::identity;
        let _ = r#async::Client::<Authenticated>::software;
        let _ = blocking::Client::<Authenticated>::software;
        let _ = r#async::Client::<Authenticated>::recipes;
        let _ = blocking::Client::<Authenticated>::recipes;
        let _ = r#async::Client::<Authenticated>::catalog;
        let _ = blocking::Client::<Authenticated>::catalog;
        let _ = r#async::Client::<Authenticated>::build_targets;
        let _ = blocking::Client::<Authenticated>::build_targets;
        let _ = r#async::Client::<Authenticated>::runs;
        let _ = blocking::Client::<Authenticated>::runs;
        let _ = r#async::Client::<Authenticated>::artifacts;
        let _ = blocking::Client::<Authenticated>::artifacts;
        let _ = r#async::Client::<Authenticated>::audit;
        let _ = blocking::Client::<Authenticated>::audit;
        let _ = r#async::Client::<Authenticated>::raw;
        let _ = blocking::Client::<Authenticated>::raw;

        let _ = r#async::IdentityResource::list_principals;
        let _ = blocking::IdentityResource::list_principals;
        let _ = r#async::IdentityResource::get_principal;
        let _ = blocking::IdentityResource::get_principal;
        let _ = r#async::IdentityResource::create_human;
        let _ = blocking::IdentityResource::create_human;
        let _ = r#async::IdentityResource::create_service;
        let _ = blocking::IdentityResource::create_service;
        let _ = r#async::IdentityResource::set_roles;
        let _ = blocking::IdentityResource::set_roles;
        let _ = r#async::IdentityResource::set_enabled;
        let _ = blocking::IdentityResource::set_enabled;
        let _ = r#async::IdentityResource::reset_password;
        let _ = blocking::IdentityResource::reset_password;
        let _ = r#async::IdentityResource::revoke_sessions;
        let _ = blocking::IdentityResource::revoke_sessions;
        let _ = r#async::IdentityResource::list_tokens;
        let _ = blocking::IdentityResource::list_tokens;
        let _ = r#async::IdentityResource::create_token;
        let _ = blocking::IdentityResource::create_token;
        let _ = r#async::IdentityResource::revoke_token;
        let _ = blocking::IdentityResource::revoke_token;
        let _ = r#async::IdentityResource::list_roles;
        let _ = blocking::IdentityResource::list_roles;
        let _ = r#async::IdentityResource::create_role;
        let _ = blocking::IdentityResource::create_role;

        let _ = r#async::Client::<Unauthenticated>::readiness;
        let _ = blocking::Client::<Unauthenticated>::readiness;
        let _ = r#async::Client::<Authenticated>::operational_status;
        let _ = blocking::Client::<Authenticated>::operational_status;
        let _ = r#async::Client::<Authenticated>::session_expires_at;
        let _ = blocking::Client::<Authenticated>::session_expires_at;
        let _ = r#async::CatalogResource::plan_validated;
        let _ = blocking::CatalogResource::plan_validated;
        let _ = r#async::CatalogResource::sync_validated;
        let _ = blocking::CatalogResource::sync_validated;
        let _ = r#async::SoftwareResource::status;
        let _ = blocking::SoftwareResource::status;
        let _ = r#async::SoftwareResource::withdraw;
        let _ = blocking::SoftwareResource::withdraw;
        let _ = r#async::RecipeResource::create_revision_at;
        let _ = blocking::RecipeResource::create_revision_at;
        let _ = r#async::WorkerResource::set_draining;
        let _ = blocking::WorkerResource::set_draining;
        let _ = r#async::RunResource::watch;
        let _ = blocking::RunResource::watch;

        let _ = r#async::CatalogResource::plan;
        let _ = blocking::CatalogResource::plan;
        let _ = r#async::CatalogResource::sync;
        let _ = blocking::CatalogResource::sync;
        let _ = r#async::CatalogResource::snapshots;
        let _ = blocking::CatalogResource::snapshots;
        let _ = r#async::CatalogResource::snapshot;
        let _ = blocking::CatalogResource::snapshot;
        let _ = r#async::CatalogResource::resolve;
        let _ = blocking::CatalogResource::resolve;
        let _ = r#async::CatalogResource::scans;
        let _ = blocking::CatalogResource::scans;
        let _ = r#async::CatalogResource::request_autopkg_scan;
        let _ = blocking::CatalogResource::request_autopkg_scan;
        let _ = r#async::CatalogResource::scan;
        let _ = blocking::CatalogResource::scan;
        let _ = r#async::CatalogResource::cancel_scan;
        let _ = blocking::CatalogResource::cancel_scan;

        let _ = r#async::BuildTargetResource::list;
        let _ = blocking::BuildTargetResource::list;
        let _ = r#async::BuildTargetResource::get;
        let _ = blocking::BuildTargetResource::get;
        let _ = r#async::BuildTargetResource::create;
        let _ = blocking::BuildTargetResource::create;
        let _ = r#async::BuildTargetResource::update;
        let _ = blocking::BuildTargetResource::update;
        let _ = r#async::BuildTargetResource::runs;
        let _ = blocking::BuildTargetResource::runs;
        let _ = r#async::BuildTargetResource::trigger;
        let _ = blocking::BuildTargetResource::trigger;

        let _ = r#async::SoftwareResource::list;
        let _ = blocking::SoftwareResource::list;
        let _ = r#async::SoftwareResource::get;
        let _ = blocking::SoftwareResource::get;
        let _ = r#async::SoftwareResource::create;
        let _ = blocking::SoftwareResource::create;
        let _ = r#async::SoftwareResource::create_with_installation;
        let _ = blocking::SoftwareResource::create_with_installation;
        let _ = r#async::SoftwareResource::update_name;
        let _ = blocking::SoftwareResource::update_name;
        let _ = r#async::SoftwareResource::update_installation;
        let _ = blocking::SoftwareResource::update_installation;
        let _ = r#async::SoftwareResource::releases;
        let _ = blocking::SoftwareResource::releases;
        let _ = r#async::SoftwareResource::release;
        let _ = blocking::SoftwareResource::release;
        let _ = r#async::SoftwareResource::variants;
        let _ = blocking::SoftwareResource::variants;
        let _ = r#async::SoftwareResource::reject;
        let _ = blocking::SoftwareResource::reject;
        let _ = r#async::SoftwareResource::channels;
        let _ = blocking::SoftwareResource::channels;
        let _ = r#async::SoftwareResource::channel;
        let _ = blocking::SoftwareResource::channel;
        let _ = r#async::SoftwareResource::promote;
        let _ = blocking::SoftwareResource::promote;
        let _ = r#async::SoftwareResource::resolve;
        let _ = blocking::SoftwareResource::resolve;

        let _ = r#async::RecipeResource::list;
        let _ = blocking::RecipeResource::list;
        let _ = r#async::RecipeResource::get;
        let _ = blocking::RecipeResource::get;
        let _ = r#async::RecipeResource::create;
        let _ = blocking::RecipeResource::create;
        let _ = r#async::RecipeResource::revisions;
        let _ = blocking::RecipeResource::revisions;
        let _ = r#async::RecipeResource::create_revision;
        let _ = blocking::RecipeResource::create_revision;
        let _ = r#async::RecipeResource::create_autopkg_revision;
        let _ = blocking::RecipeResource::create_autopkg_revision;
        let _ = r#async::RecipeResource::runs;
        let _ = blocking::RecipeResource::runs;

        let _ = r#async::RunResource::list;
        let _ = blocking::RunResource::list;
        let _ = r#async::RunResource::create;
        let _ = blocking::RunResource::create;
        let _ = r#async::RunResource::get;
        let _ = blocking::RunResource::get;
        let _ = r#async::RunResource::cancel;
        let _ = blocking::RunResource::cancel;
        let _ = r#async::RunResource::logs;
        let _ = blocking::RunResource::logs;
        let _ = r#async::RunResource::events;
        let _ = blocking::RunResource::events;
        let _ = r#async::RunResource::wait;
        let _ = blocking::RunResource::wait;

        let _ = r#async::ArtifactResource::get;
        let _ = blocking::ArtifactResource::get;
        let _ = r#async::ArtifactResource::locations;
        let _ = blocking::ArtifactResource::locations;
        let _ = r#async::ArtifactResource::head;
        let _ = blocking::ArtifactResource::head;
        let _ = r#async::ArtifactResource::upload_bytes;
        let _ = blocking::ArtifactResource::upload_bytes;
        let _ = r#async::ArtifactResource::download;
        let _ = |resource: &blocking::ArtifactResource,
                 digest: &crate::Sha256Digest,
                 writer: &mut Vec<u8>| resource.download_to(digest, writer, None);

        let _ = r#async::StoreResource::list;
        let _ = blocking::StoreResource::list;
        let _ = r#async::StoreResource::get;
        let _ = blocking::StoreResource::get;
        let _ = r#async::StoreResource::test;
        let _ = blocking::StoreResource::test;

        let _ = r#async::WorkerResource::list;
        let _ = blocking::WorkerResource::list;
        let _ = r#async::WorkerResource::provision;
        let _ = blocking::WorkerResource::provision;
        let _ = r#async::WorkerResource::get;
        let _ = blocking::WorkerResource::get;
        let _ = r#async::WorkerResource::set_enabled;
        let _ = blocking::WorkerResource::set_enabled;
        let _ = r#async::WorkerResource::set_allowed_capabilities;
        let _ = blocking::WorkerResource::set_allowed_capabilities;
        let _ = r#async::WorkerResource::rotate_token;
        let _ = blocking::WorkerResource::rotate_token;

        let _ = r#async::JobResource::list;
        let _ = blocking::JobResource::list;
        let _ = r#async::JobResource::get;
        let _ = blocking::JobResource::get;
        let _ = r#async::AuditResource::list;
        let _ = blocking::AuditResource::list;
    }
}

pub(crate) fn validate_build_parameters(
    values: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Result<(), crate::ApiError> {
    if values.len() > 128
        || values.iter().any(|(key, value)| {
            let lower = key.to_ascii_lowercase();
            key.is_empty()
                || key.len() > 128
                || key.starts_with('-')
                || !key
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
                || lower.contains("secret")
                || lower.contains("password")
                || lower.contains("token")
                || !(value.is_boolean()
                    || value.is_number()
                    || value
                        .as_str()
                        .is_some_and(|s| s.len() <= 8192 && !s.chars().any(char::is_control)))
        })
    {
        return Err(crate::ApiError::InvalidValue {
            kind: "build parameters",
            detail: "expected bounded, non-secret scalar values",
        });
    }
    Ok(())
}

/// A bounded traversal that rejects replayed cursors before another request is made.
#[derive(Default)]
pub(crate) struct CatalogTraversal(std::collections::BTreeSet<String>);
impl CatalogTraversal {
    pub(crate) fn advance(
        &mut self,
        cursor: Option<String>,
    ) -> Result<Option<String>, crate::ApiError> {
        if let Some(value) = &cursor
            && (value.len() > 1024 || self.0.len() >= 500 || !self.0.insert(value.clone()))
        {
            return Err(crate::ApiError::InvalidValue {
                kind: "catalog pagination",
                detail: "cursor repeated or traversal exceeded 500 pages",
            });
        }
        Ok(cursor)
    }
}
