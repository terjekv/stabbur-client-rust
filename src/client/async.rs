use std::{pin::Pin, time::Duration};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures_core::Stream;
use futures_util::StreamExt;
use reqwest::{Method, Response, StatusCode, header};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::{
    Authenticated, Unauthenticated, validate_autopkg_revision, validate_page,
    validate_recipe_revision,
};
use crate::{
    ApiError, ApiToken, Artifact, ArtifactContentHead, ArtifactLocation, ArtifactUpload, AuditPage,
    BaseUrl, BuildTarget, BuildTargetSchedule, BuildTargetUpdate, CatalogAction, CatalogManifest,
    CatalogPlan, CatalogSyncReport, Channel, CreatedApiToken, Credentials, CursorPage, Health,
    InstallationMetadata, Job, JobId, JobSummary, NewAutoPkgRevision, NewRecipeRevision,
    PinnedSource, Principal, PrincipalAdmin, Problem, RawMethod, RawRequest, RawResponse, Recipe,
    RecipeCatalogLookup, RecipeCatalogScan, RecipeCatalogScanId, RecipeCatalogSnapshot,
    RecipeCatalogSnapshotId, RecipeCatalogSnapshotSummary, RecipeRevision, RecipeRevisionId,
    Release, ReleaseId, Resolution, Role, RotatedWorkerCredential, Run, RunId, RunLog, RunSummary,
    SecretToken, Sha256Digest, Software, Store, StoreId, StoreTest, Variant, VariantId, Worker,
    WorkerCredential, WorkerId,
    catalog::build_plan,
    endpoints,
    raw::{MAX_RAW_REQUEST_BYTES, MAX_RAW_RESPONSE_BYTES},
};

/// Asynchronous typestate Stabbur client.
#[derive(Clone)]
pub struct Client<S> {
    base_url: BaseUrl,
    http: reqwest::Client,
    state: S,
}

impl Client<Unauthenticated> {
    /// Builds an unauthenticated client for a safe server origin.
    pub fn from_url(value: &str) -> Result<Self, ApiError> {
        let base_url = BaseUrl::parse(value)?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("stabbur-client/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| ApiError::Transport)?;
        Ok(Self {
            base_url,
            http,
            state: Unauthenticated,
        })
    }

    /// Returns the unauthenticated server health response.
    pub async fn health(&self) -> Result<Health, ApiError> {
        decode(
            self.http
                .get(self.base_url.endpoint(endpoints::HEALTH))
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }

    /// Returns the released OpenAPI document.
    pub async fn openapi_document(&self) -> Result<serde_json::Value, ApiError> {
        decode(
            self.http
                .get(self.base_url.endpoint(endpoints::OPENAPI))
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }

    /// Performs the one-time first-administrator bootstrap.
    pub async fn bootstrap(
        &self,
        secret: &SecretToken,
        credentials: &Credentials,
    ) -> Result<Principal, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            secret: &'a str,
            username: &'a str,
            password: &'a str,
        }
        decode(
            self.http
                .post(self.base_url.endpoint(endpoints::BOOTSTRAP))
                .json(&Body {
                    secret: secret.expose_secret(),
                    username: credentials.username(),
                    password: credentials.password(),
                })
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }

    /// Exchanges interactive human credentials for a short-lived session.
    pub async fn login(
        &self,
        credentials: &Credentials,
    ) -> Result<Client<Authenticated>, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            username: &'a str,
            password: &'a str,
        }
        #[derive(Deserialize)]
        struct Login {
            token: String,
            #[serde(rename = "expires_at")]
            expires_at: DateTime<Utc>,
        }
        let response = self
            .http
            .post(self.base_url.endpoint(endpoints::LOGIN))
            .json(&Body {
                username: credentials.username(),
                password: credentials.password(),
            })
            .send()
            .await
            .map_err(|_| ApiError::Transport)?;
        let login: Login = decode(response).await?;
        Ok(self.clone_with_state(Authenticated::with_expiry(
            SecretToken::new(login.token)?,
            login.expires_at,
        )))
    }

    /// Attaches a token obtained from a protected source.
    #[must_use]
    pub fn authenticate(&self, token: SecretToken) -> Client<Authenticated> {
        self.clone_with_state(Authenticated::new(token))
    }

    fn clone_with_state<T>(&self, state: T) -> Client<T> {
        Client {
            base_url: self.base_url.clone(),
            http: self.http.clone(),
            state,
        }
    }
}

impl Client<Authenticated> {
    /// Expiration reported at login; absent for externally attached tokens.
    pub fn session_expires_at(&self) -> Option<DateTime<Utc>> {
        self.state.expires_at
    }

    /// Returns the authenticated principal.
    pub async fn me(&self) -> Result<Principal, ApiError> {
        self.get(endpoints::ME).await
    }

