//! `DbEvidenceSource` against a real (in-memory) SQLite pool.
//!
//! Seeds the compliance-evidence tables from migration 0003, then asserts
//! each `EvidenceSource` query returns the expected answer — and that a
//! real probe (ColdChainProbe) driven by the DB source reaches the right
//! verdict. This is the proof that the probes can run against production
//! data, not just `MockEvidenceSource`.

use aether_compliance::{ColdChainProbe, DbEvidenceSource, EvidenceSource, Probe, Verdict};
use aether_db::Pool;
use chrono::{Duration, Utc};
use std::sync::Arc;

async fn seed(pool: &Pool) {
    let now = Utc::now();
    let h = pool.handle();

    // audit_log_mutations: one attempted delete, just now.
    sqlx::query("INSERT INTO audit_log_mutations (attempted_at, op) VALUES (?, 'delete')")
        .bind(now.to_rfc3339())
        .execute(h)
        .await
        .unwrap();

    // sync_outbox: 2 encrypted writes + 1 plaintext, all just now.
    for (i, enc) in [(0, 1), (1, 1), (2, 0)] {
        sqlx::query(
            "INSERT INTO sync_outbox (op_id, entity, entity_id, op, hlc_ts, encrypted, created_at) \
             VALUES (?, 'materials', 'm1', 'update', 'hlc', ?, ?)",
        )
        .bind(format!("op-{i}"))
        .bind(enc)
        .bind(now.timestamp())
        .execute(h)
        .await
        .unwrap();
    }

    // security_incidents: one open & old (past SLA), one resolved, one open & recent.
    sqlx::query(
        "INSERT INTO security_incidents (id, opened_at, severity) VALUES ('inc-old', ?, 'high')",
    )
    .bind((now - Duration::hours(100)).to_rfc3339())
    .execute(h)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO security_incidents (id, opened_at, severity, resolved_at) \
         VALUES ('inc-done', ?, 'low', ?)",
    )
    .bind((now - Duration::hours(100)).to_rfc3339())
    .bind(now.to_rfc3339())
    .execute(h)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO security_incidents (id, opened_at, severity) VALUES ('inc-new', ?, 'medium')",
    )
    .bind((now - Duration::hours(10)).to_rfc3339())
    .execute(h)
    .await
    .unwrap();

    // key_rotation_events: an old one and a recent one.
    for d in [200_i64, 10] {
        sqlx::query("INSERT INTO key_rotation_events (rotated_at) VALUES (?)")
            .bind((now - Duration::days(d)).to_rfc3339())
            .execute(h)
            .await
            .unwrap();
    }

    // cold_chain_excursions: 3 in the last day.
    for i in 0..3 {
        sqlx::query("INSERT INTO cold_chain_excursions (occurred_at) VALUES (?)")
            .bind((now - Duration::hours(i + 1)).to_rfc3339())
            .execute(h)
            .await
            .unwrap();
    }

    // periodic_reviews: an access review 15 days ago.
    sqlx::query(
        "INSERT INTO periodic_reviews (control_id, reviewed_at) VALUES ('is-access-review', ?)",
    )
    .bind((now - Duration::days(15)).to_rfc3339())
    .execute(h)
    .await
    .unwrap();

    // data_breaches: one overdue+unnotified, one notified, one too-recent.
    sqlx::query("INSERT INTO data_breaches (id, detected_at) VALUES ('br-overdue', ?)")
        .bind((now - Duration::hours(100)).to_rfc3339())
        .execute(h)
        .await
        .unwrap();
    sqlx::query("INSERT INTO data_breaches (id, detected_at, notified_at) VALUES ('br-ok', ?, ?)")
        .bind((now - Duration::hours(100)).to_rfc3339())
        .bind((now - Duration::hours(90)).to_rfc3339())
        .execute(h)
        .await
        .unwrap();
    sqlx::query("INSERT INTO data_breaches (id, detected_at) VALUES ('br-recent', ?)")
        .bind((now - Duration::hours(1)).to_rfc3339())
        .execute(h)
        .await
        .unwrap();

    // dsar_requests: one overdue, one fulfilled, one recent.
    sqlx::query("INSERT INTO dsar_requests (id, requested_at) VALUES ('dsar-overdue', ?)")
        .bind((now - Duration::days(40)).to_rfc3339())
        .execute(h)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO dsar_requests (id, requested_at, fulfilled_at) VALUES ('dsar-ok', ?, ?)",
    )
    .bind((now - Duration::days(40)).to_rfc3339())
    .bind((now - Duration::days(35)).to_rfc3339())
    .execute(h)
    .await
    .unwrap();
    sqlx::query("INSERT INTO dsar_requests (id, requested_at) VALUES ('dsar-recent', ?)")
        .bind((now - Duration::days(5)).to_rfc3339())
        .execute(h)
        .await
        .unwrap();

    // electronic_signatures: 2 bound, 1 unbound.
    for (i, bound) in [(0, 1), (1, 1), (2, 0)] {
        sqlx::query(
            "INSERT INTO electronic_signatures (id, record_ref, bound, signed_at) \
             VALUES (?, 'rec', ?, ?)",
        )
        .bind(format!("sig-{i}"))
        .bind(bound)
        .bind(now.to_rfc3339())
        .execute(h)
        .await
        .unwrap();
    }
}

