//! Independent downstream consumer used by pinned-server integration tests.

use std::time::Duration;

use futures_util::StreamExt;
use stabbur_client::{
    BuildTargetSchedule, BuildTargetUpdate, CatalogManifest, CatalogRecipe, CatalogSoftware,
    Client, NewRecipeRevision, PinnedSource, SecretToken, Sha256Digest,
};

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = std::env::var("STABBUR_E2E_SERVER_URL")?;
    let token = SecretToken::new(std::env::var("STABBUR_E2E_TOKEN")?)?;
    let public = Client::from_url(&server)?;
    assert_eq!(public.health().await?.status, "ok");
    assert_eq!(
        public.openapi_document().await?["info"]["version"],
        stabbur_client::TARGET_SERVER_VERSION.trim_end_matches("-dev")
    );

    let client = public.authenticate(token);
    let principal = client.me().await?;
    assert_eq!(principal.name, "live-admin");

    let software = client
        .software()
        .create("e2e-software", "E2E Software", Some("e2e-software-create"))
        .await?;
    assert_eq!(client.software().get("e2e-software").await?.id, software.id);
    let software = client
        .software()
        .update_name("e2e-software", "E2E Software Updated", software.revision)
        .await?;
    assert_eq!(software.name, "E2E Software Updated");
    assert!(
        client
            .software()
            .releases("e2e-software", None, 50)
            .await?
            .items
            .is_empty()
    );
    assert!(client.software().channels("e2e-software").await?.is_empty());

    let manifest = CatalogManifest {
        targets: vec![],
        schema_version: 1,
        software: vec![CatalogSoftware {
            slug: "e2e-catalog-software".into(),
            name: "E2E Catalog Software".into(),
            installation: None,
        }],
        recipes: vec![CatalogRecipe {
            name: "e2e-recipe".into(),
            revision: NewRecipeRevision::fake(vec![]),
        }],
    };
    assert_eq!(client.catalog().plan(&manifest).await?.actions.len(), 3);
    assert_eq!(client.catalog().sync(&manifest).await?.applied.len(), 3);
    assert!(client.catalog().plan(&manifest).await?.is_empty());
    let recipe = client.recipes().get("e2e-recipe").await?;
    let revision = client.recipes().revisions("e2e-recipe").await?[0].clone();
    assert_eq!(revision.recipe_id, recipe.id);
    assert_eq!(revision.builder, "fake");
    assert_eq!(client.recipes().revisions("e2e-recipe").await?.len(), 1);
    assert!(client.catalog().snapshots(None, 50).await?.items.is_empty());
    assert!(!client.catalog().resolve("e2e-recipe.fake").await?.exists);
    let scan = client
        .catalog()
        .request_autopkg_scan(
            &PinnedSource {
                url: "https://127.0.0.1:9/e2e-recipes.git".into(),
                commit: "a".repeat(40),
            },
            "e2e-catalog-scan",
        )
        .await?;
    assert_eq!(client.catalog().scan(scan.id).await?.state, "queued");
    let scan = client
        .catalog()
        .cancel_scan(scan.id, "e2e-catalog-scan-cancel")
        .await?;
    assert_eq!(scan.state, "cancelled");
    assert_eq!(client.catalog().scans(None, 50).await?.items.len(), 1);

    let parameters = std::collections::BTreeMap::new();
    let target = client
        .build_targets()
        .create(
            "e2e-manual-target",
            "e2e-catalog-software",
            revision.id,
            &parameters,
            BuildTargetSchedule::Manual,
            None,
            true,
        )
        .await?;
    assert_eq!(
        client.build_targets().get("e2e-manual-target").await?.id,
        target.id
    );
    assert_eq!(client.build_targets().list(None, 50).await?.items.len(), 1);
    let run = client
        .build_targets()
        .trigger("e2e-manual-target", "e2e-fake-target-run")
        .await?;
    let run = client
        .runs()
        .wait(run.id, Duration::from_millis(100), Duration::from_secs(30))
        .await?;
    assert_eq!(run.state, "succeeded");
    assert_eq!(run.result.as_ref().unwrap()["adapter"], "fake");
    assert_eq!(
        client
            .build_targets()
            .runs("e2e-manual-target", None, 50)
            .await?
            .items[0]
            .id,
        run.id
    );
    let target = client
        .build_targets()
        .update(
            "e2e-manual-target",
            &BuildTargetUpdate {
                name: Some("e2e-manual-target-renamed".into()),
                ..BuildTargetUpdate::default()
            },
            target.revision,
        )
        .await?;
    assert_eq!(target.name, "e2e-manual-target-renamed");
    assert_eq!(
        client.recipes().runs("e2e-recipe", None, 50).await?.items[0].id,
        run.id
    );
    let job = client
        .jobs()
        .list(None, 50)
        .await?
        .items
        .into_iter()
        .find(|job| job.run_id == Some(run.id))
        .expect("fake run has one durable job");
    assert_eq!(job.state, "succeeded");
    assert_eq!(job.attempt_count, 1);
    let job = client.jobs().get(job.id).await?;
    assert_eq!(job.payload["adapter"], "fake");

    let service = client
        .identity()
        .create_service("e2e-service", &["reader".to_owned()])
        .await?;
    let created_token = client
        .identity()
        .create_token("e2e-service", "integration", None)
        .await?;
    assert!(!format!("{created_token:?}").contains(created_token.secret.expose_secret()));
    assert_eq!(client.identity().list_tokens("e2e-service").await?.len(), 1);
    client
        .identity()
        .revoke_token(&created_token.token.id)
        .await?;
    let service = client
        .identity()
        .set_enabled("e2e-service", false, service.revision)
        .await?;
    assert!(!service.enabled);

    let worker_credential = client
        .workers()
        .provision(
            "e2e-worker",
            &["builder.autopkg".to_owned(), "os.macos".to_owned()],
        )
        .await?;
    assert!(!format!("{worker_credential:?}").contains(worker_credential.token.expose_secret()));
    let worker = client.workers().get(worker_credential.worker_id).await?;
    let rotated = client
        .workers()
        .rotate_token(worker.id, worker.revision)
        .await?;
    assert!(!format!("{rotated:?}").contains(rotated.token.expose_secret()));
    let worker = client
        .workers()
        .set_enabled(rotated.worker.id, false, rotated.worker.revision)
        .await?;
    assert!(!worker.enabled);

    let digest: Sha256Digest =
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".parse()?;
    let upload = client
        .artifacts()
        .upload_bytes(&digest, bytes::Bytes::from_static(b"hello"))
        .await?;
    assert_eq!(upload.size, 5);
    assert_eq!(client.artifacts().head(&digest).await?.content_length, 5);
    assert_eq!(client.artifacts().get(&digest).await?.locations.len(), 1);
    assert_eq!(client.artifacts().locations(&digest).await?.len(), 1);
    let downloaded = client.artifacts().download(&digest, None).await?;
    let chunks = downloaded.into_stream().collect::<Vec<_>>().await;
    let body = chunks.into_iter().collect::<Result<Vec<_>, _>>()?.concat();
    assert_eq!(body, b"hello");

    assert_eq!(client.stores().list().await?.len(), 1);
    assert!(
        client
            .stores()
            .test(client.stores().list().await?[0].id)
            .await?
            .healthy
    );
    assert_eq!(client.runs().list(None, 50).await?.items.len(), 1);
    assert_eq!(client.jobs().list(None, 50).await?.items.len(), 2);
    assert!(!client.audit().list(None, 200).await?.items.is_empty());

    println!(
        "complete public-client workflow passed for {principal}",
        principal = principal.name
    );
    Ok(())
}