    /// Changes the current human principal's password and revokes its sessions.
    pub async fn change_password(&self, current: &str, replacement: &str) -> Result<(), ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            current_password: &'a str,
            new_password: &'a str,
        }
        self.empty_json(
            Method::POST,
            endpoints::PASSWORD,
            &Body {
                current_password: current,
                new_password: replacement,
            },
            None,
        )
        .await
    }

    /// Returns the redacted secret wrapper for protected credential persistence.
    #[must_use]
    pub fn token(&self) -> &SecretToken {
        &self.state.token
    }

    /// Principal, role, and API-token administration.
    #[must_use]
    pub fn identity(&self) -> IdentityResource {
        IdentityResource {
            client: self.clone(),
        }
    }
    /// Software, releases, variants, channels, and resolution.
    #[must_use]
    pub fn software(&self) -> SoftwareResource {
        SoftwareResource {
            client: self.clone(),
        }
    }
    /// Recipe metadata and immutable revisions.
    #[must_use]
    pub fn recipes(&self) -> RecipeResource {
        RecipeResource {
            client: self.clone(),
        }
    }
    /// Versioned catalog manifest planning and synchronization.
    #[must_use]
    pub fn catalog(&self) -> CatalogResource {
        CatalogResource {
            client: self.clone(),
        }
    }
    /// Desired manual and recurring build policy.
    #[must_use]
    pub fn build_targets(&self) -> BuildTargetResource {
        BuildTargetResource {
            client: self.clone(),
        }
    }
    /// Runs, ordered logs, and live events.
    #[must_use]
    pub fn runs(&self) -> RunResource {
        RunResource {
            client: self.clone(),
        }
    }
    /// Immutable artifact metadata and content.
    #[must_use]
    pub fn artifacts(&self) -> ArtifactResource {
        ArtifactResource {
            client: self.clone(),
        }
    }
    /// Artifact-store administration.
    #[must_use]
    pub fn stores(&self) -> StoreResource {
        StoreResource {
            client: self.clone(),
        }
    }
    /// Outbound worker administration.
    #[must_use]
    pub fn workers(&self) -> WorkerResource {
        WorkerResource {
            client: self.clone(),
        }
    }
    /// Durable builder-neutral jobs.
    #[must_use]
    pub fn jobs(&self) -> JobResource {
        JobResource {
            client: self.clone(),
        }
    }
    /// Append-only audit events.
    #[must_use]
    pub fn audit(&self) -> AuditResource {
        AuditResource {
            client: self.clone(),
        }
    }

    /// Sends a bounded request to a future public API operation.
    ///
    /// Paths are constructed only by [`RawRequest`], the internal worker protocol is forbidden,
    /// bearer authentication remains managed by the client, and response bodies are capped at
    /// 8 MiB. Prefer typed resource methods whenever the released client exposes the operation.
    pub async fn raw(&self, request: &RawRequest) -> Result<RawResponse, ApiError> {
        let body = request
            .body
            .as_ref()
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| ApiError::Decode)?;
        if body
            .as_ref()
            .is_some_and(|body| body.len() > MAX_RAW_REQUEST_BYTES)
        {
            return Err(ApiError::InvalidValue {
                kind: "raw request body",
                detail: "JSON body exceeds the 2 MiB limit",
            });
        }
        let mut builder = self.request(raw_method(request.method), &request.path);
        if !request.query.is_empty() {
            builder = builder.query(&request.query);
        }
        if let Some(body) = body {
            builder = builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(body);
        }
        if let Some(revision) = request.revision {
            builder = builder.header(header::IF_MATCH, revision_etag(revision));
        }
        if let Some(key) = &request.idempotency_key {
            builder = builder.header("idempotency-key", key);
        }
        decode_raw(builder.send().await.map_err(|_| ApiError::Transport)?).await
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, self.base_url.endpoint(path))
            .bearer_auth(self.state.token.expose_secret())
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        decode(
            self.request(Method::GET, path)
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }

    async fn json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &B,
        revision: Option<u64>,
    ) -> Result<T, ApiError> {
        let mut request = self.request(method, path).json(body);
        if let Some(revision) = revision {
            request = request.header(header::IF_MATCH, revision_etag(revision));
        }
        decode(request.send().await.map_err(|_| ApiError::Transport)?).await
    }

    async fn empty_json<B: Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        body: &B,
        revision: Option<u64>,
    ) -> Result<(), ApiError> {
        let mut request = self.request(method, path).json(body);
        if let Some(revision) = revision {
            request = request.header(header::IF_MATCH, revision_etag(revision));
        }
        decode_empty(request.send().await.map_err(|_| ApiError::Transport)?).await
    }
}

fn raw_method(method: RawMethod) -> Method {
    match method {
        RawMethod::Get => Method::GET,
        RawMethod::Head => Method::HEAD,
        RawMethod::Post => Method::POST,
        RawMethod::Put => Method::PUT,
        RawMethod::Patch => Method::PATCH,
        RawMethod::Delete => Method::DELETE,
    }
}

/// Async principal, token, and role operations.
#[derive(Clone)]
pub struct IdentityResource {
    client: Client<Authenticated>,
}

