use std::{fmt, str::FromStr, sync::Arc};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::ApiError;

/// Validated Stabbur server origin.
#[derive(Clone, PartialEq, Eq)]
pub struct BaseUrl(url::Url);

impl BaseUrl {
    /// Parses an HTTP(S) origin without credentials, query, fragment, or a path prefix.
    pub fn parse(value: &str) -> Result<Self, ApiError> {
        let mut url = url::Url::parse(value)
            .map_err(|_| ApiError::InvalidBaseUrl("expected an absolute HTTP(S) URL"))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(ApiError::InvalidBaseUrl("expected an absolute HTTP(S) URL"));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(ApiError::InvalidBaseUrl(
                "embedded credentials are forbidden",
            ));
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(ApiError::InvalidBaseUrl(
                "query strings and fragments are forbidden",
            ));
        }
        if url.path() != "/" && !url.path().is_empty() {
            return Err(ApiError::InvalidBaseUrl("path prefixes are not supported"));
        }
        url.set_path("");
        Ok(Self(url))
    }

    pub(crate) fn endpoint(&self, path: &str) -> String {
        debug_assert!(path.starts_with('/'));
        format!("{}{path}", self.0.as_str().trim_end_matches('/'))
    }
}

impl fmt::Debug for BaseUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BaseUrl")
            .field(&self.0.origin())
            .finish()
    }
}

impl fmt::Display for BaseUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.origin().ascii_serialization().fmt(formatter)
    }
}

impl FromStr for BaseUrl {
    type Err = ApiError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// Secret bearer value that always renders redacted.
#[derive(Clone)]
pub struct SecretToken(Arc<str>);

impl SecretToken {
    /// Wraps a non-empty token returned by a protected secret source.
    pub fn new(value: impl Into<String>) -> Result<Self, ApiError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ApiError::InvalidValue {
                kind: "token",
                detail: "must not be empty",
            });
        }
        Ok(Self(Arc::from(value)))
    }

    /// Exposes the value only for an authorization header or protected credential file.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretToken([REDACTED])")
    }
}

impl<'de> Deserialize<'de> for SecretToken {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Human login credentials with password redaction.
#[derive(Clone)]
pub struct Credentials {
    username: String,
    password: Arc<str>,
}

impl Credentials {
    /// Creates credentials obtained from an interactive or protected source.
    #[must_use]
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: Arc::from(password.into()),
        }
    }

    pub(crate) fn username(&self) -> &str {
        &self.username
    }
    pub(crate) fn password(&self) -> &str {
        &self.password
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

macro_rules! uuid_v7_id {
    ($name:ident, $kind:literal) => {
        #[doc = concat!("UUIDv7 ", $kind, " identity.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[doc = concat!("Parses a UUIDv7 ", $kind, " identity.")]
            pub fn parse(value: &str) -> Result<Self, ApiError> {
                let id = Uuid::parse_str(value).map_err(|_| ApiError::InvalidValue {
                    kind: concat!($kind, " ID"),
                    detail: "expected UUIDv7",
                })?;
                if id.get_version_num() != 7 {
                    return Err(ApiError::InvalidValue {
                        kind: concat!($kind, " ID"),
                        detail: "expected UUIDv7",
                    });
                }
                Ok(Self(id))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = ApiError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).map_err(serde::de::Error::custom)
            }
        }
    };
}

uuid_v7_id!(SoftwareId, "software");
uuid_v7_id!(PrincipalId, "principal");
uuid_v7_id!(RecipeId, "recipe");
uuid_v7_id!(RecipeRevisionId, "recipe revision");
uuid_v7_id!(RecipeCatalogSnapshotId, "recipe catalog snapshot");
uuid_v7_id!(RecipeCatalogScanId, "recipe catalog scan");
uuid_v7_id!(BuildTargetId, "build target");
uuid_v7_id!(RunId, "run");
uuid_v7_id!(JobId, "job");
uuid_v7_id!(ReleaseId, "release");
uuid_v7_id!(VariantId, "variant");
uuid_v7_id!(StoreId, "store");
uuid_v7_id!(WorkerId, "worker");
uuid_v7_id!(ArtifactLocationId, "artifact location");

