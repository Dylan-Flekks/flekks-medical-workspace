use super::*;
use crate::StateRuntime;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;
use sqlx::QueryBuilder;
use sqlx::Row;
use sqlx::Sqlite;

async fn runtime() -> std::sync::Arc<StateRuntime> {
    StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state db should initialize")
}

#[tokio::test]
async fn synthetic_provisioning_is_explicit_durable_and_idempotent() {
    let runtime = runtime().await;
    let initial = runtime
        .workspace()
        .workspace_data_policy_status()
        .await
        .expect("policy should read");
    assert_eq!(
        initial,
        crate::WorkspaceDataPolicyStatus {
            schema_version: 1,
            data_classification: crate::WorkspaceDataClassification::Unclassified,
            classified_at: None,
            classified_by: None,
        }
    );
    let oversized = "x".repeat(MAX_CLASSIFIED_BY_BYTES + 1);
    let error = runtime
        .workspace()
        .provision_synthetic_workspace(&oversized)
        .await
        .expect_err("oversized provenance must fail");
    assert!(error.to_string().contains("256 byte limit"));
    assert_eq!(
        runtime
            .workspace()
            .workspace_data_policy_status()
            .await
            .expect("rejected provisioning must preserve policy"),
        initial
    );

    let provisioned = runtime
        .workspace()
        .provision_synthetic_workspace("  local synthetic-data launcher  ")
        .await
        .expect("empty workspace should provision");
    let crate::WorkspaceSyntheticProvisionOutcome::Provisioned(status) = provisioned else {
        panic!("first provisioning must report a transition");
    };
    assert_eq!(
        status.data_classification,
        crate::WorkspaceDataClassification::Synthetic
    );
    assert!(status.classified_at.is_some());
    assert_eq!(
        status.classified_by.as_deref(),
        Some("local synthetic-data launcher")
    );
    assert_eq!(
        runtime
            .workspace()
            .workspace_data_policy_status()
            .await
            .expect("provisioned policy should read"),
        status
    );

    assert_eq!(
        runtime
            .workspace()
            .provision_synthetic_workspace("different retry source")
            .await
            .expect("synthetic provisioning should be idempotent"),
        crate::WorkspaceSyntheticProvisionOutcome::AlreadySynthetic(status)
    );
}

#[tokio::test]
async fn every_workspace_domain_table_blocks_api_and_direct_provisioning() {
    let runtime = runtime().await;
    let mut connection = runtime
        .workspace()
        .pool
        .acquire()
        .await
        .expect("workspace connection");
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .expect("foreign key fixture mode");
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&mut *connection)
        .await
        .expect("check fixture mode");

    for table in WORKSPACE_DOMAIN_TABLES {
        insert_placeholder(&mut connection, table).await;
        let direct = sqlx::query(
            "UPDATE workspace_data_policy SET data_classification = 'synthetic', classified_at_ms = 1, classified_by = 'direct fixture' WHERE singleton_id = 1",
        )
        .execute(&mut *connection)
        .await;
        assert!(direct.is_err(), "{table} must block the SQL transition");
        let error = runtime
            .workspace()
            .provision_synthetic_workspace("API fixture")
            .await
            .expect_err("domain data must block API provisioning");
        assert!(
            error.to_string().contains("workspace records exist"),
            "unexpected {table} error: {error}"
        );
        let mut delete = QueryBuilder::<Sqlite>::new("DELETE FROM ");
        delete.push(table);
        delete
            .build()
            .execute(&mut *connection)
            .await
            .unwrap_or_else(|error| panic!("delete {table} placeholder: {error}"));
    }
}

#[tokio::test]
async fn policy_schema_and_trigger_cover_the_exact_workspace_domain() {
    let runtime = runtime().await;
    let actual = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND name LIKE 'workspace_%' AND name != 'workspace_data_policy' ORDER BY name",
    )
    .fetch_all(runtime.workspace().pool.as_ref())
    .await
    .expect("workspace schema should list");
    let mut expected = WORKSPACE_DOMAIN_TABLES
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected);

    let trigger_sql = sqlx::query_scalar::<_, String>(
        "SELECT sql FROM sqlite_schema WHERE type = 'trigger' AND name = 'workspace_data_policy_restrict_update'",
    )
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .expect("policy trigger should exist");
    for table in WORKSPACE_DOMAIN_TABLES {
        assert!(
            trigger_sql.contains(table),
            "policy trigger must cover {table}"
        );
    }
}

#[tokio::test]
async fn policy_singleton_rejects_delete_replace_tamper_and_downgrade() {
    let runtime = runtime().await;
    let pool = runtime.workspace().pool.as_ref();
    for statement in [
        "DELETE FROM workspace_data_policy",
        "INSERT OR REPLACE INTO workspace_data_policy (singleton_id, schema_version, data_classification, classified_at_ms, classified_by) VALUES (1, 1, 'unclassified', NULL, NULL)",
        "UPDATE workspace_data_policy SET singleton_id = 2",
        "UPDATE workspace_data_policy SET schema_version = 2",
        "UPDATE workspace_data_policy SET data_classification = 'synthetic', classified_at_ms = 1, classified_by = ' padded '",
        "UPDATE workspace_data_policy SET data_classification = 'synthetic', classified_at_ms = -1, classified_by = 'fixture'",
        "UPDATE workspace_data_policy SET data_classification = 'synthetic', classified_at_ms = 1, classified_by = NULL",
    ] {
        assert!(
            sqlx::query(statement).execute(pool).await.is_err(),
            "policy mutation must fail: {statement}"
        );
    }

    runtime
        .workspace()
        .provision_synthetic_workspace("mutation fixture")
        .await
        .expect("empty workspace should provision");
    for statement in [
        "UPDATE workspace_data_policy SET data_classification = 'unclassified', classified_at_ms = NULL, classified_by = NULL",
        "UPDATE workspace_data_policy SET classified_by = 'replacement source'",
        "UPDATE workspace_data_policy SET schema_version = 2",
        "UPDATE workspace_data_policy SET singleton_id = 2",
    ] {
        assert!(
            sqlx::query(statement).execute(pool).await.is_err(),
            "provisioned policy mutation must fail: {statement}"
        );
    }
}

