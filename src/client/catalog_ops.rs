//! Shared async/blocking reconciliation orchestration; policy stays in the neutral planner.
macro_rules! catalog_methods {
    ($($mode:ident)?, $($wait:tt)*) => {
        /// Validates raw desired state, then computes an ordered read-only plan.
        pub $($mode)? fn plan(&self, manifest: &CatalogManifest) -> Result<CatalogPlan, ApiError> {
            let manifest = crate::ValidatedCatalogManifest::new(manifest.clone())?;
            self.plan_validated(&manifest)$($wait)*
        }
        /// Plans from a preserved validation proof without reconstructing its invariants.
        pub $($mode)? fn plan_validated(&self, manifest: &crate::ValidatedCatalogManifest) -> Result<CatalogPlan, ApiError> {
            let software = self.all_software()$($wait)*?;
            let recipes = self.all_recipes(manifest.as_manifest())$($wait)*?;
            let mut targets = Vec::new();
            if !manifest.as_manifest().targets.is_empty() {
                let mut cursor = None;
                let mut traversal = super::CatalogTraversal::default();
                loop {
                    let page = self.client.build_targets().list(cursor.as_deref(), 200)$($wait)*?;
                    targets.extend(page.items);
                    if page.next_cursor.is_none() { break; }
                    cursor = traversal.advance(page.next_cursor)?;
                }
            }
            build_plan(manifest, &software, &recipes, &targets)
        }
        /// Reconciles validated desired state using revision and append-sequence preconditions.
        pub $($mode)? fn sync(&self, manifest: &CatalogManifest) -> Result<CatalogSyncReport, ApiError> {
            let manifest = crate::ValidatedCatalogManifest::new(manifest.clone())?;
            self.sync_validated(&manifest, None)$($wait)*
        }
        /// Applies a freshly computed plan only if it matches an optionally reviewed saved plan.
        pub $($mode)? fn sync_validated(&self, manifest: &crate::ValidatedCatalogManifest, reviewed: Option<&CatalogPlan>) -> Result<CatalogSyncReport, ApiError> {
            let plan = self.plan_validated(manifest)$($wait)*?;
            if reviewed.is_some_and(|reviewed| reviewed != &plan) { return Err(ApiError::StalePlan); }
            let mut created = std::collections::BTreeMap::new();
            for action in &plan.actions {
                match action {
                    CatalogAction::CreateSoftware { software } => {
                        let key = format!("catalog-software-{}", software.slug);
                        self.client.software().create_with_installation(&software.slug, &software.name, software.installation.as_ref(), Some(&key))$($wait)*?;
                    }
                    CatalogAction::UpdateSoftwareName { slug, name, expected_revision } => {
                        self.client.software().update_name(slug, name, *expected_revision)$($wait)*?;
                    }
                    CatalogAction::UpdateSoftwareInstallation { slug, installation, expected_revision } => {
                        self.client.software().update_installation(slug, installation, *expected_revision)$($wait)*?;
                    }
                    CatalogAction::CreateRecipe { name } => {
                        let key = format!("catalog-recipe-{name}");
                        self.client.recipes().create(name, Some(&key))$($wait)*?;
                    }
                    CatalogAction::CreateRecipeRevision { recipe, revision, expected_sequence } => {
                        let value = self.client.recipes().create_revision_at(recipe, revision, *expected_sequence)$($wait)*?;
                        if value.sequence != *expected_sequence { return Err(ApiError::Decode); }
                        created.insert((recipe.clone(), *expected_sequence), value.id);
                    }
                    CatalogAction::CreateTarget { target, revision } => {
                        let id = revision.resolve(&created)?;
                        self.client.build_targets().create(&target.name, &target.software, id, &target.parameters, target.schedule, None, target.enabled)$($wait)*?;
                    }
                    CatalogAction::UpdateTarget { target, revision, expected_revision } => {
                        let update = BuildTargetUpdate { recipe_revision: Some(revision.resolve(&created)?), parameters: Some(target.parameters.clone()),
                            schedule: Some(target.schedule), enabled: Some(target.enabled), ..BuildTargetUpdate::default() };
                        self.client.build_targets().update(&target.name, &update, *expected_revision)$($wait)*?;
                    }
                }
            }
            Ok(CatalogSyncReport { schema_version: plan.schema_version, applied: plan.actions })
        }
    }
}
pub(crate) use catalog_methods;