/// Lowercase SHA-256 digest.
#[derive(Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    /// Validates a 64-character lowercase hexadecimal digest.
    pub fn parse(value: &str) -> Result<Self, ApiError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ApiError::InvalidValue {
                kind: "SHA-256 digest",
                detail: "expected 64 lowercase hexadecimal characters",
            });
        }
        Ok(Self(value.to_owned()))
    }

    /// Returns the normalized digest.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
impl FromStr for Sha256Digest {
    type Err = ApiError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}
impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

/// Cursor-paginated response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorPage<T> {
    /// Items in this page.
    pub items: Vec<T>,
    /// Opaque next cursor.
    pub next_cursor: Option<String>,
}

/// Authenticated principal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    /// Principal identity.
    pub id: PrincipalId,
    /// Login or service name.
    pub name: String,
    /// Principal kind.
    pub kind: String,
    /// Assigned role names.
    pub roles: Vec<String>,
}

/// Administrative principal metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrincipalAdmin {
    /// Principal identity.
    pub id: PrincipalId,
    /// Login or service name.
    pub name: String,
    /// Principal kind.
    pub kind: String,
    /// Assigned roles.
    pub roles: Vec<String>,
    /// Whether authentication is enabled.
    pub enabled: bool,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Optimistic-concurrency revision.
    pub revision: u64,
}

/// Declarative installation and installed-state detection metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallationMetadata {
    /// Client- or packager-interpreted installation metadata.
    pub install: serde_json::Value,
    /// Client- or packager-interpreted detection metadata.
    pub detection: serde_json::Value,
}

/// Software catalog resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Software {
    /// UUIDv7 identity.
    pub id: SoftwareId,
    /// Unique lowercase slug.
    pub slug: String,
    /// Human-readable name.
    pub name: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Optimistic-concurrency revision.
    pub revision: u64,
    /// Declarative installation metadata.
    pub installation: Option<InstallationMetadata>,
}

/// One durable artifact location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLocation {
    /// Location identity.
    pub id: ArtifactLocationId,
    /// Store identity.
    pub store_id: StoreId,
    /// Location state.
    pub state: String,
    /// Last verification time.
    pub verified_at: Option<DateTime<Utc>>,
    /// Safe adapter error summary.
    pub last_error: Option<String>,
}

/// Immutable artifact metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Content digest.
    pub digest: Sha256Digest,
    /// Exact size.
    pub size: u64,
    /// Media type.
    pub media_type: String,
    /// Ingestion time.
    pub created_at: DateTime<Utc>,
    /// Independently tracked locations.
    pub locations: Vec<ArtifactLocation>,
}

/// Verified artifact upload outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactUpload {
    /// Verified digest.
    pub digest: Sha256Digest,
    /// Verified size.
    pub size: u64,
    /// Whether existing content was reused.
    pub reused: bool,
}

/// Metadata returned by an artifact-content `HEAD` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactContentHead {
    /// Exact immutable content size.
    pub content_length: u64,
    /// Strong digest ETag.
    pub etag: String,
    /// Whether the endpoint advertises byte ranges.
    pub accepts_ranges: bool,
}

/// Append-only audit event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Audit identity.
    pub id: String,
    /// Actor representation.
    pub actor: serde_json::Value,
    /// Stable action.
    pub action: String,
    /// Resource kind.
    pub resource_kind: String,
    /// Optional resource identity.
    pub resource_id: Option<String>,
    /// Safe details.
    pub details: serde_json::Value,
    /// Correlation identity.
    pub request_id: Option<String>,
    /// Event time.
    pub occurred_at: DateTime<Utc>,
}

/// Cursor page of audit events.
pub type AuditPage = CursorPage<AuditEvent>;

/// Long-lived API-token metadata; the secret is never recoverable here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiToken {
    /// Token identity.
    pub id: String,
    /// Owning principal.
    pub principal_id: PrincipalId,
    /// Operator-facing name.
    pub name: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Optional expiration.
    pub expires_at: Option<DateTime<Utc>>,
    /// Optional revocation time.
    pub revoked_at: Option<DateTime<Utc>>,
}

