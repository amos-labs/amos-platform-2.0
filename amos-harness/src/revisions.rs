//! Entity revision tracking and template management services.
//!
//! Provides:
//! - `RevisionService`: Create, list, get, and revert entity revisions
//! - `TemplateService`: Template registry, versions, subscriptions, and update checks

use amos_core::{AmosError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

// ============================================================================
// Row Types (mapped from DB tables)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RevisionRow {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub version: i32,
    pub content_hash: String,
    pub snapshot: JsonValue,
    pub diff_from_prev: Option<JsonValue>,
    pub change_type: String,
    pub changed_by: Option<String>,
    pub change_summary: Option<String>,
    pub template_id: Option<Uuid>,
    pub template_version: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TemplateRegistryRow {
    pub id: Uuid,
    pub entity_type: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub current_version: i32,
    pub content_hash: String,
    pub snapshot: JsonValue,
    pub category: Option<String>,
    pub icon_url: Option<String>,
    pub tags: Option<JsonValue>,
    pub metadata: Option<JsonValue>,
    pub is_published: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub deprecated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TemplateVersionRow {
    pub id: Uuid,
    pub template_id: Uuid,
    pub version: i32,
    pub content_hash: String,
    pub snapshot: JsonValue,
    pub diff_from_prev: Option<JsonValue>,
    pub release_notes: Option<String>,
    pub is_breaking: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TemplateSubscriptionRow {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub template_id: Uuid,
    pub installed_version: i32,
    pub latest_version: Option<i32>,
    pub customization_status: String,
    pub local_content_hash: Option<String>,
    pub template_content_hash: Option<String>,
    pub auto_update: bool,
    pub pin_version: bool,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Request / Response Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRevisionRequest {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub snapshot: JsonValue,
    pub change_type: String,
    pub changed_by: String,
    pub change_summary: Option<String>,
    pub template_id: Option<Uuid>,
    pub template_version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertRequest {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub target_version: i32,
    pub changed_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionListResponse {
    pub revisions: Vec<RevisionRow>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateCheckResult {
    pub template_slug: String,
    pub current_version: i32,
    pub latest_version: i32,
    pub customization_status: String,
    pub has_update: bool,
    pub is_breaking: bool,
}

// ============================================================================
// RevisionService
// ============================================================================

pub struct RevisionService {
    db_pool: PgPool,
}

impl RevisionService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    /// Create a new revision for an entity.
    pub async fn create_revision(&self, req: CreateRevisionRequest) -> Result<RevisionRow> {
        // Get the latest version number for this entity
        let max_version: Option<i32> = sqlx::query_scalar(
            "SELECT MAX(version) FROM entity_revisions WHERE entity_type = $1 AND entity_id = $2",
        )
        .bind(&req.entity_type)
        .bind(req.entity_id)
        .fetch_one(&self.db_pool)
        .await?;

        let next_version = max_version.unwrap_or(0) + 1;

        // Compute content hash
        let content_hash = Self::compute_content_hash(&req.snapshot);

        // Get previous snapshot for diff computation
        let prev_snapshot: Option<JsonValue> = if next_version > 1 {
            sqlx::query_scalar(
                "SELECT snapshot FROM entity_revisions
                 WHERE entity_type = $1 AND entity_id = $2 AND version = $3",
            )
            .bind(&req.entity_type)
            .bind(req.entity_id)
            .bind(next_version - 1)
            .fetch_optional(&self.db_pool)
            .await?
        } else {
            None
        };

        // Compute diff from previous version
        let diff_from_prev = if let Some(prev) = prev_snapshot {
            Self::compute_diff(&prev, &req.snapshot)
        } else {
            None
        };

        // Insert new revision
        let revision = sqlx::query_as::<_, RevisionRow>(
            "INSERT INTO entity_revisions
             (id, entity_type, entity_id, version, content_hash, snapshot, diff_from_prev,
              change_type, changed_by, change_summary, template_id, template_version, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
             RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(&req.entity_type)
        .bind(req.entity_id)
        .bind(next_version)
        .bind(&content_hash)
        .bind(&req.snapshot)
        .bind(&diff_from_prev)
        .bind(&req.change_type)
        .bind(&req.changed_by)
        .bind(&req.change_summary)
        .bind(req.template_id)
        .bind(req.template_version)
        .bind(Utc::now())
        .fetch_one(&self.db_pool)
        .await?;

        Ok(revision)
    }

    /// List revisions for an entity (paginated, newest first).
    pub async fn list_revisions(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<RevisionListResponse> {
        let total: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*) FROM entity_revisions WHERE entity_type = $1 AND entity_id = $2",
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_one(&self.db_pool)
        .await?;

        let revisions = sqlx::query_as::<_, RevisionRow>(
            "SELECT * FROM entity_revisions
             WHERE entity_type = $1 AND entity_id = $2
             ORDER BY version DESC
             LIMIT $3 OFFSET $4",
        )
        .bind(entity_type)
        .bind(entity_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(RevisionListResponse {
            revisions,
            total: total.unwrap_or(0),
        })
    }

    /// Get a specific revision by version.
    pub async fn get_revision(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        version: i32,
    ) -> Result<RevisionRow> {
        sqlx::query_as::<_, RevisionRow>(
            "SELECT * FROM entity_revisions
             WHERE entity_type = $1 AND entity_id = $2 AND version = $3",
        )
        .bind(entity_type)
        .bind(entity_id)
        .bind(version)
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or_else(|| AmosError::NotFound {
            entity: "Revision".to_string(),
            id: format!("{}/{}/v{}", entity_type, entity_id, version),
        })
    }

    /// Get the latest revision for an entity.
    pub async fn get_latest_revision(
        &self,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Option<RevisionRow>> {
        let revision = sqlx::query_as::<_, RevisionRow>(
            "SELECT * FROM entity_revisions
             WHERE entity_type = $1 AND entity_id = $2
             ORDER BY version DESC
             LIMIT 1",
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(revision)
    }

    /// Revert an entity to a previous version (creates a new revision with old snapshot).
    pub async fn revert_to_version(&self, req: RevertRequest) -> Result<RevisionRow> {
        let target_revision = self
            .get_revision(&req.entity_type, req.entity_id, req.target_version)
            .await?;

        let create_req = CreateRevisionRequest {
            entity_type: req.entity_type,
            entity_id: req.entity_id,
            snapshot: target_revision.snapshot,
            change_type: "revert".to_string(),
            changed_by: req.changed_by,
            change_summary: Some(format!("Reverted to version {}", req.target_version)),
            template_id: target_revision.template_id,
            template_version: target_revision.template_version,
        };

        self.create_revision(create_req).await
    }

    /// Compute SHA-256 content hash of a JSON snapshot.
    pub fn compute_content_hash(snapshot: &JsonValue) -> String {
        let canonical_json = serde_json::to_string(snapshot).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(canonical_json.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Compute a simple diff between two JSON snapshots.
    /// Returns `{ "changed": {...}, "added": {...}, "removed": {...} }`.
    pub fn compute_diff(old: &JsonValue, new: &JsonValue) -> Option<JsonValue> {
        if old == new {
            return None;
        }

        let mut diff = serde_json::json!({
            "changed": {},
            "added": {},
            "removed": {}
        });

        match (old.as_object(), new.as_object()) {
            (Some(old_map), Some(new_map)) => {
                for (key, old_value) in old_map.iter() {
                    if let Some(new_value) = new_map.get(key) {
                        if old_value != new_value {
                            diff["changed"][key] = serde_json::json!({
                                "old": old_value,
                                "new": new_value
                            });
                        }
                    } else {
                        diff["removed"][key] = old_value.clone();
                    }
                }
                for (key, new_value) in new_map.iter() {
                    if !old_map.contains_key(key) {
                        diff["added"][key] = new_value.clone();
                    }
                }

                let changed_empty = diff["changed"].as_object().map_or(true, |o| o.is_empty());
                let added_empty = diff["added"].as_object().map_or(true, |o| o.is_empty());
                let removed_empty = diff["removed"].as_object().map_or(true, |o| o.is_empty());

                if changed_empty && added_empty && removed_empty {
                    None
                } else {
                    Some(diff)
                }
            }
            _ => Some(serde_json::json!({
                "changed": { "_root": { "old": old, "new": new } },
                "added": {},
                "removed": {}
            })),
        }
    }
}

// ============================================================================
// TemplateService
// ============================================================================

pub struct TemplateService {
    db_pool: PgPool,
}

impl TemplateService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    /// List all published templates, optionally filtered by entity_type.
    pub async fn list_templates(
        &self,
        entity_type: Option<&str>,
    ) -> Result<Vec<TemplateRegistryRow>> {
        let templates = if let Some(et) = entity_type {
            sqlx::query_as::<_, TemplateRegistryRow>(
                "SELECT * FROM template_registry
                 WHERE is_published = true AND entity_type = $1
                 ORDER BY name",
            )
            .bind(et)
            .fetch_all(&self.db_pool)
            .await?
        } else {
            sqlx::query_as::<_, TemplateRegistryRow>(
                "SELECT * FROM template_registry
                 WHERE is_published = true
                 ORDER BY name",
            )
            .fetch_all(&self.db_pool)
            .await?
        };

        Ok(templates)
    }

    /// Get a specific template by entity_type and slug.
    pub async fn get_template(
        &self,
        entity_type: &str,
        slug: &str,
    ) -> Result<TemplateRegistryRow> {
        sqlx::query_as::<_, TemplateRegistryRow>(
            "SELECT * FROM template_registry WHERE entity_type = $1 AND slug = $2",
        )
        .bind(entity_type)
        .bind(slug)
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or_else(|| AmosError::NotFound {
            entity: "Template".to_string(),
            id: format!("{}/{}", entity_type, slug),
        })
    }

    /// Get all versions of a template.
    pub async fn get_template_versions(
        &self,
        template_id: Uuid,
    ) -> Result<Vec<TemplateVersionRow>> {
        let versions = sqlx::query_as::<_, TemplateVersionRow>(
            "SELECT * FROM template_versions WHERE template_id = $1 ORDER BY version DESC",
        )
        .bind(template_id)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(versions)
    }

    /// Check if an entity's subscribed template has updates available.
    pub async fn check_for_updates(
        &self,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Option<TemplateCheckResult>> {
        let subscription = self.get_subscription(entity_type, entity_id).await?;

        if let Some(sub) = subscription {
            let template = sqlx::query_as::<_, TemplateRegistryRow>(
                "SELECT * FROM template_registry WHERE id = $1",
            )
            .bind(sub.template_id)
            .fetch_optional(&self.db_pool)
            .await?
            .ok_or_else(|| AmosError::NotFound {
                entity: "Template".to_string(),
                id: sub.template_id.to_string(),
            })?;

            let latest = template.current_version;
            let has_update = latest > sub.installed_version;

            let is_breaking = if has_update {
                let breaking_count: Option<i64> = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM template_versions
                     WHERE template_id = $1 AND version > $2 AND version <= $3 AND is_breaking = true",
                )
                .bind(sub.template_id)
                .bind(sub.installed_version)
                .bind(latest)
                .fetch_one(&self.db_pool)
                .await?;
                breaking_count.unwrap_or(0) > 0
            } else {
                false
            };

            Ok(Some(TemplateCheckResult {
                template_slug: template.slug,
                current_version: sub.installed_version,
                latest_version: latest,
                customization_status: sub.customization_status,
                has_update,
                is_breaking,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get a template subscription for an entity.
    pub async fn get_subscription(
        &self,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Option<TemplateSubscriptionRow>> {
        let subscription = sqlx::query_as::<_, TemplateSubscriptionRow>(
            "SELECT * FROM template_subscriptions WHERE entity_type = $1 AND entity_id = $2",
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(subscription)
    }

    /// Update subscription customization status.
    pub async fn update_subscription_status(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        status: &str,
        local_hash: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE template_subscriptions
             SET customization_status = $1, local_content_hash = $2, updated_at = $3
             WHERE entity_type = $4 AND entity_id = $5",
        )
        .bind(status)
        .bind(local_hash)
        .bind(Utc::now())
        .bind(entity_type)
        .bind(entity_id)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────────

    fn test_revision(version: i32, change_type: &str) -> RevisionRow {
        RevisionRow {
            id: Uuid::new_v4(),
            entity_type: "integration".to_string(),
            entity_id: Uuid::new_v4(),
            version,
            content_hash: "abc123".to_string(),
            snapshot: serde_json::json!({"name": "Test", "version": version}),
            diff_from_prev: None,
            change_type: change_type.to_string(),
            changed_by: Some("test_user".to_string()),
            change_summary: Some(format!("v{}", version)),
            template_id: None,
            template_version: None,
            created_at: Utc::now(),
        }
    }

    fn test_template() -> TemplateRegistryRow {
        TemplateRegistryRow {
            id: Uuid::new_v4(),
            entity_type: "integration".to_string(),
            slug: "test-template".to_string(),
            name: "Test Template".to_string(),
            description: Some("A test template".to_string()),
            current_version: 1,
            content_hash: "abc123".to_string(),
            snapshot: serde_json::json!({"name": "Test Template", "config": {}}),
            category: Some("General".to_string()),
            icon_url: None,
            tags: Some(serde_json::json!(["test"])),
            metadata: None,
            is_published: true,
            published_at: Some(Utc::now()),
            deprecated_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_template_version(template_id: Uuid, version: i32) -> TemplateVersionRow {
        TemplateVersionRow {
            id: Uuid::new_v4(),
            template_id,
            version,
            content_hash: format!("hash_v{}", version),
            snapshot: serde_json::json!({"version": version}),
            diff_from_prev: None,
            release_notes: Some(format!("Release v{}", version)),
            is_breaking: false,
            created_at: Utc::now(),
        }
    }

    fn test_subscription(template_id: Uuid) -> TemplateSubscriptionRow {
        TemplateSubscriptionRow {
            id: Uuid::new_v4(),
            entity_type: "integration".to_string(),
            entity_id: Uuid::new_v4(),
            template_id,
            installed_version: 1,
            latest_version: Some(1),
            customization_status: "stock".to_string(),
            local_content_hash: Some("hash_v1".to_string()),
            template_content_hash: Some("hash_v1".to_string()),
            auto_update: false,
            pin_version: false,
            last_checked_at: None,
            last_synced_at: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ── Content Hash Tests ──────────────────────────────────────────────

    #[test]
    fn test_compute_content_hash_consistency() {
        let snapshot = serde_json::json!({
            "name": "Test Entity",
            "value": 42,
            "nested": { "key": "value" }
        });

        let hash1 = RevisionService::compute_content_hash(&snapshot);
        let hash2 = RevisionService::compute_content_hash(&snapshot);

        assert_eq!(hash1, hash2, "Hashes should be consistent");
        assert_eq!(hash1.len(), 64, "SHA-256 hash should be 64 hex chars");
    }

    #[test]
    fn hash_different_snapshots_produce_different_hashes() {
        let snap_a = serde_json::json!({"name": "A"});
        let snap_b = serde_json::json!({"name": "B"});

        let hash_a = RevisionService::compute_content_hash(&snap_a);
        let hash_b = RevisionService::compute_content_hash(&snap_b);

        assert_ne!(hash_a, hash_b, "Different snapshots must have different hashes");
    }

    #[test]
    fn hash_empty_object() {
        let empty = serde_json::json!({});
        let hash = RevisionService::compute_content_hash(&empty);
        assert_eq!(hash.len(), 64);
        // Should be deterministic
        assert_eq!(hash, RevisionService::compute_content_hash(&serde_json::json!({})));
    }

    #[test]
    fn hash_null_value() {
        let null_val = serde_json::json!(null);
        let hash = RevisionService::compute_content_hash(&null_val);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn hash_nested_objects() {
        let snap = serde_json::json!({
            "level1": {
                "level2": {
                    "level3": [1, 2, {"deep": true}]
                }
            }
        });

        let hash1 = RevisionService::compute_content_hash(&snap);
        let hash2 = RevisionService::compute_content_hash(&snap);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn hash_is_lowercase_hex() {
        let snap = serde_json::json!({"key": "value"});
        let hash = RevisionService::compute_content_hash(&snap);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    // ── Diff Tests ──────────────────────────────────────────────────────

    #[test]
    fn test_compute_diff_detects_changes() {
        let old = serde_json::json!({
            "name": "Old Name",
            "value": 10,
            "unchanged": "same"
        });

        let new = serde_json::json!({
            "name": "New Name",
            "value": 20,
            "unchanged": "same",
            "added_field": "new"
        });

        let diff = RevisionService::compute_diff(&old, &new);
        assert!(diff.is_some(), "Diff should be computed");
        let diff = diff.unwrap();

        assert!(diff["changed"]["name"].is_object());
        assert_eq!(diff["changed"]["name"]["old"], "Old Name");
        assert_eq!(diff["changed"]["name"]["new"], "New Name");
        assert!(diff["changed"]["value"].is_object());
        assert_eq!(diff["added"]["added_field"], "new");
        assert!(diff["changed"]["unchanged"].is_null());
    }

    #[test]
    fn test_compute_diff_detects_removed_fields() {
        let old = serde_json::json!({
            "name": "Test",
            "removed_field": "will be gone"
        });
        let new = serde_json::json!({ "name": "Test" });

        let diff = RevisionService::compute_diff(&old, &new);
        assert!(diff.is_some());
        assert_eq!(diff.unwrap()["removed"]["removed_field"], "will be gone");
    }

    #[test]
    fn test_compute_diff_returns_none_for_identical() {
        let snapshot = serde_json::json!({ "name": "Test", "value": 42 });
        let diff = RevisionService::compute_diff(&snapshot, &snapshot);
        assert!(diff.is_none(), "Identical snapshots should have no diff");
    }

    #[test]
    fn diff_all_fields_changed() {
        let old = serde_json::json!({"a": 1, "b": 2, "c": 3});
        let new = serde_json::json!({"a": 10, "b": 20, "c": 30});

        let diff = RevisionService::compute_diff(&old, &new).unwrap();
        let changed = diff["changed"].as_object().unwrap();
        assert_eq!(changed.len(), 3);
        assert!(diff["added"].as_object().unwrap().is_empty());
        assert!(diff["removed"].as_object().unwrap().is_empty());
    }

    #[test]
    fn diff_only_additions() {
        let old = serde_json::json!({});
        let new = serde_json::json!({"new_field": "value", "another": 42});

        let diff = RevisionService::compute_diff(&old, &new).unwrap();
        assert!(diff["changed"].as_object().unwrap().is_empty());
        assert_eq!(diff["added"].as_object().unwrap().len(), 2);
        assert!(diff["removed"].as_object().unwrap().is_empty());
    }

    #[test]
    fn diff_only_removals() {
        let old = serde_json::json!({"removed_a": 1, "removed_b": 2});
        let new = serde_json::json!({});

        let diff = RevisionService::compute_diff(&old, &new).unwrap();
        assert!(diff["changed"].as_object().unwrap().is_empty());
        assert!(diff["added"].as_object().unwrap().is_empty());
        assert_eq!(diff["removed"].as_object().unwrap().len(), 2);
    }

    #[test]
    fn diff_mixed_operations() {
        let old = serde_json::json!({"keep": "same", "change": 1, "remove": "gone"});
        let new = serde_json::json!({"keep": "same", "change": 2, "add": "new"});

        let diff = RevisionService::compute_diff(&old, &new).unwrap();
        assert_eq!(diff["changed"].as_object().unwrap().len(), 1);
        assert_eq!(diff["added"].as_object().unwrap().len(), 1);
        assert_eq!(diff["removed"].as_object().unwrap().len(), 1);
        assert_eq!(diff["changed"]["change"]["old"], 1);
        assert_eq!(diff["changed"]["change"]["new"], 2);
        assert_eq!(diff["added"]["add"], "new");
        assert_eq!(diff["removed"]["remove"], "gone");
    }

    #[test]
    fn diff_nested_value_change_is_shallow() {
        // Diff is shallow — nested changes show full old/new values
        let old = serde_json::json!({"config": {"a": 1, "b": 2}});
        let new = serde_json::json!({"config": {"a": 1, "b": 3}});

        let diff = RevisionService::compute_diff(&old, &new).unwrap();
        assert!(diff["changed"]["config"].is_object());
        assert_eq!(diff["changed"]["config"]["old"], serde_json::json!({"a": 1, "b": 2}));
        assert_eq!(diff["changed"]["config"]["new"], serde_json::json!({"a": 1, "b": 3}));
    }

    #[test]
    fn diff_array_value_change() {
        let old = serde_json::json!({"features": ["a", "b"]});
        let new = serde_json::json!({"features": ["a", "b", "c"]});

        let diff = RevisionService::compute_diff(&old, &new).unwrap();
        assert_eq!(diff["changed"]["features"]["old"], serde_json::json!(["a", "b"]));
        assert_eq!(diff["changed"]["features"]["new"], serde_json::json!(["a", "b", "c"]));
    }

    #[test]
    fn diff_type_change_string_to_number() {
        let old = serde_json::json!({"value": "42"});
        let new = serde_json::json!({"value": 42});

        let diff = RevisionService::compute_diff(&old, &new).unwrap();
        assert_eq!(diff["changed"]["value"]["old"], "42");
        assert_eq!(diff["changed"]["value"]["new"], 42);
    }

    #[test]
    fn diff_non_object_values_use_root() {
        let old = serde_json::json!("old string");
        let new = serde_json::json!("new string");

        let diff = RevisionService::compute_diff(&old, &new).unwrap();
        assert_eq!(diff["changed"]["_root"]["old"], "old string");
        assert_eq!(diff["changed"]["_root"]["new"], "new string");
    }

    #[test]
    fn diff_null_to_object() {
        let old = serde_json::json!(null);
        let new = serde_json::json!({"key": "value"});

        let diff = RevisionService::compute_diff(&old, &new).unwrap();
        assert!(diff["changed"]["_root"].is_object());
    }

    // ── RevisionRow Serialization Tests ─────────────────────────────────

    #[test]
    fn test_revision_row_serialization() {
        let revision = test_revision(1, "create");
        let serialized = serde_json::to_string(&revision).unwrap();
        let deserialized: RevisionRow = serde_json::from_str(&serialized).unwrap();
        assert_eq!(revision.id, deserialized.id);
        assert_eq!(revision.version, deserialized.version);
    }

    #[test]
    fn revision_row_with_all_optional_fields_null() {
        let revision = RevisionRow {
            id: Uuid::new_v4(),
            entity_type: "site".to_string(),
            entity_id: Uuid::new_v4(),
            version: 1,
            content_hash: "hash".to_string(),
            snapshot: serde_json::json!({}),
            diff_from_prev: None,
            change_type: "create".to_string(),
            changed_by: None,
            change_summary: None,
            template_id: None,
            template_version: None,
            created_at: Utc::now(),
        };

        let json = serde_json::to_value(&revision).unwrap();
        assert!(json["changed_by"].is_null());
        assert!(json["change_summary"].is_null());
        assert!(json["template_id"].is_null());
        assert!(json["diff_from_prev"].is_null());
    }

    #[test]
    fn revision_row_with_template_fields_populated() {
        let template_id = Uuid::new_v4();
        let revision = RevisionRow {
            id: Uuid::new_v4(),
            entity_type: "integration".to_string(),
            entity_id: Uuid::new_v4(),
            version: 1,
            content_hash: "hash".to_string(),
            snapshot: serde_json::json!({"name": "Test"}),
            diff_from_prev: Some(serde_json::json!({"changed": {}})),
            change_type: "template_sync".to_string(),
            changed_by: Some("system".to_string()),
            change_summary: Some("Synced from template v2".to_string()),
            template_id: Some(template_id),
            template_version: Some(2),
            created_at: Utc::now(),
        };

        let json = serde_json::to_value(&revision).unwrap();
        assert_eq!(json["template_id"], template_id.to_string());
        assert_eq!(json["template_version"], 2);
        assert_eq!(json["change_type"], "template_sync");
    }

    #[test]
    fn revision_row_with_diff_populated() {
        let diff = serde_json::json!({
            "changed": {"name": {"old": "A", "new": "B"}},
            "added": {},
            "removed": {}
        });
        let mut revision = test_revision(2, "update");
        revision.diff_from_prev = Some(diff.clone());

        let serialized = serde_json::to_string(&revision).unwrap();
        let deserialized: RevisionRow = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.diff_from_prev, Some(diff));
    }

    // ── TemplateRegistryRow Tests ───────────────────────────────────────

    #[test]
    fn test_template_registry_row_serialization() {
        let template = test_template();
        let serialized = serde_json::to_string(&template).unwrap();
        let deserialized: TemplateRegistryRow = serde_json::from_str(&serialized).unwrap();
        assert_eq!(template.id, deserialized.id);
        assert_eq!(template.slug, deserialized.slug);
    }

    #[test]
    fn template_registry_all_optional_null() {
        let template = TemplateRegistryRow {
            id: Uuid::new_v4(),
            entity_type: "canvas".to_string(),
            slug: "minimal".to_string(),
            name: "Minimal".to_string(),
            description: None,
            current_version: 1,
            content_hash: "hash".to_string(),
            snapshot: serde_json::json!({}),
            category: None,
            icon_url: None,
            tags: None,
            metadata: None,
            is_published: false,
            published_at: None,
            deprecated_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_value(&template).unwrap();
        assert!(json["description"].is_null());
        assert!(json["category"].is_null());
        assert!(json["icon_url"].is_null());
        assert!(json["tags"].is_null());
        assert!(json["metadata"].is_null());
        assert!(json["published_at"].is_null());
        assert!(json["deprecated_at"].is_null());
        assert_eq!(json["is_published"], false);
    }

    // ── TemplateVersionRow Tests ────────────────────────────────────────

    #[test]
    fn template_version_row_serialization() {
        let tid = Uuid::new_v4();
        let version = test_template_version(tid, 1);

        let serialized = serde_json::to_string(&version).unwrap();
        let deserialized: TemplateVersionRow = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.template_id, tid);
        assert_eq!(deserialized.version, 1);
        assert!(!deserialized.is_breaking);
    }

    #[test]
    fn template_version_breaking_flag() {
        let tid = Uuid::new_v4();
        let mut version = test_template_version(tid, 2);
        version.is_breaking = true;

        let json = serde_json::to_value(&version).unwrap();
        assert_eq!(json["is_breaking"], true);
    }

    // ── TemplateSubscriptionRow Tests ───────────────────────────────────

    #[test]
    fn template_subscription_row_serialization() {
        let tid = Uuid::new_v4();
        let sub = test_subscription(tid);

        let serialized = serde_json::to_string(&sub).unwrap();
        let deserialized: TemplateSubscriptionRow = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.template_id, tid);
        assert_eq!(deserialized.installed_version, 1);
        assert_eq!(deserialized.customization_status, "stock");
        assert!(!deserialized.auto_update);
        assert!(!deserialized.pin_version);
    }

    #[test]
    fn subscription_customization_statuses() {
        let tid = Uuid::new_v4();
        for status in &["stock", "customized", "outdated", "diverged"] {
            let mut sub = test_subscription(tid);
            sub.customization_status = status.to_string();
            let json = serde_json::to_value(&sub).unwrap();
            assert_eq!(json["customization_status"], *status);
        }
    }

    #[test]
    fn subscription_optional_hash_fields() {
        let tid = Uuid::new_v4();
        let mut sub = test_subscription(tid);
        sub.local_content_hash = None;
        sub.template_content_hash = None;
        sub.latest_version = None;

        let json = serde_json::to_value(&sub).unwrap();
        assert!(json["local_content_hash"].is_null());
        assert!(json["template_content_hash"].is_null());
        assert!(json["latest_version"].is_null());
    }

    // ── Request/Response Types Tests ────────────────────────────────────

    #[test]
    fn test_create_revision_request_serialization() {
        let request = CreateRevisionRequest {
            entity_type: "canvas".to_string(),
            entity_id: Uuid::new_v4(),
            snapshot: serde_json::json!({"name": "Test Canvas"}),
            change_type: "update".to_string(),
            changed_by: "user@example.com".to_string(),
            change_summary: Some("Updated name".to_string()),
            template_id: None,
            template_version: None,
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: CreateRevisionRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(request.entity_type, deserialized.entity_type);
        assert_eq!(request.change_type, deserialized.change_type);
    }

    #[test]
    fn create_revision_request_with_template() {
        let tid = Uuid::new_v4();
        let request = CreateRevisionRequest {
            entity_type: "integration".to_string(),
            entity_id: Uuid::new_v4(),
            snapshot: serde_json::json!({}),
            change_type: "template_sync".to_string(),
            changed_by: "system".to_string(),
            change_summary: None,
            template_id: Some(tid),
            template_version: Some(3),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["template_id"], tid.to_string());
        assert_eq!(json["template_version"], 3);
    }

    #[test]
    fn revert_request_serialization() {
        let eid = Uuid::new_v4();
        let request = RevertRequest {
            entity_type: "canvas".to_string(),
            entity_id: eid,
            target_version: 3,
            changed_by: "admin".to_string(),
        };

        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: RevertRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.entity_id, eid);
        assert_eq!(deserialized.target_version, 3);
        assert_eq!(deserialized.changed_by, "admin");
    }

    #[test]
    fn revision_list_response_serialization() {
        let response = RevisionListResponse {
            revisions: vec![test_revision(2, "update"), test_revision(1, "create")],
            total: 2,
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["total"], 2);
        assert_eq!(json["revisions"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn revision_list_response_empty() {
        let response = RevisionListResponse {
            revisions: vec![],
            total: 0,
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["total"], 0);
        assert!(json["revisions"].as_array().unwrap().is_empty());
    }

    #[test]
    fn template_check_result_no_update() {
        let result = TemplateCheckResult {
            template_slug: "stripe".to_string(),
            current_version: 1,
            latest_version: 1,
            customization_status: "stock".to_string(),
            has_update: false,
            is_breaking: false,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["has_update"], false);
        assert_eq!(json["is_breaking"], false);
        assert_eq!(json["current_version"], json["latest_version"]);
    }

    #[test]
    fn template_check_result_with_breaking_update() {
        let result = TemplateCheckResult {
            template_slug: "stripe".to_string(),
            current_version: 1,
            latest_version: 3,
            customization_status: "customized".to_string(),
            has_update: true,
            is_breaking: true,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["has_update"], true);
        assert_eq!(json["is_breaking"], true);
        assert_eq!(json["current_version"], 1);
        assert_eq!(json["latest_version"], 3);
        assert_eq!(json["customization_status"], "customized");
    }

    // ── Entity Type Coverage ────────────────────────────────────────────

    #[test]
    fn revision_supports_all_entity_types() {
        for entity_type in &["integration", "canvas", "collection", "site", "page"] {
            let mut rev = test_revision(1, "create");
            rev.entity_type = entity_type.to_string();
            let json = serde_json::to_value(&rev).unwrap();
            assert_eq!(json["entity_type"], *entity_type);
        }
    }

    #[test]
    fn change_type_values() {
        for change_type in &["create", "update", "revert", "template_sync", "manual"] {
            let rev = test_revision(1, change_type);
            assert_eq!(rev.change_type, *change_type);
        }
    }
}