impl IdentityResource {
    /// Lists administrative principal metadata.
    pub async fn list_principals(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<PrincipalAdmin>, ApiError> {
        page(&self.client, endpoints::PRINCIPALS, cursor, limit).await
    }
    /// Loads a principal by UUIDv7 or exact name.
    pub async fn get_principal(&self, identity: &str) -> Result<PrincipalAdmin, ApiError> {
        self.client.get(&endpoints::principal(identity)).await
    }
    /// Creates a human principal. The password is never included in errors or Debug output.
    pub async fn create_human(
        &self,
        name: &str,
        password: &str,
        roles: &[String],
    ) -> Result<PrincipalAdmin, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            kind: &'static str,
            password: &'a str,
            roles: &'a [String],
        }
        self.client
            .json(
                Method::POST,
                endpoints::PRINCIPALS,
                &Body {
                    name,
                    kind: "human",
                    password,
                    roles,
                },
                None,
            )
            .await
    }
    /// Creates a service principal without a human password.
    pub async fn create_service(
        &self,
        name: &str,
        roles: &[String],
    ) -> Result<PrincipalAdmin, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            kind: &'static str,
            password: Option<&'a str>,
            roles: &'a [String],
        }
        self.client
            .json(
                Method::POST,
                endpoints::PRINCIPALS,
                &Body {
                    name,
                    kind: "service",
                    password: None,
                    roles,
                },
                None,
            )
            .await
    }
    /// Replaces a principal's role assignments using optimistic concurrency.
    pub async fn set_roles(
        &self,
        identity: &str,
        roles: &[String],
        revision: u64,
    ) -> Result<PrincipalAdmin, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            roles: &'a [String],
        }
        self.client
            .json(
                Method::PATCH,
                &endpoints::principal(identity),
                &Body { roles },
                Some(revision),
            )
            .await
    }
    /// Enables or disables a principal using optimistic concurrency.
    pub async fn set_enabled(
        &self,
        identity: &str,
        enabled: bool,
        revision: u64,
    ) -> Result<PrincipalAdmin, ApiError> {
        #[derive(Serialize)]
        struct Body {
            enabled: bool,
        }
        self.client
            .json(
                Method::PATCH,
                &endpoints::principal(identity),
                &Body { enabled },
                Some(revision),
            )
            .await
    }
    /// Administratively resets a human password.
    pub async fn reset_password(&self, identity: &str, password: &str) -> Result<(), ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            new_password: &'a str,
        }
        self.client
            .empty_json(
                Method::POST,
                &endpoints::principal_password(identity),
                &Body {
                    new_password: password,
                },
                None,
            )
            .await
    }
    /// Revokes all current sessions for a principal.
    pub async fn revoke_sessions(&self, identity: &str) -> Result<(), ApiError> {
        decode_empty(
            self.client
                .request(Method::POST, &endpoints::principal_sessions(identity))
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }
    /// Lists named long-lived API tokens for a principal.
    pub async fn list_tokens(&self, identity: &str) -> Result<Vec<ApiToken>, ApiError> {
        Ok(self
            .client
            .get::<Items<ApiToken>>(&endpoints::principal_tokens(identity))
            .await?
            .items)
    }
    /// Creates a named API token and returns its secret exactly once.
    pub async fn create_token(
        &self,
        identity: &str,
        name: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<CreatedApiToken, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            expires_at: Option<DateTime<Utc>>,
        }
        self.client
            .json(
                Method::POST,
                &endpoints::principal_tokens(identity),
                &Body { name, expires_at },
                None,
            )
            .await
    }
    /// Revokes a named API token idempotently.
    pub async fn revoke_token(&self, identity: &str) -> Result<ApiToken, ApiError> {
        decode(
            self.client
                .request(Method::DELETE, &endpoints::token(identity))
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }
    /// Lists built-in and custom roles.
    pub async fn list_roles(&self) -> Result<Vec<Role>, ApiError> {
        Ok(self
            .client
            .get::<Items<Role>>(endpoints::ROLES)
            .await?
            .items)
    }
    /// Creates a custom role.
    pub async fn create_role(&self, name: &str, permissions: &[String]) -> Result<Role, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            permissions: &'a [String],
        }
        self.client
            .json(
                Method::POST,
                endpoints::ROLES,
                &Body { name, permissions },
                None,
            )
            .await
    }
}

/// Asynchronous desired-state catalog reconciliation.
#[derive(Clone)]
pub struct CatalogResource {
    client: Client<Authenticated>,
}