/// One-time API-token creation result.
#[derive(Clone, Deserialize)]
pub struct CreatedApiToken {
    /// Persisted token metadata.
    pub token: ApiToken,
    /// One-time bearer secret.
    pub secret: SecretToken,
}

impl fmt::Debug for CreatedApiToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreatedApiToken")
            .field("token", &self.token)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

/// Role and permission policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    /// Role name.
    pub name: String,
    /// Exact permission names.
    pub permissions: Vec<String>,
    /// Whether this role is built in and immutable.
    pub built_in: bool,
    /// Optimistic-concurrency revision.
    pub revision: u64,
}

/// Recipe metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipe {
    /// Recipe identity.
    pub id: RecipeId,
    /// Unique name.
    pub name: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Optimistic-concurrency revision.
    pub revision: u64,
}

/// Pinned Git source for an immutable AutoPkg revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedSource {
    /// Absolute HTTPS Git URL.
    pub url: String,
    /// Full lowercase Git commit.
    pub commit: String,
}

/// Manual or fixed-interval build-target trigger policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BuildTargetSchedule {
    /// Runs are created only through an explicit trigger request.
    Manual,
    /// Runs become eligible at a durable fixed-interval cursor.
    Interval {
        /// Seconds between eligible cursors.
        every_seconds: u32,
    },
}

/// Persisted desired build policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildTarget {
    /// Target identity.
    pub id: BuildTargetId,
    /// Unique operator-facing name.
    pub name: String,
    /// Software receiving build output.
    pub software_id: SoftwareId,
    /// Exact immutable recipe revision.
    pub recipe_revision_id: RecipeRevisionId,
    /// Declared non-secret builder parameters.
    pub parameters: serde_json::Value,
    /// Manual or recurring trigger policy.
    pub schedule: BuildTargetSchedule,
    /// Whether manual and scheduled triggering is allowed.
    pub enabled: bool,
    /// Next recurring cursor, absent for manual targets.
    pub next_run_at: Option<DateTime<Utc>>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Latest configuration or cursor change time.
    pub updated_at: DateTime<Utc>,
    /// Optimistic-concurrency revision.
    pub revision: u64,
}

/// Complete replacement fields for a build-target update.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildTargetUpdate {
    /// Replacement unique name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Replacement immutable recipe revision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe_revision: Option<RecipeRevisionId>,
    /// Replacement non-secret parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    /// Replacement trigger policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<BuildTargetSchedule>,
    /// Replacement recurring cursor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run_at: Option<DateTime<Utc>>,
    /// Replacement enabled state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Exact builder-neutral recipe catalog source observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCatalogSource {
    /// Stable source locator.
    pub locator: String,
    /// Exact opaque observed revision.
    pub revision: String,
}

/// One normalized recipe observed in a catalog snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCatalogEntry {
    /// Builder-owned stable identifier or entrypoint.
    pub identifier: String,
    /// Stable builder adapter selector.
    pub builder: String,
    /// Normalized parent identifiers.
    pub parents: Vec<String>,
    /// Capabilities required to execute this recipe.
    pub required_capabilities: Vec<String>,
}

/// One safe catalog validation diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCatalogDiagnostic {
    /// Recipe identifier when the diagnostic belongs to one entry.
    pub identifier: Option<String>,
    /// Stable machine-readable code.
    pub code: String,
    /// `info`, `warning`, or `error`.
    pub severity: String,
    /// Safe bounded operator-facing detail.
    pub detail: String,
}

/// Complete canonical builder-neutral catalog manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCatalogManifest {
    /// Catalog wire schema version.
    pub schema_version: u32,
    /// Stable producer adapter.
    pub producer: String,
    /// Exact pinned source.
    pub source: RecipeCatalogSource,
    /// Sorted normalized recipes.
    pub recipes: Vec<RecipeCatalogEntry>,
    /// Sorted safe diagnostics.
    pub diagnostics: Vec<RecipeCatalogDiagnostic>,
}

