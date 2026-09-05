//! Centralized safe endpoint construction.

pub(crate) const BOOTSTRAP: &str = "/api/v1/auth/bootstrap";
pub(crate) const LOGIN: &str = "/api/v1/auth/login";
pub(crate) const ME: &str = "/api/v1/auth/me";
pub(crate) const PASSWORD: &str = "/api/v1/auth/password";
pub(crate) const PRINCIPALS: &str = "/api/v1/auth/principals";
pub(crate) const ROLES: &str = "/api/v1/auth/roles";
pub(crate) const SOFTWARE: &str = "/api/v1/software";
pub(crate) const RECIPES: &str = "/api/v1/recipes";
pub(crate) const RECIPE_CATALOGS: &str = "/api/v1/recipe-catalogs";
pub(crate) const RECIPE_CATALOG_ENTRIES: &str = "/api/v1/recipe-catalog-entries";
pub(crate) const RECIPE_CATALOG_SCANS: &str = "/api/v1/recipe-catalog-scans";
pub(crate) const BUILD_TARGETS: &str = "/api/v1/build-targets";
pub(crate) const RUNS: &str = "/api/v1/runs";
pub(crate) const RELEASES: &str = "/api/v1/releases";
pub(crate) const ARTIFACTS: &str = "/api/v1/artifacts";
pub(crate) const STORES: &str = "/api/v1/stores";
pub(crate) const WORKERS: &str = "/api/v1/workers";
pub(crate) const JOBS: &str = "/api/v1/jobs";
pub(crate) const AUDIT: &str = "/api/v1/audit";
pub(crate) const OPENAPI: &str = "/api/v1/openapi.json";
pub(crate) const HEALTH: &str = "/healthz";

pub(crate) fn principal(identity: &str) -> String {
    child(PRINCIPALS, identity)
}
pub(crate) fn principal_password(identity: &str) -> String {
    nested(PRINCIPALS, identity, "password")
}
pub(crate) fn principal_sessions(identity: &str) -> String {
    nested(PRINCIPALS, identity, "revoke-sessions")
}
pub(crate) fn principal_tokens(identity: &str) -> String {
    nested(PRINCIPALS, identity, "tokens")
}
pub(crate) fn token(identity: &str) -> String {
    child("/api/v1/auth/tokens", identity)
}
pub(crate) fn software(identity: &str) -> String {
    child(SOFTWARE, identity)
}
pub(crate) fn software_releases(identity: &str) -> String {
    nested(SOFTWARE, identity, "releases")
}
pub(crate) fn software_channels(identity: &str) -> String {
    nested(SOFTWARE, identity, "channels")
}
pub(crate) fn channel(identity: &str, name: &str) -> String {
    format!(
        "{}/{}/channels/{}",
        SOFTWARE,
        encode_segment(identity),
        encode_segment(name)
    )
}
pub(crate) fn resolve(identity: &str) -> String {
    nested(SOFTWARE, identity, "resolve")
}
pub(crate) fn release(identity: &str) -> String {
    child(RELEASES, identity)
}
pub(crate) fn release_variants(identity: &str) -> String {
    nested(RELEASES, identity, "variants")
}
pub(crate) fn reject_release(identity: &str) -> String {
    nested(RELEASES, identity, "reject")
}
pub(crate) fn recipe(identity: &str) -> String {
    child(RECIPES, identity)
}
pub(crate) fn recipe_revisions(identity: &str) -> String {
    nested(RECIPES, identity, "revisions")
}
pub(crate) fn recipe_runs(identity: &str) -> String {
    nested(RECIPES, identity, "runs")
}
pub(crate) fn recipe_catalog_snapshot(identity: &str) -> String {
    child(RECIPE_CATALOGS, identity)
}
pub(crate) fn recipe_catalog_scan(identity: &str) -> String {
    child(RECIPE_CATALOG_SCANS, identity)
}
pub(crate) fn recipe_catalog_scan_cancel(identity: &str) -> String {
    nested(RECIPE_CATALOG_SCANS, identity, "cancel")
}
pub(crate) fn build_target(identity: &str) -> String {
    child(BUILD_TARGETS, identity)
}
pub(crate) fn build_target_runs(identity: &str) -> String {
    nested(BUILD_TARGETS, identity, "runs")
}
pub(crate) fn run(identity: &str) -> String {
    child(RUNS, identity)
}
pub(crate) fn run_cancel(identity: &str) -> String {
    nested(RUNS, identity, "cancel")
}
pub(crate) fn run_logs(identity: &str) -> String {
    nested(RUNS, identity, "logs")
}
pub(crate) fn run_events(identity: &str) -> String {
    nested(RUNS, identity, "events")
}
pub(crate) fn artifact(digest: &str) -> String {
    child(ARTIFACTS, digest)
}
pub(crate) fn artifact_content(digest: &str) -> String {
    nested(ARTIFACTS, digest, "content")
}
pub(crate) fn artifact_locations(digest: &str) -> String {
    nested(ARTIFACTS, digest, "locations")
}
pub(crate) fn store(identity: &str) -> String {
    child(STORES, identity)
}
pub(crate) fn store_test(identity: &str) -> String {
    nested(STORES, identity, "test")
}
pub(crate) fn worker(identity: &str) -> String {
    child(WORKERS, identity)
}
pub(crate) fn worker_rotate(identity: &str) -> String {
    nested(WORKERS, identity, "rotate-token")
}
pub(crate) fn job(identity: &str) -> String {
    child(JOBS, identity)
}

fn child(root: &str, value: &str) -> String {
    format!("{root}/{}", encode_segment(value))
}
fn nested(root: &str, value: &str, tail: &str) -> String {
    format!("{root}/{}/{tail}", encode_segment(value))
}

fn encode_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02X}").expect("writing to a String is infallible");
        }
    }
    encoded
}

pub(crate) const READY: &str = "/readyz";
pub(crate) const OPERATIONS: &str = "/api/v1/operations/status";
pub(crate) fn software_status(identity: &str) -> String {
    format!("{}/status", software(identity))
}
pub(crate) fn withdraw_release(identity: &str) -> String {
    format!("{}/withdraw", release(identity))
}
pub(crate) fn drain_worker(identity: &str) -> String {
    format!("{}/drain", worker(identity))
}

#[cfg(test)]
mod tests {
    use super::{channel, software};

    #[test]
    fn dynamic_values_are_one_opaque_segment() {
        assert_eq!(
            software("../admin/token?raw=true"),
            "/api/v1/software/..%2Fadmin%2Ftoken%3Fraw%3Dtrue"
        );
        assert_eq!(
            channel("safe", "stable/../../tokens"),
            "/api/v1/software/safe/channels/stable%2F..%2F..%2Ftokens"
        );
    }
}