impl CatalogResource {
    /// Lists immutable worker-observed catalog snapshot metadata.
    pub async fn snapshots(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<RecipeCatalogSnapshotSummary>, ApiError> {
        page(&self.client, endpoints::RECIPE_CATALOGS, cursor, limit).await
    }

    /// Loads one immutable catalog snapshot and its complete bounded manifest.
    pub async fn snapshot(
        &self,
        id: RecipeCatalogSnapshotId,
    ) -> Result<RecipeCatalogSnapshot, ApiError> {
        self.client
            .get(&endpoints::recipe_catalog_snapshot(&id.to_string()))
            .await
    }

    /// Resolves an exact recipe identifier in each latest producer/source observation.
    pub async fn resolve(&self, identifier: &str) -> Result<RecipeCatalogLookup, ApiError> {
        decode(
            self.client
                .request(Method::GET, endpoints::RECIPE_CATALOG_ENTRIES)
                .query(&[("identifier", identifier)])
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }

    /// Lists durable server-requested catalog scans.
    pub async fn scans(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<RecipeCatalogScan>, ApiError> {
        page(&self.client, endpoints::RECIPE_CATALOG_SCANS, cursor, limit).await
    }

    /// Queues one exact pinned AutoPkg repository scan.
    pub async fn request_autopkg_scan(
        &self,
        source: &PinnedSource,
        idempotency_key: &str,
    ) -> Result<RecipeCatalogScan, ApiError> {
        #[derive(Serialize)]
        struct Source<'a> {
            locator: &'a str,
            revision: &'a str,
        }
        #[derive(Serialize)]
        struct Body<'a> {
            producer: &'static str,
            source: Source<'a>,
        }
        super::validate_pinned_source(source)?;
        decode(
            self.client
                .request(Method::POST, endpoints::RECIPE_CATALOG_SCANS)
                .header("idempotency-key", idempotency_key)
                .json(&Body {
                    producer: "autopkg",
                    source: Source {
                        locator: &source.url,
                        revision: &source.commit,
                    },
                })
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }

    /// Loads one durable catalog scan.
    pub async fn scan(&self, id: RecipeCatalogScanId) -> Result<RecipeCatalogScan, ApiError> {
        self.client
            .get(&endpoints::recipe_catalog_scan(&id.to_string()))
            .await
    }

    /// Cancels queued or leased catalog scan work idempotently.
    pub async fn cancel_scan(
        &self,
        id: RecipeCatalogScanId,
        idempotency_key: &str,
    ) -> Result<RecipeCatalogScan, ApiError> {
        decode(
            self.client
                .request(
                    Method::POST,
                    &endpoints::recipe_catalog_scan_cancel(&id.to_string()),
                )
                .header("idempotency-key", idempotency_key)
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }

    super::catalog_ops::catalog_methods!(async, .await);

    async fn all_software(&self) -> Result<Vec<Software>, ApiError> {
        let mut values = Vec::new();
        let mut cursor = None;
        let mut traversal = super::CatalogTraversal::default();
        loop {
            let page = self.client.software().list(cursor.as_deref(), 200).await?;
            values.extend(page.items);
            let Some(next) = traversal.advance(page.next_cursor)? else {
                return Ok(values);
            };
            cursor = Some(next);
        }
    }

    async fn all_recipes(
        &self,
        manifest: &CatalogManifest,
    ) -> Result<Vec<(Recipe, Vec<RecipeRevision>)>, ApiError> {
        let mut current = Vec::new();
        let mut cursor = None;
        let mut traversal = super::CatalogTraversal::default();
        loop {
            let page = self.client.recipes().list(cursor.as_deref(), 200).await?;
            for recipe in page.items {
                if manifest
                    .recipes
                    .binary_search_by(|desired| desired.name.cmp(&recipe.name))
                    .is_ok()
                {
                    let revisions = self.client.recipes().revisions(&recipe.name).await?;
                    current.push((recipe, revisions));
                }
            }
            let Some(next) = traversal.advance(page.next_cursor)? else {
                return Ok(current);
            };
            cursor = Some(next);
        }
    }
}

/// Async desired build-target policy operations.
#[derive(Clone)]
pub struct BuildTargetResource {
    client: Client<Authenticated>,
}

impl BuildTargetResource {
    /// Lists one cursor page of desired build targets.
    pub async fn list(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<BuildTarget>, ApiError> {
        page(&self.client, endpoints::BUILD_TARGETS, cursor, limit).await
    }

    /// Loads a target by UUIDv7 or exact name.
    pub async fn get(&self, identity: &str) -> Result<BuildTarget, ApiError> {
        self.client.get(&endpoints::build_target(identity)).await
    }

    /// Creates desired manual or recurring build policy.
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        name: &str,
        software: &str,
        recipe_revision: RecipeRevisionId,
        parameters: &std::collections::BTreeMap<String, serde_json::Value>,
        schedule: BuildTargetSchedule,
        next_run_at: Option<&DateTime<Utc>>,
        enabled: bool,
    ) -> Result<BuildTarget, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            software: &'a str,
            recipe_revision: RecipeRevisionId,
            parameters: &'a std::collections::BTreeMap<String, serde_json::Value>,
            schedule: BuildTargetSchedule,
            next_run_at: Option<&'a DateTime<Utc>>,
            enabled: bool,
        }
        self.client
            .json(
                Method::POST,
                endpoints::BUILD_TARGETS,
                &Body {
                    name,
                    software,
                    recipe_revision,
                    parameters,
                    schedule,
                    next_run_at,
                    enabled,
                },
                None,
            )
            .await
    }

    /// Applies a partial replacement using optimistic concurrency.
    pub async fn update(
        &self,
        identity: &str,
        update: &BuildTargetUpdate,
        revision: u64,
    ) -> Result<BuildTarget, ApiError> {
        self.client
            .json(
                Method::PATCH,
                &endpoints::build_target(identity),
                update,
                Some(revision),
            )
            .await
    }

    /// Lists runs created from one desired target.
    pub async fn runs(
        &self,
        identity: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<RunSummary>, ApiError> {
        page(
            &self.client,
            &endpoints::build_target_runs(identity),
            cursor,
            limit,
        )
        .await
    }

    /// Queues one manual target run idempotently.
    pub async fn trigger(&self, identity: &str, idempotency_key: &str) -> Result<Run, ApiError> {
        decode(
            self.client
                .request(Method::POST, &endpoints::build_target_runs(identity))
                .header("idempotency-key", idempotency_key)
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }
}

/// Async software catalog and lifecycle operations.
#[derive(Clone)]
pub struct SoftwareResource {
    client: Client<Authenticated>,
}

impl SoftwareResource {
    /// Lists one cursor page.
    pub async fn list(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<Software>, ApiError> {
        page(&self.client, endpoints::SOFTWARE, cursor, limit).await
    }
    /// Loads software by UUIDv7 or lowercase slug.
    pub async fn get(&self, identity: &str) -> Result<Software, ApiError> {
        self.client.get(&endpoints::software(identity)).await
    }
    /// Creates software without installation metadata.
    pub async fn create(
        &self,
        slug: &str,
        name: &str,
        idempotency_key: Option<&str>,
    ) -> Result<Software, ApiError> {
        self.create_with_installation(slug, name, None, idempotency_key)
            .await
    }
    /// Creates software with optional declarative installation metadata.
    pub async fn create_with_installation(
        &self,
        slug: &str,
        name: &str,
        installation: Option<&InstallationMetadata>,
        idempotency_key: Option<&str>,
    ) -> Result<Software, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            slug: &'a str,
            name: &'a str,
            installation: Option<&'a InstallationMetadata>,
        }
        let mut request = self
            .client
            .request(Method::POST, endpoints::SOFTWARE)
            .json(&Body {
                slug,
                name,
                installation,
            });
        if let Some(key) = idempotency_key {
            request = request.header("idempotency-key", key);
        }
        decode(request.send().await.map_err(|_| ApiError::Transport)?).await
    }
    /// Updates only the display name.
    pub async fn update_name(
        &self,
        identity: &str,
        name: &str,
        revision: u64,
    ) -> Result<Software, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
        }
        self.client
            .json(
                Method::PATCH,
                &endpoints::software(identity),
                &Body { name },
                Some(revision),
            )
            .await
    }
    /// Replaces only installation metadata.
    pub async fn update_installation(
        &self,
        identity: &str,
        installation: &InstallationMetadata,
        revision: u64,
    ) -> Result<Software, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            installation: &'a InstallationMetadata,
        }
        self.client
            .json(
                Method::PATCH,
                &endpoints::software(identity),
                &Body { installation },
                Some(revision),
            )
            .await
    }
    /// Lists releases for software.
    pub async fn releases(
        &self,
        identity: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<Release>, ApiError> {
        page(
            &self.client,
            &endpoints::software_releases(identity),
            cursor,
            limit,
        )
        .await
    }
    /// Loads a release.
    pub async fn release(&self, id: ReleaseId) -> Result<Release, ApiError> {
        self.client.get(&endpoints::release(&id.to_string())).await
    }
    /// Lists variants for a release.
    pub async fn variants(&self, id: ReleaseId) -> Result<Vec<Variant>, ApiError> {
        Ok(self
            .client
            .get::<Items<Variant>>(&endpoints::release_variants(&id.to_string()))
            .await?
            .items)
    }
    /// Rejects a release and transactionally removes active channel bindings.
    pub async fn reject(
        &self,
        id: ReleaseId,
        reason: &str,
        revision: u64,
    ) -> Result<Release, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            reason: &'a str,
        }
        self.client
            .json(
                Method::POST,
                &endpoints::reject_release(&id.to_string()),
                &Body { reason },
                Some(revision),
            )
            .await
    }
    /// Lists software channels.
    pub async fn channels(&self, identity: &str) -> Result<Vec<Channel>, ApiError> {
        Ok(self
            .client
            .get::<Items<Channel>>(&endpoints::software_channels(identity))
            .await?
            .items)
    }
    /// Loads a channel.
    pub async fn channel(&self, identity: &str, name: &str) -> Result<Channel, ApiError> {
        self.client.get(&endpoints::channel(identity, name)).await
    }
    /// Creates or advances testing/stable with an explicit ETag revision (`0` creates).
    pub async fn promote(
        &self,
        identity: &str,
        channel: &str,
        release: ReleaseId,
        pinned_variant: Option<VariantId>,
        reason: Option<&str>,
        revision: u64,
    ) -> Result<Channel, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            release_id: ReleaseId,
            pinned_variant_id: Option<VariantId>,
            reason: Option<&'a str>,
        }
        self.client
            .json(
                Method::PUT,
                &endpoints::channel(identity, channel),
                &Body {
                    release_id: release,
                    pinned_variant_id: pinned_variant,
                    reason,
                },
                Some(revision),
            )
            .await
    }
    /// Resolves exactly one readable primary installer.
    pub async fn resolve(
        &self,
        identity: &str,
        channel: &str,
        platform: &str,
        architecture: &str,
        macos: Option<&str>,
    ) -> Result<Resolution, ApiError> {
        let request = self
            .client
            .request(Method::GET, &endpoints::resolve(identity))
            .query(&[
                ("channel", Some(channel)),
                ("platform", Some(platform)),
                ("architecture", Some(architecture)),
                ("macos", macos),
            ]);
        decode(request.send().await.map_err(|_| ApiError::Transport)?).await
    }
}