/// Lightweight immutable catalog snapshot metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCatalogSnapshotSummary {
    /// Snapshot identity.
    pub id: RecipeCatalogSnapshotId,
    /// Authenticated publishing worker.
    pub worker_id: WorkerId,
    /// Stable producer adapter.
    pub producer: String,
    /// Exact pinned source.
    pub source: RecipeCatalogSource,
    /// Lowercase SHA-256 manifest digest.
    pub manifest_digest: Sha256Digest,
    /// Normalized recipe count.
    pub recipe_count: u32,
    /// Safe diagnostic count.
    pub diagnostic_count: u32,
    /// Server observation time.
    pub observed_at: DateTime<Utc>,
}

/// Immutable catalog snapshot including its bounded manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCatalogSnapshot {
    /// Snapshot metadata.
    pub summary: RecipeCatalogSnapshotSummary,
    /// Complete canonical manifest.
    pub manifest: RecipeCatalogManifest,
}

/// One exact catalog identifier match from a latest source snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCatalogMatch {
    /// Containing snapshot identity.
    pub snapshot_id: RecipeCatalogSnapshotId,
    /// Publishing worker identity.
    pub worker_id: WorkerId,
    /// Stable producer adapter.
    pub producer: String,
    /// Exact pinned source.
    pub source: RecipeCatalogSource,
    /// Normalized matching recipe.
    pub recipe: RecipeCatalogEntry,
    /// Server observation time.
    pub observed_at: DateTime<Utc>,
}

/// Exact identifier lookup across latest source snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCatalogLookup {
    /// Exact requested identifier.
    pub identifier: String,
    /// Whether at least one match exists.
    pub exists: bool,
    /// Matching latest observations.
    pub matches: Vec<RecipeCatalogMatch>,
}

/// Safe typed terminal catalog-scan failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCatalogScanFailure {
    /// Stable producer when the worker decoded the request.
    pub producer: Option<String>,
    /// Stable machine-readable code.
    pub code: String,
    /// Safe operator-facing detail.
    pub detail: String,
    /// Worker failure time.
    pub failed_at: DateTime<Utc>,
}

/// Durable server-requested recipe catalog scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCatalogScan {
    /// Scan identity.
    pub id: RecipeCatalogScanId,
    /// Durable worker job identity.
    pub job_id: JobId,
    /// Stable producer adapter.
    pub producer: String,
    /// Exact pinned source.
    pub source: RecipeCatalogSource,
    /// Durable queue state.
    pub state: String,
    /// Published snapshot after successful completion.
    pub snapshot_id: Option<RecipeCatalogSnapshotId>,
    /// Safe typed failure after unsuccessful completion.
    pub failure: Option<RecipeCatalogScanFailure>,
    /// Server request time.
    pub requested_at: DateTime<Utc>,
    /// Terminal completion time.
    pub completed_at: Option<DateTime<Utc>>,
}

/// AutoPkg artifact selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoPkgArtifactSelector {
    /// JSON pointer resolving to an isolated output path.
    pub path_pointer: String,
    /// Stored media type.
    pub media_type: String,
    /// Semantic artifact role.
    pub role: String,
}

/// AutoPkg variant selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoPkgVariantSelector {
    /// Platform.
    pub platform: String,
    /// Architecture.
    pub architecture: String,
    /// Inclusive minimum macOS version.
    pub minimum_macos: Option<String>,
    /// Inclusive maximum macOS version.
    pub maximum_macos: Option<String>,
    /// Resolver priority.
    pub resolution_priority: i32,
    /// Selected artifacts.
    pub artifacts: Vec<AutoPkgArtifactSelector>,
}

/// AutoPkg verification selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoPkgVerificationSelector {
    /// Stable check name.
    pub name: String,
    /// JSON pointer resolving to a boolean.
    pub pointer: String,
    /// Whether publication requires success.
    pub required: bool,
}

/// Reviewed AutoPkg output selectors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoPkgOutputSelectors {
    /// Version JSON pointer.
    pub version_pointer: String,
    /// Recipe-trust JSON pointer.
    pub recipe_trust_pointer: String,
    /// Variant selectors.
    pub variants: Vec<AutoPkgVariantSelector>,
    /// Verification selectors.
    pub verification: Vec<AutoPkgVerificationSelector>,
}