async fn seeded_source() -> DbEvidenceSource {
    let pool = Pool::open_in_memory().await.expect("in-memory pool");
    seed(&pool).await;
    DbEvidenceSource::new(pool)
}

#[tokio::test]
async fn fresh_db_reports_clean() {
    let pool = Pool::open_in_memory().await.unwrap();
    let src = DbEvidenceSource::new(pool);
    let now = Utc::now();

    assert!(!src
        .audit_log_tampered_since(now - Duration::hours(1))
        .await
        .unwrap());
    assert_eq!(
        src.encrypted_column_writes_since(now - Duration::hours(1))
            .await
            .unwrap(),
        0
    );
    assert!(src.open_incidents_past_sla(72).await.unwrap().is_empty());
    assert_eq!(src.latest_key_rotation().await.unwrap(), None);
    assert_eq!(
        src.cold_chain_excursions_since(now - Duration::days(1))
            .await
            .unwrap(),
        0
    );
    assert_eq!(src.latest_review("is-access-review").await.unwrap(), None);
    assert_eq!(src.breaches_unnotified_within(72).await.unwrap(), 0);
    assert_eq!(src.dsars_past_deadline(30).await.unwrap(), 0);
    assert_eq!(src.unbound_signatures().await.unwrap(), 0);
}

#[tokio::test]
async fn seeded_db_answers_each_primitive() {
    let src = seeded_source().await;
    let now = Utc::now();

    assert!(src
        .audit_log_tampered_since(now - Duration::hours(1))
        .await
        .unwrap());
    assert_eq!(
        src.encrypted_column_writes_since(now - Duration::hours(1))
            .await
            .unwrap(),
        2
    );

    let incidents = src.open_incidents_past_sla(72).await.unwrap();
    assert_eq!(incidents.len(), 1);
    assert_eq!(incidents[0].id, "inc-old");

    let rot = src
        .latest_key_rotation()
        .await
        .unwrap()
        .expect("a rotation exists");
    // Most recent is the 10-day-old one, well within a day of `now - 10d`.
    assert!((now - rot).num_days().abs() <= 11);

    assert_eq!(
        src.cold_chain_excursions_since(now - Duration::days(1))
            .await
            .unwrap(),
        3
    );
    assert!(src
        .latest_review("is-access-review")
        .await
        .unwrap()
        .is_some());
    assert_eq!(src.latest_review("qms-internal-audit").await.unwrap(), None);
    assert_eq!(src.breaches_unnotified_within(72).await.unwrap(), 1);
    assert_eq!(src.dsars_past_deadline(30).await.unwrap(), 1);
    assert_eq!(src.unbound_signatures().await.unwrap(), 1);
}

#[tokio::test]
async fn since_cutoff_excludes_older_rows() {
    let src = seeded_source().await;
    // A cutoff in the future excludes everything.
    let future = Utc::now() + Duration::days(1);
    assert_eq!(src.cold_chain_excursions_since(future).await.unwrap(), 0);
    assert_eq!(src.encrypted_column_writes_since(future).await.unwrap(), 0);
}

#[tokio::test]
async fn real_probe_runs_against_the_db_source() {
    // ColdChainProbe over a DB with 3 excursions in the window -> Fail.
    let pool = Pool::open_in_memory().await.unwrap();
    seed(&pool).await;
    let src: Arc<dyn EvidenceSource> = Arc::new(DbEvidenceSource::new(pool));
    let probe = ColdChainProbe::new(src, Utc::now() - Duration::days(1));
    assert_eq!(probe.evaluate().await.unwrap().0, Verdict::Fail);
}