/// Async recipe operations.
#[derive(Clone)]
pub struct RecipeResource {
    client: Client<Authenticated>,
}

impl RecipeResource {
    /// Lists recipes.
    pub async fn list(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<Recipe>, ApiError> {
        page(&self.client, endpoints::RECIPES, cursor, limit).await
    }
    /// Loads a recipe by UUIDv7 or exact name.
    pub async fn get(&self, identity: &str) -> Result<Recipe, ApiError> {
        self.client.get(&endpoints::recipe(identity)).await
    }
    /// Creates recipe metadata.
    pub async fn create(
        &self,
        name: &str,
        idempotency_key: Option<&str>,
    ) -> Result<Recipe, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
        }
        let mut request = self
            .client
            .request(Method::POST, endpoints::RECIPES)
            .json(&Body { name });
        if let Some(key) = idempotency_key {
            request = request.header("idempotency-key", key);
        }
        decode(request.send().await.map_err(|_| ApiError::Transport)?).await
    }
    /// Lists immutable revisions.
    pub async fn revisions(&self, identity: &str) -> Result<Vec<RecipeRevision>, ApiError> {
        Ok(self
            .client
            .get::<Items<RecipeRevision>>(&endpoints::recipe_revisions(identity))
            .await?
            .items)
    }
    /// Creates a validated builder-neutral immutable revision.
    pub async fn create_revision(
        &self,
        identity: &str,
        revision: &NewRecipeRevision,
        idempotency_key: Option<&str>,
    ) -> Result<RecipeRevision, ApiError> {
        validate_recipe_revision(revision)?;
        let mut request = self
            .client
            .request(Method::POST, &endpoints::recipe_revisions(identity))
            .json(revision);
        if let Some(key) = idempotency_key {
            request = request.header("idempotency-key", key);
        }
        decode(request.send().await.map_err(|_| ApiError::Transport)?).await
    }
    /// Creates a checked immutable AutoPkg revision through the builder-neutral contract.
    pub async fn create_autopkg_revision(
        &self,
        identity: &str,
        revision: &NewAutoPkgRevision,
        idempotency_key: Option<&str>,
    ) -> Result<RecipeRevision, ApiError> {
        validate_autopkg_revision(revision)?;
        self.create_revision(
            identity,
            &NewRecipeRevision::autopkg(revision),
            idempotency_key,
        )
        .await
    }
    /// Lists runs associated with a recipe.
    pub async fn runs(
        &self,
        identity: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<RunSummary>, ApiError> {
        page(
            &self.client,
            &endpoints::recipe_runs(identity),
            cursor,
            limit,
        )
        .await
    }
}

/// Async run operations.
#[derive(Clone)]
pub struct RunResource {
    client: Client<Authenticated>,
}

