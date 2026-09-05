//! Generated operation constants. Do not edit by hand.

/// One released HTTP operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operation {
    /// HTTP method.
    pub method: &'static str,
    /// Safe endpoint template.
    pub path: &'static str,
}

/// `bootstrap`.
pub const BOOTSTRAP: Operation = Operation {
    method: "POST",
    path: "/api/v1/auth/bootstrap",
};
/// `cancel_recipe_catalog_scan`.
pub const CANCEL_RECIPE_CATALOG_SCAN: Operation = Operation {
    method: "POST",
    path: "/api/v1/recipe-catalog-scans/{scan}/cancel",
};
/// `cancel_run`.
pub const CANCEL_RUN: Operation = Operation {
    method: "POST",
    path: "/api/v1/runs/{run}/cancel",
};
/// `change_password`.
pub const CHANGE_PASSWORD: Operation = Operation {
    method: "POST",
    path: "/api/v1/auth/password",
};
/// `create_api_token`.
pub const CREATE_API_TOKEN: Operation = Operation {
    method: "POST",
    path: "/api/v1/auth/principals/{principal}/tokens",
};
/// `create_build_target`.
pub const CREATE_BUILD_TARGET: Operation = Operation {
    method: "POST",
    path: "/api/v1/build-targets",
};
/// `create_principal`.
pub const CREATE_PRINCIPAL: Operation = Operation {
    method: "POST",
    path: "/api/v1/auth/principals",
};
/// `create_recipe`.
pub const CREATE_RECIPE: Operation = Operation {
    method: "POST",
    path: "/api/v1/recipes",
};
/// `create_recipe_catalog_scan`.
pub const CREATE_RECIPE_CATALOG_SCAN: Operation = Operation {
    method: "POST",
    path: "/api/v1/recipe-catalog-scans",
};
/// `create_recipe_revision`.
pub const CREATE_RECIPE_REVISION: Operation = Operation {
    method: "POST",
    path: "/api/v1/recipes/{recipe}/revisions",
};
/// `create_role`.
pub const CREATE_ROLE: Operation = Operation {
    method: "POST",
    path: "/api/v1/auth/roles",
};
/// `create_run`.
pub const CREATE_RUN: Operation = Operation {
    method: "POST",
    path: "/api/v1/runs",
};
/// `create_software`.
pub const CREATE_SOFTWARE: Operation = Operation {
    method: "POST",
    path: "/api/v1/software",
};
/// `download_artifact_content`.
pub const DOWNLOAD_ARTIFACT_CONTENT: Operation = Operation {
    method: "GET",
    path: "/api/v1/artifacts/{digest}/content",
};
/// `drain_worker`.
pub const DRAIN_WORKER: Operation = Operation {
    method: "POST",
    path: "/api/v1/workers/{worker}/drain",
};
/// `get_artifact`.
pub const GET_ARTIFACT: Operation = Operation {
    method: "GET",
    path: "/api/v1/artifacts/{digest}",
};
/// `get_build_target`.
pub const GET_BUILD_TARGET: Operation = Operation {
    method: "GET",
    path: "/api/v1/build-targets/{target}",
};
/// `get_channel`.
pub const GET_CHANNEL: Operation = Operation {
    method: "GET",
    path: "/api/v1/software/{software}/channels/{channel}",
};
/// `get_job`.
pub const GET_JOB: Operation = Operation {
    method: "GET",
    path: "/api/v1/jobs/{job}",
};
/// `get_principal`.
pub const GET_PRINCIPAL: Operation = Operation {
    method: "GET",
    path: "/api/v1/auth/principals/{principal}",
};
/// `get_recipe`.
pub const GET_RECIPE: Operation = Operation {
    method: "GET",
    path: "/api/v1/recipes/{recipe}",
};
/// `get_recipe_catalog_scan`.
pub const GET_RECIPE_CATALOG_SCAN: Operation = Operation {
    method: "GET",
    path: "/api/v1/recipe-catalog-scans/{scan}",
};
/// `get_recipe_catalog_snapshot`.
pub const GET_RECIPE_CATALOG_SNAPSHOT: Operation = Operation {
    method: "GET",
    path: "/api/v1/recipe-catalogs/{snapshot}",
};
/// `get_release`.
pub const GET_RELEASE: Operation = Operation {
    method: "GET",
    path: "/api/v1/releases/{release}",
};
/// `get_run`.
pub const GET_RUN: Operation = Operation {
    method: "GET",
    path: "/api/v1/runs/{run}",
};
/// `get_software`.
pub const GET_SOFTWARE: Operation = Operation {
    method: "GET",
    path: "/api/v1/software/{software}",
};
/// `get_store`.
pub const GET_STORE: Operation = Operation {
    method: "GET",
    path: "/api/v1/stores/{store}",
};
/// `get_worker`.
pub const GET_WORKER: Operation = Operation {
    method: "GET",
    path: "/api/v1/workers/{worker}",
};
/// `head_artifact_content`.
pub const HEAD_ARTIFACT_CONTENT: Operation = Operation {
    method: "HEAD",
    path: "/api/v1/artifacts/{digest}/content",
};
/// `healthz`.
pub const HEALTHZ: Operation = Operation {
    method: "GET",
    path: "/healthz",
};
/// `list_api_tokens`.
pub const LIST_API_TOKENS: Operation = Operation {
    method: "GET",
    path: "/api/v1/auth/principals/{principal}/tokens",
};
/// `list_artifact_locations`.
pub const LIST_ARTIFACT_LOCATIONS: Operation = Operation {
    method: "GET",
    path: "/api/v1/artifacts/{digest}/locations",
};
/// `list_audit_events`.
pub const LIST_AUDIT_EVENTS: Operation = Operation {
    method: "GET",
    path: "/api/v1/audit",
};
/// `list_build_targets`.
pub const LIST_BUILD_TARGETS: Operation = Operation {
    method: "GET",
    path: "/api/v1/build-targets",
};
/// `list_build_target_runs`.
pub const LIST_BUILD_TARGET_RUNS: Operation = Operation {
    method: "GET",
    path: "/api/v1/build-targets/{target}/runs",
};
/// `list_channels`.
pub const LIST_CHANNELS: Operation = Operation {
    method: "GET",
    path: "/api/v1/software/{software}/channels",
};
/// `list_jobs`.
pub const LIST_JOBS: Operation = Operation {
    method: "GET",
    path: "/api/v1/jobs",
};
/// `list_principals`.
pub const LIST_PRINCIPALS: Operation = Operation {
    method: "GET",
    path: "/api/v1/auth/principals",
};
/// `list_recipes`.
pub const LIST_RECIPES: Operation = Operation {
    method: "GET",
    path: "/api/v1/recipes",
};
/// `list_recipe_catalog_scans`.
pub const LIST_RECIPE_CATALOG_SCANS: Operation = Operation {
    method: "GET",
    path: "/api/v1/recipe-catalog-scans",
};
/// `list_recipe_catalog_snapshots`.
pub const LIST_RECIPE_CATALOG_SNAPSHOTS: Operation = Operation {
    method: "GET",
    path: "/api/v1/recipe-catalogs",
};
/// `list_recipe_revisions`.
pub const LIST_RECIPE_REVISIONS: Operation = Operation {
    method: "GET",
    path: "/api/v1/recipes/{recipe}/revisions",
};
/// `list_recipe_runs`.
pub const LIST_RECIPE_RUNS: Operation = Operation {
    method: "GET",
    path: "/api/v1/recipes/{recipe}/runs",
};
/// `list_releases`.
pub const LIST_RELEASES: Operation = Operation {
    method: "GET",
    path: "/api/v1/software/{software}/releases",
};
/// `list_release_variants`.
pub const LIST_RELEASE_VARIANTS: Operation = Operation {
    method: "GET",
    path: "/api/v1/releases/{release}/variants",
};
/// `list_roles`.
pub const LIST_ROLES: Operation = Operation {
    method: "GET",
    path: "/api/v1/auth/roles",
};
/// `list_runs`.
pub const LIST_RUNS: Operation = Operation {
    method: "GET",
    path: "/api/v1/runs",
};
/// `list_run_logs`.
pub const LIST_RUN_LOGS: Operation = Operation {
    method: "GET",
    path: "/api/v1/runs/{run}/logs",
};
/// `list_software`.
pub const LIST_SOFTWARE: Operation = Operation {
    method: "GET",
    path: "/api/v1/software",
};
/// `list_stores`.
pub const LIST_STORES: Operation = Operation {
    method: "GET",
    path: "/api/v1/stores",
};
/// `list_workers`.
pub const LIST_WORKERS: Operation = Operation {
    method: "GET",
    path: "/api/v1/workers",
};
/// `login`.
pub const LOGIN: Operation = Operation {
    method: "POST",
    path: "/api/v1/auth/login",
};
/// `me`.
pub const ME: Operation = Operation {
    method: "GET",
    path: "/api/v1/auth/me",
};
/// `openapi_document`.
pub const OPENAPI_DOCUMENT: Operation = Operation {
    method: "GET",
    path: "/api/v1/openapi.json",
};
/// `operational_status`.
pub const OPERATIONAL_STATUS: Operation = Operation {
    method: "GET",
    path: "/api/v1/operations/status",
};
/// `promote_channel`.
pub const PROMOTE_CHANNEL: Operation = Operation {
    method: "PUT",
    path: "/api/v1/software/{software}/channels/{channel}",
};
/// `provision_worker`.
pub const PROVISION_WORKER: Operation = Operation {
    method: "POST",
    path: "/api/v1/workers",
};
/// `readyz`.
pub const READYZ: Operation = Operation {
    method: "GET",
    path: "/readyz",
};
/// `reject_release`.
pub const REJECT_RELEASE: Operation = Operation {
    method: "POST",
    path: "/api/v1/releases/{release}/reject",
};
/// `reset_principal_password`.
pub const RESET_PRINCIPAL_PASSWORD: Operation = Operation {
    method: "POST",
    path: "/api/v1/auth/principals/{principal}/password",
};
/// `resolve_recipe_catalog_entry`.
pub const RESOLVE_RECIPE_CATALOG_ENTRY: Operation = Operation {
    method: "GET",
    path: "/api/v1/recipe-catalog-entries",
};
/// `resolve_software`.
pub const RESOLVE_SOFTWARE: Operation = Operation {
    method: "GET",
    path: "/api/v1/software/{software}/resolve",
};
/// `revoke_api_token`.
pub const REVOKE_API_TOKEN: Operation = Operation {
    method: "DELETE",
    path: "/api/v1/auth/tokens/{token}",
};
/// `revoke_principal_sessions`.
pub const REVOKE_PRINCIPAL_SESSIONS: Operation = Operation {
    method: "POST",
    path: "/api/v1/auth/principals/{principal}/revoke-sessions",
};
/// `rotate_worker_token`.
pub const ROTATE_WORKER_TOKEN: Operation = Operation {
    method: "POST",
    path: "/api/v1/workers/{worker}/rotate-token",
};
/// `software_status`.
pub const SOFTWARE_STATUS: Operation = Operation {
    method: "GET",
    path: "/api/v1/software/{software}/status",
};
/// `stream_run_events`.
pub const STREAM_RUN_EVENTS: Operation = Operation {
    method: "GET",
    path: "/api/v1/runs/{run}/events",
};
/// `test_store`.
pub const TEST_STORE: Operation = Operation {
    method: "POST",
    path: "/api/v1/stores/{store}/test",
};
/// `trigger_build_target`.
pub const TRIGGER_BUILD_TARGET: Operation = Operation {
    method: "POST",
    path: "/api/v1/build-targets/{target}/runs",
};
/// `update_build_target`.
pub const UPDATE_BUILD_TARGET: Operation = Operation {
    method: "PATCH",
    path: "/api/v1/build-targets/{target}",
};
/// `update_principal`.
pub const UPDATE_PRINCIPAL: Operation = Operation {
    method: "PATCH",
    path: "/api/v1/auth/principals/{principal}",
};
/// `update_software`.
pub const UPDATE_SOFTWARE: Operation = Operation {
    method: "PATCH",
    path: "/api/v1/software/{software}",
};
/// `update_worker`.
pub const UPDATE_WORKER: Operation = Operation {
    method: "PATCH",
    path: "/api/v1/workers/{worker}",
};
/// `upload_artifact_content`.
pub const UPLOAD_ARTIFACT_CONTENT: Operation = Operation {
    method: "PUT",
    path: "/api/v1/artifacts/{digest}/content",
};
/// `withdraw_release`.
pub const WITHDRAW_RELEASE: Operation = Operation {
    method: "POST",
    path: "/api/v1/releases/{release}/withdraw",
};