#[tokio::test]
async fn policy_reads_fail_closed_on_missing_corrupt_multiple_or_drifted_schema() {
    let missing = runtime().await;
    sqlx::query("DROP TRIGGER workspace_data_policy_reject_delete")
        .execute(missing.workspace().pool.as_ref())
        .await
        .expect("drop delete guard fixture");
    sqlx::query("DELETE FROM workspace_data_policy")
        .execute(missing.workspace().pool.as_ref())
        .await
        .expect("delete policy fixture");
    assert!(
        missing
            .workspace()
            .workspace_data_policy_status()
            .await
            .unwrap_err()
            .to_string()
            .contains("exactly one row")
    );

    let corrupt = runtime().await;
    let mut corrupt_connection = corrupt_policy_guards(&corrupt).await;
    sqlx::query("UPDATE workspace_data_policy SET data_classification = 'unknown', classified_at_ms = 1, classified_by = 'tamper'")
        .execute(&mut *corrupt_connection)
        .await
        .expect("corrupt classification fixture");
    assert!(
        corrupt
            .workspace()
            .workspace_data_policy_status()
            .await
            .unwrap_err()
            .to_string()
            .contains("unknown stored")
    );

    let multiple = runtime().await;
    let mut multiple_connection = multiple
        .workspace()
        .pool
        .acquire()
        .await
        .expect("multiple policy fixture connection");
    sqlx::query("DROP TRIGGER workspace_data_policy_reject_insert")
        .execute(&mut *multiple_connection)
        .await
        .expect("drop insert guard fixture");
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&mut *multiple_connection)
        .await
        .expect("multiple-row fixture mode");
    sqlx::query("INSERT INTO workspace_data_policy (singleton_id, schema_version, data_classification, classified_at_ms, classified_by) VALUES (2, 1, 'unclassified', NULL, NULL)")
        .execute(&mut *multiple_connection)
        .await
        .expect("second policy row fixture");
    assert!(
        multiple
            .workspace()
            .workspace_data_policy_status()
            .await
            .unwrap_err()
            .to_string()
            .contains("exactly one row")
    );

    let unknown_schema = runtime().await;
    sqlx::query("CREATE TABLE workspace_future_records (id TEXT PRIMARY KEY)")
        .execute(unknown_schema.workspace().pool.as_ref())
        .await
        .expect("future schema fixture");
    assert!(
        unknown_schema
            .workspace()
            .provision_synthetic_workspace("schema drift fixture")
            .await
            .unwrap_err()
            .to_string()
            .contains("does not match")
    );

    let missing_schema = runtime().await;
    sqlx::query("DROP TABLE workspace_guide_runs")
        .execute(missing_schema.workspace().pool.as_ref())
        .await
        .expect("missing schema fixture");
    assert!(
        missing_schema
            .workspace()
            .workspace_data_policy_status()
            .await
            .unwrap_err()
            .to_string()
            .contains("does not match")
    );
}

async fn insert_placeholder(connection: &mut sqlx::SqliteConnection, table: &str) {
    let mut table_info = QueryBuilder::<Sqlite>::new("PRAGMA table_info(");
    table_info.push(table).push(")");
    let columns = table_info
        .build()
        .fetch_all(&mut *connection)
        .await
        .unwrap_or_else(|error| panic!("inspect {table}: {error}"))
        .into_iter()
        .filter_map(|row| {
            let not_null: bool = row.try_get("notnull").expect("notnull");
            let primary_key: i64 = row.try_get("pk").expect("pk");
            (not_null || primary_key > 0).then(|| {
                let name: String = row.try_get("name").expect("name");
                let kind: String = row.try_get("type").expect("type");
                (name, kind.to_ascii_uppercase().contains("INT"))
            })
        })
        .collect::<Vec<_>>();
    let mut query = QueryBuilder::<Sqlite>::new("INSERT INTO ");
    query.push(table).push(" (");
    {
        let mut separated = query.separated(", ");
        for (name, _) in &columns {
            separated.push(name);
        }
    }
    query.push(") VALUES (");
    {
        let mut separated = query.separated(", ");
        for (_, integer) in columns {
            if integer {
                separated.push_bind(1_i64);
            } else {
                separated.push_bind("x");
            }
        }
    }
    query.push(")");
    query
        .build()
        .execute(&mut *connection)
        .await
        .unwrap_or_else(|error| panic!("insert {table} placeholder: {error}"));
}

async fn corrupt_policy_guards(runtime: &StateRuntime) -> sqlx::pool::PoolConnection<Sqlite> {
    let mut connection = runtime
        .workspace()
        .pool
        .acquire()
        .await
        .expect("corrupt policy fixture connection");
    sqlx::query("DROP TRIGGER workspace_data_policy_restrict_update")
        .execute(&mut *connection)
        .await
        .expect("drop update guard fixture");
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&mut *connection)
        .await
        .expect("corrupt policy fixture mode");
    connection
}