impl RunResource {
    /// Lists run summaries without large parameter/result payloads.
    pub async fn list(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<RunSummary>, ApiError> {
        page(&self.client, endpoints::RUNS, cursor, limit).await
    }
    /// Queues a builder-neutral run idempotently.
    pub async fn create(
        &self,
        software: &str,
        revision: RecipeRevisionId,
        parameters: &serde_json::Map<String, serde_json::Value>,
        idempotency_key: &str,
    ) -> Result<Run, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            software: &'a str,
            recipe_revision: RecipeRevisionId,
            parameters: &'a serde_json::Map<String, serde_json::Value>,
        }
        decode(
            self.client
                .request(Method::POST, endpoints::RUNS)
                .header("idempotency-key", idempotency_key)
                .json(&Body {
                    software,
                    recipe_revision: revision,
                    parameters,
                })
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }
    /// Loads full run details, provenance, and verification output.
    pub async fn get(&self, id: RunId) -> Result<Run, ApiError> {
        self.client.get(&endpoints::run(&id.to_string())).await
    }
    /// Requests cancellation idempotently.
    pub async fn cancel(&self, id: RunId, idempotency_key: &str) -> Result<Run, ApiError> {
        decode(
            self.client
                .request(Method::POST, &endpoints::run_cancel(&id.to_string()))
                .header("idempotency-key", idempotency_key)
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }
    /// Lists ordered exact-byte logs.
    pub async fn logs(
        &self,
        id: RunId,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<RunLog>, ApiError> {
        page(
            &self.client,
            &endpoints::run_logs(&id.to_string()),
            cursor,
            limit,
        )
        .await
    }
    /// Opens the server-sent-event replay/live stream as bounded transport chunks.
    pub async fn events(
        &self,
        id: RunId,
        last_event_id: Option<u64>,
    ) -> Result<RunEventStream, ApiError> {
        let mut request = self
            .client
            .request(Method::GET, &endpoints::run_events(&id.to_string()))
            .timeout(Duration::from_secs(3600));
        if let Some(id) = last_event_id {
            request = request.header("last-event-id", id);
        }
        let response = request.send().await.map_err(|_| ApiError::Transport)?;
        if !response.status().is_success() {
            return Err(decode_error(response).await);
        }
        if !response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|value| value.trim().eq_ignore_ascii_case("text/event-stream"))
            })
        {
            return Err(ApiError::InvalidEventStream);
        }
        Ok(RunEventStream {
            stream: Box::pin(
                response
                    .bytes_stream()
                    .map(|result| result.map_err(|_| ApiError::Transport)),
            ),
        })
    }
    /// Polls until the run is terminal or the timeout expires.
    pub async fn wait(
        &self,
        id: RunId,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<Run, ApiError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let run = tokio::time::timeout_at(deadline, self.get(id))
                .await
                .map_err(|_| ApiError::WaitTimeout)??;
            if run.is_terminal() {
                return Ok(run);
            }
            tokio::time::timeout_at(
                deadline,
                tokio::time::sleep(poll_interval.max(Duration::from_millis(10))),
            )
            .await
            .map_err(|_| ApiError::WaitTimeout)?;
        }
    }
}

/// Crate-owned asynchronous SSE byte stream.
pub struct RunEventStream {
    stream: Pin<Box<dyn Stream<Item = Result<Bytes, ApiError>> + Send>>,
}
impl RunEventStream {
    /// Returns transport chunks; callers may feed them to an SSE parser incrementally.
    pub fn into_stream(self) -> Pin<Box<dyn Stream<Item = Result<Bytes, ApiError>> + Send>> {
        self.stream
    }
}

/// Async artifact operations.
#[derive(Clone)]
pub struct ArtifactResource {
    client: Client<Authenticated>,
}

impl ArtifactResource {
    /// Loads immutable artifact metadata.
    pub async fn get(&self, digest: &Sha256Digest) -> Result<Artifact, ApiError> {
        self.client.get(&endpoints::artifact(digest.as_str())).await
    }
    /// Lists independently tracked locations.
    pub async fn locations(
        &self,
        digest: &Sha256Digest,
    ) -> Result<Vec<ArtifactLocation>, ApiError> {
        Ok(self
            .client
            .get::<Items<ArtifactLocation>>(&endpoints::artifact_locations(digest.as_str()))
            .await?
            .items)
    }
    /// Loads immutable content headers without transferring the body.
    pub async fn head(&self, digest: &Sha256Digest) -> Result<ArtifactContentHead, ApiError> {
        let response = self
            .client
            .request(Method::HEAD, &endpoints::artifact_content(digest.as_str()))
            .send()
            .await
            .map_err(|_| ApiError::Transport)?;
        if !response.status().is_success() {
            return Err(decode_error(response).await);
        }
        let content_length = response
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .ok_or(ApiError::Decode)?;
        let etag = response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .ok_or(ApiError::Decode)?;
        let accepts_ranges = response
            .headers()
            .get(header::ACCEPT_RANGES)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value == "bytes");
        Ok(ArtifactContentHead {
            content_length,
            etag,
            accepts_ranges,
        })
    }
    /// Uploads declared immutable bytes; the server rehashes before publication.
    pub async fn upload_bytes(
        &self,
        digest: &Sha256Digest,
        bytes: Bytes,
    ) -> Result<ArtifactUpload, ApiError> {
        decode(
            self.client
                .request(Method::PUT, &endpoints::artifact_content(digest.as_str()))
                .timeout(Duration::from_secs(3600))
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(bytes)
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }
    /// Opens a bounded streaming download, optionally resuming from an offset.
    pub async fn download(
        &self,
        digest: &Sha256Digest,
        offset: Option<u64>,
    ) -> Result<ArtifactDownload, ApiError> {
        let mut request = self
            .client
            .request(Method::GET, &endpoints::artifact_content(digest.as_str()))
            .timeout(Duration::from_secs(3600));
        if let Some(offset) = offset {
            request = request.header(header::RANGE, format!("bytes={offset}-"));
        }
        let response = request.send().await.map_err(|_| ApiError::Transport)?;
        let resumed = response.status() == StatusCode::PARTIAL_CONTENT;
        if !response.status().is_success() {
            return Err(decode_error(response).await);
        }
        if offset.is_some_and(|value| value > 0) && !resumed {
            return Err(ApiError::InvalidValue {
                kind: "download range",
                detail: "server did not honor the requested resume offset",
            });
        }
        let content_length = response.content_length();
        let etag = response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let stream = response
            .bytes_stream()
            .map(|result| result.map_err(|_| ApiError::Transport));
        Ok(ArtifactDownload {
            content_length,
            etag,
            resumed,
            stream: Box::pin(stream),
        })
    }
}

/// Crate-owned asynchronous artifact stream.
pub struct ArtifactDownload {
    /// Response-body size for this transfer.
    pub content_length: Option<u64>,
    /// Strong digest ETag.
    pub etag: Option<String>,
    /// Whether the server returned `206 Partial Content`.
    pub resumed: bool,
    stream: Pin<Box<dyn Stream<Item = Result<Bytes, ApiError>> + Send>>,
}
impl ArtifactDownload {
    /// Returns the bounded byte stream.
    pub fn into_stream(self) -> Pin<Box<dyn Stream<Item = Result<Bytes, ApiError>> + Send>> {
        self.stream
    }
}

