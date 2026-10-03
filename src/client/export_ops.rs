//! Shared typed saved-export operations for both transports.
macro_rules! export_methods {
    ($($mode:ident)?, $($wait:tt)*) => {
        /// Protected repository base URL for device configuration, without credentials.
        pub fn repository_url(&self, identity:&str)->String {
            self.client.base_url.endpoint(&endpoints::export_action(identity,"repository"))
        }
        /// Lists saved exports.
        pub $($mode)? fn list(&self, cursor: Option<&str>, limit:u32)->Result<CursorPage<crate::exports::ExportRecord>,ApiError> {
            page(&self.client, endpoints::EXPORTS, cursor, limit)$($wait)*
        }
        /// Gets one export by ID or slug.
        pub $($mode)? fn get(&self, identity:&str)->Result<crate::exports::ExportRecord,ApiError> {
            self.client.get(&endpoints::export(identity))$($wait)*
        }
        /// Saves a new draft; never publishes.
        pub $($mode)? fn create(&self, definition:&crate::exports::ExportDefinition)->Result<crate::exports::ExportRecord,ApiError> {
            self.client.json(Method::POST,endpoints::EXPORTS,definition,None)$($wait)*
        }
        /// Replaces a saved draft using its observed revision.
        pub $($mode)? fn update(&self, identity:&str, definition:&crate::exports::ExportDefinition, revision:u64)->Result<crate::exports::ExportRecord,ApiError> {
            self.client.json(Method::PUT,&endpoints::export(identity),definition,Some(revision))$($wait)*
        }
        /// Previews all selections and blockers without publication.
        pub $($mode)? fn plan(&self, identity:&str)->Result<crate::exports::ExportPlan,ApiError> {
            self.client.json(Method::POST,&endpoints::export_action(identity,"plan"),&(),None)$($wait)*
        }
        /// Atomically publishes a reviewed plan; the server rejects stale facts and blockers.
        pub $($mode)? fn apply(&self, plan:&crate::exports::ExportPlan)->Result<crate::exports::ExportSnapshotView,ApiError> {
            if !plan.ready { return Err(ApiError::StalePlan); }
            decode(self.client.request(Method::POST,&endpoints::export_action(&plan.export.to_string(),"apply"))
                .timeout(Duration::from_secs(3600)).json(&serde_json::json!({"fingerprint":plan.fingerprint,"reviewed":true}))
                .send()$($wait)*.map_err(|_|ApiError::Transport)?)$($wait)*
        }
        /// Reads a publication plus current release eligibility.
        pub $($mode)? fn snapshot(&self, identity:&str, generation:u64)->Result<crate::exports::ExportSnapshotView,ApiError> {
            self.client.get(&endpoints::export_snapshot(identity,generation))$($wait)*
        }
        /// Lists publication history, ordered by generation.
        pub $($mode)? fn history(&self, identity:&str, after:u64, limit:u32)->Result<CursorPage<crate::exports::ExportSnapshot>,ApiError> {
            validate_page(limit)?;
            decode(self.client.request(Method::GET,&endpoints::export_action(identity,"history"))
                .query(&[("after",after.to_string()),("limit",limit.to_string())]).send()$($wait)*.map_err(|_|ApiError::Transport)?)$($wait)*
        }
        /// Issues a repository-only reader credential for protected device configuration.
        pub $($mode)? fn issue_reader(&self, identity:&str)->Result<crate::exports::ExportReader,ApiError> {
            self.client.json(Method::POST,&endpoints::export_action(identity,"readers"),&(),None)$($wait)*
        }
        /// Revokes all earlier reader credentials for this export.
        pub $($mode)? fn revoke_readers(&self, identity:&str)->Result<(),ApiError> {
            self.client.empty_json(Method::POST,&endpoints::export_action(identity,"readers/revoke"),&(),None)$($wait)*
        }
    }
}
pub(crate) use export_methods;