/// Checked immutable AutoPkg revision request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewAutoPkgRevision {
    /// Pinned sources.
    pub sources: Vec<PinnedSource>,
    /// AutoPkg recipe identifier.
    pub entrypoint: String,
    /// Declared non-secret input values.
    #[serde(default)]
    pub inputs: std::collections::BTreeMap<String, String>,
    /// Reviewed report selectors.
    pub output: AutoPkgOutputSelectors,
    /// Additional scheduling requirements.
    #[serde(default)]
    pub required_capabilities: Vec<String>,
}

/// Builder-neutral immutable recipe revision request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewRecipeRevision {
    /// Stable server-supported builder adapter name.
    pub builder: String,
    /// Adapter-owned immutable definition.
    pub definition: serde_json::Value,
    /// Additional scheduling requirements beyond the selected builder's mandatory capabilities.
    #[serde(default)]
    pub required_capabilities: Vec<String>,
}

impl NewRecipeRevision {
    /// Creates a deterministic portable fake-builder revision for smoke tests and small installs.
    #[must_use]
    pub fn fake(required_capabilities: Vec<String>) -> Self {
        Self {
            builder: "fake".to_owned(),
            definition: serde_json::json!({}),
            required_capabilities,
        }
    }

    /// Wraps a checked AutoPkg definition in the builder-neutral revision contract.
    #[must_use]
    pub fn autopkg(revision: &NewAutoPkgRevision) -> Self {
        Self {
            builder: "autopkg".to_owned(),
            definition: serde_json::json!({
                "sources": revision.sources,
                "entrypoint": revision.entrypoint,
                "inputs": revision.inputs,
                "output": revision.output,
            }),
            required_capabilities: revision.required_capabilities.clone(),
        }
    }
}

/// Immutable recipe revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeRevision {
    /// Revision identity.
    pub id: RecipeRevisionId,
    /// Parent recipe.
    pub recipe_id: RecipeId,
    /// Monotonic sequence.
    pub sequence: u64,
    /// Builder adapter identifier.
    pub builder: String,
    /// Immutable builder definition.
    pub definition: serde_json::Value,
    /// Scheduling requirements.
    pub required_capabilities: Vec<String>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

/// Run summary returned by collection endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummary {
    /// Run identity.
    pub id: RunId,
    /// Immutable recipe revision.
    pub recipe_revision_id: RecipeRevisionId,
    /// Software identity.
    pub software_id: SoftwareId,
    /// Durable state.
    pub state: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Terminal time.
    pub completed_at: Option<DateTime<Utc>>,
}

/// Durable run details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    /// Run identity.
    pub id: RunId,
    /// Immutable recipe revision.
    pub recipe_revision_id: RecipeRevisionId,
    /// Software identity.
    pub software_id: SoftwareId,
    /// Durable state.
    pub state: String,
    /// Non-secret builder parameters.
    pub parameters: serde_json::Value,
    /// Builder-neutral result and provenance.
    pub result: Option<serde_json::Value>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Terminal time.
    pub completed_at: Option<DateTime<Utc>>,
}

impl Run {
    /// Whether the run is in a terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self.state.as_str(), "succeeded" | "failed" | "cancelled")
    }
}

/// Ordered exact-byte run log entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunLog {
    /// Emitting attempt UUIDv7 in wire form.
    pub attempt_id: String,
    /// Monotonic sequence.
    pub sequence: u64,
    /// `stdout`, `stderr`, or `system`.
    pub stream: String,
    /// Standard-base64 exact bytes.
    pub message_base64: String,
    /// Server receipt time.
    pub occurred_at: DateTime<Utc>,
}

/// Durable job summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSummary {
    /// Job identity.
    pub id: JobId,
    /// Owning build run, absent for non-build work.
    pub run_id: Option<RunId>,
    /// Owning catalog scan, absent for build work.
    pub recipe_catalog_scan_id: Option<RecipeCatalogScanId>,
    /// Scheduling requirements.
    pub required_capabilities: Vec<String>,
    /// Durable state.
    pub state: String,
    /// Maximum attempts.
    pub maximum_attempts: u32,
    /// Attempt count.
    pub attempt_count: u32,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