/// Async store operations.
#[derive(Clone)]
pub struct StoreResource {
    client: Client<Authenticated>,
}
impl StoreResource {
    /// Lists configured stores.
    pub async fn list(&self) -> Result<Vec<Store>, ApiError> {
        Ok(self
            .client
            .get::<Items<Store>>(endpoints::STORES)
            .await?
            .items)
    }
    /// Loads a configured store.
    pub async fn get(&self, id: StoreId) -> Result<Store, ApiError> {
        self.client.get(&endpoints::store(&id.to_string())).await
    }
    /// Runs a non-mutating adapter probe.
    pub async fn test(&self, id: StoreId) -> Result<StoreTest, ApiError> {
        decode(
            self.client
                .request(Method::POST, &endpoints::store_test(&id.to_string()))
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }
}

/// Async worker administration.
#[derive(Clone)]
pub struct WorkerResource {
    client: Client<Authenticated>,
}
impl WorkerResource {
    /// Lists workers.
    pub async fn list(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<Worker>, ApiError> {
        page(&self.client, endpoints::WORKERS, cursor, limit).await
    }
    /// Provisions an outbound worker and returns a credential once.
    pub async fn provision(
        &self,
        name: &str,
        allowed_capabilities: &[String],
    ) -> Result<WorkerCredential, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            allowed_capabilities: &'a [String],
        }
        self.client
            .json(
                Method::POST,
                endpoints::WORKERS,
                &Body {
                    name,
                    allowed_capabilities,
                },
                None,
            )
            .await
    }
    /// Loads administrative worker metadata.
    pub async fn get(&self, id: WorkerId) -> Result<Worker, ApiError> {
        self.client.get(&endpoints::worker(&id.to_string())).await
    }
    /// Enables or drains a worker and invalidates active attempts when disabling.
    pub async fn set_enabled(
        &self,
        id: WorkerId,
        enabled: bool,
        revision: u64,
    ) -> Result<Worker, ApiError> {
        #[derive(Serialize)]
        struct Body {
            enabled: bool,
        }
        self.client
            .json(
                Method::PATCH,
                &endpoints::worker(&id.to_string()),
                &Body { enabled },
                Some(revision),
            )
            .await
    }
    /// Replaces the server-enforced capability ceiling.
    pub async fn set_allowed_capabilities(
        &self,
        id: WorkerId,
        allowed_capabilities: &[String],
        revision: u64,
    ) -> Result<Worker, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            allowed_capabilities: &'a [String],
        }
        self.client
            .json(
                Method::PATCH,
                &endpoints::worker(&id.to_string()),
                &Body {
                    allowed_capabilities,
                },
                Some(revision),
            )
            .await
    }
    /// Rotates a worker credential and invalidates active attempts.
    pub async fn rotate_token(
        &self,
        id: WorkerId,
        revision: u64,
    ) -> Result<RotatedWorkerCredential, ApiError> {
        let response = self
            .client
            .request(Method::POST, &endpoints::worker_rotate(&id.to_string()))
            .header(header::IF_MATCH, revision_etag(revision))
            .send()
            .await
            .map_err(|_| ApiError::Transport)?;
        decode(response).await
    }
}

