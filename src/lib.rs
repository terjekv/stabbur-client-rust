#![forbid(unsafe_code)]
#![cfg_attr(
    not(any(feature = "async", feature = "blocking")),
    allow(dead_code, unused_imports)
)]

//! Supported Rust client for the Stabbur HTTP API.
//!
//! Authentication state is enforced by the type system:
//!
//! ```compile_fail
//! use stabbur_client::Client;
//!
//! let public = Client::from_url("https://stabbur.example.net")?;
//! let _principal = public.me(); // available only after login or token attachment
//! # Ok::<(), stabbur_client::ApiError>(())
//! ```

mod endpoints;
mod events;
pub use events::{RunCompletion, RunEvent, RunEventDecoder, TerminalRunState};
mod error;
mod raw;
mod types;

/// Versioned catalog manifests and deterministic reconciliation plans.
pub mod catalog;

/// Asynchronous and blocking typestate clients.
pub mod client;
pub mod resources;

/// Stabbur server release targeted by this client release.
pub const TARGET_SERVER_VERSION: &str = "0.1.0";

pub use catalog::{
    CATALOG_SCHEMA_VERSION, CatalogAction, CatalogManifest, CatalogPlan, CatalogRecipe,
    CatalogSoftware, CatalogSyncReport,
};
#[cfg(feature = "async")]
pub use client::r#async::Client;
#[cfg(feature = "blocking")]
pub use client::blocking;
pub use client::{Authenticated, Unauthenticated};
pub use error::{ApiError, Problem, ValidationError};
pub use raw::{RawMethod, RawRequest, RawResponse};
pub use types::{
    ApiToken, Artifact, ArtifactContentHead, ArtifactLocation, ArtifactLocationId, ArtifactUpload,
    AuditEvent, AuditPage, AutoPkgArtifactSelector, AutoPkgOutputSelectors, AutoPkgVariantSelector,
    AutoPkgVerificationSelector, BaseUrl, BuildTarget, BuildTargetId, BuildTargetSchedule,
    BuildTargetUpdate, Channel, CreatedApiToken, Credentials, CursorPage, Health,
    InstallationMetadata, Job, JobId, JobSummary, NewAutoPkgRevision, NewRecipeRevision,
    PinnedSource, Principal, PrincipalAdmin, PrincipalId, Recipe, RecipeCatalogDiagnostic,
    RecipeCatalogEntry, RecipeCatalogLookup, RecipeCatalogManifest, RecipeCatalogMatch,
    RecipeCatalogScan, RecipeCatalogScanFailure, RecipeCatalogScanId, RecipeCatalogSnapshot,
    RecipeCatalogSnapshotId, RecipeCatalogSnapshotSummary, RecipeCatalogSource, RecipeId,
    RecipeRevision, RecipeRevisionId, Release, ReleaseId, Resolution, Role,
    RotatedWorkerCredential, Run, RunId, RunLog, RunSummary, SecretToken, Sha256Digest, Software,
    SoftwareId, Store, StoreId, StoreTest, Variant, VariantArtifact, VariantId, Worker,
    WorkerCredential, WorkerId,
};

pub use types::{
    BlockedTarget, OperationalStatus, ReleaseAvailability, SoftwareChannelStatus, SoftwareStatus,
};

pub use catalog::{CatalogTarget, PlannedRecipeRevision, ValidatedCatalogManifest};

pub use catalog::{SourcePinChange, SourceUpdateProposal, ValidatedSourcePin};