/// Durable job details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    /// Job identity.
    pub id: JobId,
    /// Owning build run, absent for non-build work.
    pub run_id: Option<RunId>,
    /// Owning catalog scan, absent for build work.
    pub recipe_catalog_scan_id: Option<RecipeCatalogScanId>,
    /// Scheduling requirements.
    pub required_capabilities: Vec<String>,
    /// Builder-neutral payload.
    pub payload: serde_json::Value,
    /// Durable state.
    pub state: String,
    /// Maximum attempts.
    pub maximum_attempts: u32,
    /// Attempt count.
    pub attempt_count: u32,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

/// Immutable software release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    /// Publication eligibility independent of attained lifecycle.
    #[serde(default)]
    pub availability: ReleaseAvailability,
    /// Release identity.
    pub id: ReleaseId,
    /// Parent software.
    pub software_id: SoftwareId,
    /// Opaque upstream version.
    pub version: String,
    /// Highest lifecycle state achieved.
    pub state: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Optimistic-concurrency revision.
    pub revision: u64,
}

/// Artifact attached to a variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantArtifact {
    /// Content digest.
    pub digest: Sha256Digest,
    /// Exact size.
    pub size: u64,
    /// Media type.
    pub media_type: String,
    /// Semantic role.
    pub role: String,
    /// Whether a readable present location exists.
    pub readable: bool,
}

/// Installable release variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variant {
    /// Variant identity.
    pub id: VariantId,
    /// Parent release.
    pub release_id: ReleaseId,
    /// Platform.
    pub platform: String,
    /// Architecture.
    pub architecture: String,
    /// Inclusive minimum macOS version.
    pub minimum_macos: Option<String>,
    /// Inclusive maximum macOS version.
    pub maximum_macos: Option<String>,
    /// Resolver priority.
    pub resolution_priority: i32,
    /// Attached immutable artifacts.
    pub artifacts: Vec<VariantArtifact>,
}

/// Mutable software channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    /// Software identity.
    pub software_id: SoftwareId,
    /// Channel name.
    pub name: String,
    /// Current release target.
    pub release_id: ReleaseId,
    /// Optional variant pin.
    pub pinned_variant_id: Option<VariantId>,
    /// Optimistic-concurrency revision.
    pub revision: u64,
}

/// Resolver result with exactly one primary installer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    /// Selected channel.
    pub channel: String,
    /// Selected release.
    pub release: Release,
    /// Selected variant.
    pub variant: Variant,
    /// Primary installer digest.
    pub artifact_digest: Sha256Digest,
    /// Primary installer size.
    pub artifact_size: u64,
    /// Authenticated content path.
    pub content_path: String,
}

/// Configured artifact store.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Store {
    /// Store identity.
    pub id: StoreId,
    /// Operator-facing name.
    pub name: String,
    /// Placement role.
    pub role: String,
    /// Adapter kind.
    pub kind: String,
    /// Whether serving and placement may use it.
    pub enabled: bool,
    /// Optimistic-concurrency revision.
    pub revision: u64,
    /// Full reads supported.
    pub read: bool,
    /// Streaming writes supported.
    pub write: bool,
    /// Deletion supported.
    pub delete: bool,
    /// Byte ranges supported.
    pub range: bool,
    /// Multipart supported.
    pub multipart: bool,
    /// Redirect/presign supported.
    pub redirect: bool,
}

/// Non-mutating store probe result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreTest {
    /// Whether the adapter responded as expected.
    pub healthy: bool,
    /// Safe result detail.
    pub detail: String,
}

/// Administrative worker metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worker {
    /// New claims are paused while current attempts may finish.
    #[serde(default)]
    pub draining: bool,
    /// Worker identity.
    pub id: WorkerId,
    /// Operator-facing name.
    pub name: String,
    /// Server-enforced capability ceiling.
    pub allowed_capabilities: Vec<String>,
    /// Latest worker advertisement.
    pub advertised_capabilities: Vec<String>,
    /// Whether authentication and leasing are enabled.
    pub enabled: bool,
    /// Latest registration activity.
    pub last_seen_at: DateTime<Utc>,
    /// Optimistic-concurrency revision.
    pub revision: u64,
}