/// Async job inspection.
#[derive(Clone)]
pub struct JobResource {
    client: Client<Authenticated>,
}
impl JobResource {
    /// Lists lightweight job summaries.
    pub async fn list(
        &self,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<CursorPage<JobSummary>, ApiError> {
        page(&self.client, endpoints::JOBS, cursor, limit).await
    }
    /// Loads a full builder-neutral job payload.
    pub async fn get(&self, id: JobId) -> Result<Job, ApiError> {
        self.client.get(&endpoints::job(&id.to_string())).await
    }
}

/// Async audit operations.
#[derive(Clone)]
pub struct AuditResource {
    client: Client<Authenticated>,
}
impl AuditResource {
    /// Lists one cursor page of append-only audit events.
    pub async fn list(&self, cursor: Option<&str>, limit: u32) -> Result<AuditPage, ApiError> {
        page(&self.client, endpoints::AUDIT, cursor, limit).await
    }
}

#[derive(Deserialize)]
struct Items<T> {
    items: Vec<T>,
}

async fn page<T: DeserializeOwned>(
    client: &Client<Authenticated>,
    path: &str,
    cursor: Option<&str>,
    limit: u32,
) -> Result<CursorPage<T>, ApiError> {
    validate_page(limit)?;
    let mut request = client
        .request(Method::GET, path)
        .query(&[("limit", limit.to_string())]);
    if let Some(cursor) = cursor {
        request = request.query(&[("cursor", cursor)]);
    }
    decode(request.send().await.map_err(|_| ApiError::Transport)?).await
}

fn revision_etag(revision: u64) -> String {
    format!("\"rev-{revision}\"")
}

async fn decode<T: DeserializeOwned>(response: Response) -> Result<T, ApiError> {
    if !response.status().is_success() {
        return Err(decode_error(response).await);
    }
    let body = read_bounded(response).await?;
    serde_json::from_slice(&body).map_err(|_| ApiError::Decode)
}
async fn decode_empty(response: Response) -> Result<(), ApiError> {
    if !response.status().is_success() {
        return Err(decode_error(response).await);
    }
    Ok(())
}
async fn decode_error(response: Response) -> ApiError {
    let status = response.status().as_u16();
    let request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    match read_bounded(response)
        .await
        .ok()
        .and_then(|body| serde_json::from_slice::<Problem>(&body).ok())
    {
        Some(problem) => ApiError::Server(problem),
        None => ApiError::HttpStatus { status, request_id },
    }
}

async fn decode_raw(response: Response) -> Result<RawResponse, ApiError> {
    if !response.status().is_success() {
        return Err(decode_error(response).await);
    }
    let status = response.status().as_u16();
    let request_id = response
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = read_bounded(response).await?;
    Ok(RawResponse {
        status,
        request_id,
        content_type,
        body,
    })
}

async fn read_bounded(mut response: Response) -> Result<Vec<u8>, ApiError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RAW_RESPONSE_BYTES as u64)
    {
        return Err(ApiError::ResponseTooLarge);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| ApiError::Transport)? {
        if body.len().saturating_add(chunk.len()) > MAX_RAW_RESPONSE_BYTES {
            return Err(ApiError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

impl Client<Unauthenticated> {
    /// Returns traffic readiness separately from process liveness.
    pub async fn readiness(&self) -> Result<Health, ApiError> {
        decode(
            self.http
                .get(self.base_url.endpoint(endpoints::READY))
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }
}
impl Client<Authenticated> {
    /// Returns durable queue and worker measurements.
    pub async fn operational_status(&self) -> Result<crate::OperationalStatus, ApiError> {
        self.get(endpoints::OPERATIONS).await
    }
}
impl SoftwareResource {
    /// Returns execution and publication status without client-side joins.
    pub async fn status(&self, identity: &str) -> Result<crate::SoftwareStatus, ApiError> {
        self.client.get(&endpoints::software_status(identity)).await
    }
    /// Withdraws publication eligibility while preserving lifecycle and immutable bytes.
    pub async fn withdraw(
        &self,
        release: ReleaseId,
        reason: &str,
        revision: u64,
    ) -> Result<Release, ApiError> {
        if reason.is_empty()
            || reason.len() > 1024
            || reason.trim() != reason
            || reason.chars().any(char::is_control)
        {
            return Err(ApiError::InvalidValue {
                kind: "withdrawal reason",
                detail: "expected 1-1024 non-control bytes without surrounding whitespace",
            });
        }
        decode(
            self.client
                .request(
                    Method::POST,
                    &endpoints::withdraw_release(&release.to_string()),
                )
                .header(header::IF_MATCH, revision_etag(revision))
                .json(&serde_json::json!({"reason": reason}))
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }
}
impl WorkerResource {
    /// Pauses or resumes claims without invalidating active attempts.
    pub async fn set_draining(
        &self,
        worker: WorkerId,
        draining: bool,
        revision: u64,
    ) -> Result<Worker, ApiError> {
        decode(
            self.client
                .request(Method::POST, &endpoints::drain_worker(&worker.to_string()))
                .header(header::IF_MATCH, revision_etag(revision))
                .json(&serde_json::json!({"draining": draining}))
                .send()
                .await
                .map_err(|_| ApiError::Transport)?,
        )
        .await
    }
}

/// Crate-owned stream of validated run events with replay after interruption.
pub struct TypedRunEvents {
    stream: Pin<Box<dyn Stream<Item = Result<crate::RunEvent, ApiError>> + Send>>,
}
impl TypedRunEvents {
    /// Consumes this wrapper into a typed event stream.
    pub fn into_stream(
        self,
    ) -> Pin<Box<dyn Stream<Item = Result<crate::RunEvent, ApiError>> + Send>> {
        self.stream
    }
}
type EventByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, ApiError>> + Send>>;

struct AsyncWatchState {
    resource: RunResource,
    cursor: crate::events::WatchCursor,
    source: Option<EventByteStream>,
    deadline: tokio::time::Instant,
    failures: u8,
}
impl RunResource {
    /// Watches typed events until completion or the overall deadline, resuming interrupted streams.
    pub fn watch(
        &self,
        id: RunId,
        last_event_id: Option<u64>,
        timeout: Duration,
    ) -> TypedRunEvents {
        let state = AsyncWatchState {
            resource: self.clone(),
            cursor: crate::events::WatchCursor::new(id, last_event_id),
            source: None,
            deadline: tokio::time::Instant::now() + timeout,
            failures: 0,
        };
        let stream = futures_util::stream::unfold(state, |mut state| async {
            if state.cursor.finished {
                return None;
            }
            let result = tokio::time::timeout_at(state.deadline, state.next_event())
                .await
                .unwrap_or(Err(ApiError::WaitTimeout));
            if result.is_err() {
                state.cursor.finished = true;
            }
            Some((result, state))
        });
        TypedRunEvents {
            stream: Box::pin(stream),
        }
    }
}
impl AsyncWatchState {
    async fn next_event(&mut self) -> Result<crate::RunEvent, ApiError> {
        loop {
            if let Some(event) = self.cursor.pop() {
                return Ok(event);
            }
            if self.failures > 3 {
                return Err(ApiError::EventStreamInterrupted);
            }
            if self.source.is_none() {
                if self.failures > 0 {
                    tokio::time::sleep(Duration::from_millis(100 * u64::from(self.failures))).await;
                }
                match self
                    .resource
                    .events(self.cursor.run, self.cursor.last)
                    .await
                {
                    Ok(source) => {
                        self.cursor.reconnect();
                        self.source = Some(source.into_stream());
                    }
                    Err(ApiError::Transport) => {
                        self.failures += 1;
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
            if let Some(Ok(bytes)) = self
                .source
                .as_mut()
                .expect("source opened above")
                .next()
                .await
            {
                self.cursor.feed(&bytes)?;
            } else {
                self.source = None;
                self.failures += 1;
            }
        }
    }
}

impl RecipeResource {
    /// Appends a revision only if this is still the expected next sequence.
    pub async fn create_revision_at(
        &self,
        recipe: &str,
        revision: &NewRecipeRevision,
        sequence: u64,
    ) -> Result<RecipeRevision, ApiError> {
        validate_recipe_revision(revision)?;
        if sequence == 0 {
            return Err(ApiError::InvalidValue {
                kind: "recipe sequence",
                detail: "sequence must be positive",
            });
        }
        decode(self.client.request(Method::POST, &endpoints::recipe_revisions(recipe))
            .json(&serde_json::json!({"builder": revision.builder, "definition": revision.definition, "required_capabilities": revision.required_capabilities, "expected_sequence": sequence}))
            .send().await.map_err(|_| ApiError::Transport)?).await
    }
}