/// One-time provisioned worker credential.
#[derive(Clone, Deserialize)]
pub struct WorkerCredential {
    /// Server-assigned worker identity.
    pub worker_id: WorkerId,
    /// One-time bearer token.
    pub token: SecretToken,
}

impl fmt::Debug for WorkerCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerCredential")
            .field("worker_id", &self.worker_id)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

/// One-time rotated worker credential.
#[derive(Clone, Deserialize)]
pub struct RotatedWorkerCredential {
    /// Updated worker metadata.
    pub worker: Worker,
    /// New one-time bearer token.
    pub token: SecretToken,
}

impl fmt::Debug for RotatedWorkerCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RotatedWorkerCredential")
            .field("worker", &self.worker)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

/// Public health response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    /// Health status.
    pub status: String,
    /// Server version.
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::{BaseUrl, Credentials, SecretToken, Sha256Digest, SoftwareId};

    #[test]
    fn base_url_rejects_escape_surfaces() {
        for value in [
            "https://user:secret@example.invalid",
            "https://example.invalid/prefix",
            "https://example.invalid?token=secret",
            "file:///tmp/stabbur",
        ] {
            assert!(BaseUrl::parse(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn secrets_are_redacted() {
        let token = SecretToken::new("top-secret").unwrap();
        let credentials = Credentials::new("operator", "top-secret");
        assert!(!format!("{token:?}").contains("top-secret"));
        assert!(!format!("{credentials:?}").contains("top-secret"));
    }

    #[test]
    fn typed_values_are_strict() {
        assert!(Sha256Digest::parse(&"a".repeat(64)).is_ok());
        assert!(Sha256Digest::parse(&"A".repeat(64)).is_err());
        assert!(SoftwareId::parse("00000000-0000-4000-8000-000000000000").is_err());
    }
}

/// Publication eligibility of an immutable release.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReleaseAvailability {
    /// Publication remains permitted by the ordinary lifecycle rules.
    #[default]
    Available,
    /// Publication has been explicitly withdrawn.
    Withdrawn {
        /// Audited operator explanation.
        reason: String,
        /// Withdrawal timestamp.
        at: DateTime<Utc>,
    },
}
/// Published channel and its exact release version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoftwareChannelStatus {
    /// Channel name.
    pub name: String,
    /// Exact release identity.
    pub release_id: ReleaseId,
    /// Opaque version.
    pub version: String,
    /// Channel concurrency revision.
    pub revision: u64,
}
/// An enabled target without one recently observed matching worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedTarget {
    /// Target identity.
    pub id: BuildTargetId,
    /// Operator-facing name.
    pub name: String,
    /// Requirements that must match on one worker.
    pub required_capabilities: Vec<String>,
}
/// Software execution and publication overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoftwareStatus {
    /// Owning software identity.
    pub software_id: SoftwareId,
    /// Current published channels.
    pub channels: Vec<SoftwareChannelStatus>,
    /// Most recent run.
    pub latest_run: Option<RunSummary>,
    /// Latest successful build/check.
    pub last_success_at: Option<DateTime<Utc>>,
    /// Next scheduled check.
    pub next_run_at: Option<DateTime<Utc>>,
    /// Enabled target count.
    pub enabled_targets: u64,
    /// Queued and running build count.
    pub outstanding_runs: u64,
    /// First 200 targets without a matching worker observed within five minutes.
    pub blocked_targets: Vec<BlockedTarget>,
}
/// Durable queue and worker measurements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationalStatus {
    /// Queued jobs.
    pub queued_jobs: u64,
    /// Leased jobs.
    pub running_jobs: u64,
    /// Historical failed jobs.
    pub failed_jobs: u64,
    /// Historical expired attempts.
    pub expired_attempts: u64,
    /// Enabled draining workers.
    pub draining_workers: u64,
    /// Oldest queued job creation time.
    pub oldest_queued_at: Option<DateTime<Utc>>,
}
