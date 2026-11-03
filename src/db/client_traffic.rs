use std::collections::HashMap;

use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::traffic::counter_delta;

/// One attachment's counters in the latest poll. A direction is `None` when
/// Xray did not return that stat this round, which must not be confused with
/// a real zero.
#[derive(Debug, Clone, Copy)]
pub struct AttachmentSample {
    pub inbound_id: i64,
    pub up: Option<i64>,
    pub down: Option<i64>,
}

/// All attachment samples for one client.
#[derive(Debug, Clone)]
pub struct ClientSamples {
    pub client_id: i64,
    pub samples: Vec<AttachmentSample>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Baseline {
    last_up: i64,
    last_down: i64,
    needs_reconcile: bool,
}

/// Apply one poll for every client in a single SQLite transaction.
///
/// Per attachment the stored baseline is read, the delta computed with
/// [`counter_delta`], the baseline advanced, and the summed deltas added to
/// `clients.up/down`. Because both halves commit together, a crash can never
/// add traffic without recording the matching baseline (which would bill the
/// same bytes twice on the next tick).
///
/// Returns the deltas actually applied per client; clients whose counters did
/// not move are absent.
pub async fn apply_tick(
    db: &SqlitePool,
    clients: &[ClientSamples],
    ts: i64,
) -> Result<HashMap<i64, (i64, i64)>, sqlx::Error> {
    let mut applied = HashMap::new();
    if clients.is_empty() {
        return Ok(applied);
    }

    let mut tx = db.begin().await?;
    let rows: Vec<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT client_id, inbound_id, last_up, last_down, needs_reconcile
         FROM client_inbound_traffic",
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut baselines = HashMap::with_capacity(rows.len());
    for (client_id, inbound_id, last_up, last_down, needs_reconcile) in rows {
        baselines.insert(
            (client_id, inbound_id),
            Baseline {
                last_up,
                last_down,
                needs_reconcile: needs_reconcile != 0,
            },
        );
    }

    for plan in clients {
        let mut total_up: i64 = 0;
        let mut total_down: i64 = 0;

        for sample in &plan.samples {
            let key = (plan.client_id, sample.inbound_id);
            let base = baselines.get(&key).copied().unwrap_or_default();
            let mut next_up = base.last_up.max(0);
            let mut next_down = base.last_down.max(0);
            let mut changed = false;

            if base.needs_reconcile {
                // The last scheduled reset only partly succeeded (or timed
                // out): adopt the current counters as the new baseline and
                // never bill the gap, so an unknown reset outcome cannot be
                // counted twice. Wait for at least one real direction before
                // dropping the flag, otherwise a later sample would be billed
                // against the pre-reset baseline.
                let mut adopted = false;
                if let Some(up) = sample.up.filter(|value| *value >= 0) {
                    next_up = up;
                    adopted = true;
                }
                if let Some(down) = sample.down.filter(|value| *value >= 0) {
                    next_down = down;
                    adopted = true;
                }
                if !adopted {
                    continue;
                }
                changed = true;
            } else {
                if let Some((delta, next)) = counter_delta(sample.up, base.last_up) {
                    total_up = total_up.saturating_add(delta);
                    if next != next_up {
                        next_up = next;
                        changed = true;
                    }
                }
                if let Some((delta, next)) = counter_delta(sample.down, base.last_down) {
                    total_down = total_down.saturating_add(delta);
                    if next != next_down {
                        next_down = next;
                        changed = true;
                    }
                }
            }

            if changed {
                upsert_baseline(
                    &mut tx,
                    plan.client_id,
                    sample.inbound_id,
                    next_up,
                    next_down,
                    false,
                    ts,
                )
                .await?;
            }
        }

        if total_up > 0 || total_down > 0 {
            sqlx::query(
                "UPDATE clients SET up = up + ?, down = down + ?, last_online = ?,
                 updated_at = ? WHERE id = ?",
            )
            .bind(total_up)
            .bind(total_down)
            .bind(ts)
            .bind(ts)
            .bind(plan.client_id)
            .execute(&mut *tx)
            .await?;
            applied.insert(plan.client_id, (total_up, total_down));
        }
    }

    tx.commit().await?;
    Ok(applied)
}

async fn upsert_baseline(
    tx: &mut Transaction<'_, Sqlite>,
    client_id: i64,
    inbound_id: i64,
    last_up: i64,
    last_down: i64,
    needs_reconcile: bool,
    ts: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO client_inbound_traffic
           (client_id, inbound_id, last_up, last_down, needs_reconcile, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(client_id, inbound_id) DO UPDATE SET
           last_up = excluded.last_up,
           last_down = excluded.last_down,
           needs_reconcile = excluded.needs_reconcile,
           updated_at = excluded.updated_at",
    )
    .bind(client_id)
    .bind(inbound_id)
    .bind(last_up)
    .bind(last_down)
    .bind(needs_reconcile)
    .bind(ts)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Drop every stored counter baseline after Xray changed generation. Only
/// `last_*` values are touched; the accumulated `up/down` totals stay intact.
pub async fn reset_all_baselines(db: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    for statement in [
        "UPDATE clients SET last_up = 0, last_down = 0",
        "UPDATE inbounds SET last_up = 0, last_down = 0",
        "UPDATE outbound_traffics SET last_up = 0, last_down = 0",
        "UPDATE client_inbound_traffic SET last_up = 0, last_down = 0, needs_reconcile = 0",
    ] {
        sqlx::query(statement).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Clear one client's totals and attachment baselines after every attachment
/// reset was confirmed by Xray. Callers must not use this on a partial reset:
/// zeroing the database while a counter still holds its old value would bill
/// that value again.
pub async fn reset_client_verified(
    db: &SqlitePool,
    client_id: i64,
    ts: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    sqlx::query(
        "UPDATE clients SET up = 0, down = 0, last_up = 0, last_down = 0,
         last_reset_time = ?, updated_at = ? WHERE id = ?",
    )
    .bind(ts)
    .bind(ts)
    .bind(client_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE client_inbound_traffic SET last_up = 0, last_down = 0,
         needs_reconcile = 0, updated_at = ? WHERE client_id = ?",
    )
    .bind(ts)
    .bind(client_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Flag every attachment of a client for reconciliation without touching the
/// stored baselines or totals.
pub async fn mark_reconcile(
    db: &SqlitePool,
    client_id: i64,
    inbound_ids: &[i64],
    ts: i64,
) -> Result<(), sqlx::Error> {
    if inbound_ids.is_empty() {
        return Ok(());
    }
    let mut tx = db.begin().await?;
    for inbound_id in inbound_ids {
        sqlx::query(
            "INSERT INTO client_inbound_traffic
               (client_id, inbound_id, last_up, last_down, needs_reconcile, updated_at)
             VALUES (?, ?, 0, 0, 1, ?)
             ON CONFLICT(client_id, inbound_id) DO UPDATE SET
               needs_reconcile = 1,
               updated_at = excluded.updated_at",
        )
        .bind(client_id)
        .bind(inbound_id)
        .bind(ts)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn setup() -> SqlitePool {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("sqlite options")
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("open in-memory sqlite");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");
        sqlx::query(
            "INSERT INTO inbounds (id, port, protocol, tag, created_at, updated_at)
             VALUES (1, 443, 'vless', 'in-a', 1, 1), (2, 8443, 'vless', 'in-b', 1, 1)",
        )
        .execute(&pool)
        .await
        .expect("seed inbounds");
        sqlx::query(
            "INSERT INTO clients (id, inbound_id, email, created_at, updated_at)
             VALUES (1, 1, 'alice', 1, 1)",
        )
        .execute(&pool)
        .await
        .expect("seed client");
        sqlx::query("INSERT INTO client_inbounds (client_id, inbound_id) VALUES (1, 1), (1, 2)")
            .execute(&pool)
            .await
            .expect("seed associations");
        pool
    }

    fn plan(samples: &[(i64, Option<i64>, Option<i64>)]) -> Vec<ClientSamples> {
        vec![ClientSamples {
            client_id: 1,
            samples: samples
                .iter()
                .map(|(inbound_id, up, down)| AttachmentSample {
                    inbound_id: *inbound_id,
                    up: *up,
                    down: *down,
                })
                .collect(),
        }]
    }

    async fn totals(pool: &SqlitePool) -> (i64, i64) {
        sqlx::query_as("SELECT up, down FROM clients WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn baselines(pool: &SqlitePool) -> Vec<(i64, i64, i64, i64)> {
        sqlx::query_as(
            "SELECT inbound_id, last_up, last_down, needs_reconcile
             FROM client_inbound_traffic WHERE client_id = 1 ORDER BY inbound_id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn accumulates_each_attachment_independently() {
        let pool = setup().await;
        let applied = apply_tick(
            &pool,
            &plan(&[(1, Some(100), Some(200)), (2, Some(50), Some(10))]),
            1_000,
        )
        .await
        .unwrap();
        assert_eq!(applied.get(&1), Some(&(150, 210)));
        assert_eq!(totals(&pool).await, (150, 210));

        // One attachment resets while the other keeps growing: only the fresh
        // value of the reset attachment is billed.
        let applied = apply_tick(
            &pool,
            &plan(&[(1, Some(0), Some(0)), (2, Some(70), Some(40))]),
            1_005,
        )
        .await
        .unwrap();
        assert_eq!(applied.get(&1), Some(&(20, 30)));
        assert_eq!(totals(&pool).await, (170, 240));
        assert_eq!(baselines(&pool).await, vec![(1, 0, 0, 0), (2, 70, 40, 0)]);
    }

    #[tokio::test]
    async fn missing_direction_preserves_its_baseline() {
        let pool = setup().await;
        apply_tick(&pool, &plan(&[(1, Some(100), Some(200))]), 1_000)
            .await
            .unwrap();

        // Xray omitted the uplink stat: the download delta is still billed and
        // the uplink baseline must stay at 100 instead of falling to zero.
        let applied = apply_tick(&pool, &plan(&[(1, None, Some(260))]), 1_005)
            .await
            .unwrap();
        assert_eq!(applied.get(&1), Some(&(0, 60)));
        assert_eq!(baselines(&pool).await, vec![(1, 100, 260, 0)]);

        // Mirror case: the downlink stat is missing, so last_down must stay
        // at 260 while the uplink delta is billed normally.
        let applied = apply_tick(&pool, &plan(&[(1, Some(140), None)]), 1_010)
            .await
            .unwrap();
        assert_eq!(applied.get(&1), Some(&(40, 0)));
        assert_eq!(baselines(&pool).await, vec![(1, 140, 260, 0)]);
    }

    #[tokio::test]
    async fn a_failed_attachment_write_rolls_back_the_whole_tick() {
        let pool = setup().await;
        // Inbound 999 does not exist; with foreign keys on, upserting its
        // baseline fails after the valid attachment was already written.
        let applied = apply_tick(
            &pool,
            &plan(&[(1, Some(100), Some(100)), (999, Some(5), Some(5))]),
            1_000,
        )
        .await;
        assert!(applied.is_err(), "foreign-key violation must fail the tick");
        // Nothing from the failed transaction may survive: no total, no
        // baseline for the attachment that was written before the failure.
        assert_eq!(totals(&pool).await, (0, 0));
        assert!(baselines(&pool).await.is_empty());
    }

    #[tokio::test]
    async fn partial_reset_is_reconciled_without_billing_the_gap() {
        let pool = setup().await;
        apply_tick(&pool, &plan(&[(1, Some(500), Some(500))]), 1_000)
            .await
            .unwrap();

        mark_reconcile(&pool, 1, &[1, 2], 1_010).await.unwrap();

        // A missing sample must keep the reconcile flag.
        apply_tick(&pool, &plan(&[(1, None, None)]), 1_015)
            .await
            .unwrap();
        assert_eq!(baselines(&pool).await[0].3, 1);

        // First real sample after the failed reset: adopt it, bill nothing,
        // clear the flag.
        let applied = apply_tick(&pool, &plan(&[(1, Some(900), Some(950))]), 1_020)
            .await
            .unwrap();
        assert!(applied.is_empty());
        // Attachment 1 adopted its sample; attachment 2 was not reported this
        // round, so its flag stays until it has a real sample to adopt.
        assert_eq!(baselines(&pool).await, vec![(1, 900, 950, 0), (2, 0, 0, 1)]);
        assert_eq!(totals(&pool).await, (500, 500));

        // Normal accounting resumes from the adopted baseline.
        let applied = apply_tick(&pool, &plan(&[(1, Some(950), Some(1_000))]), 1_025)
            .await
            .unwrap();
        assert_eq!(applied.get(&1), Some(&(50, 50)));
    }

    #[tokio::test]
    async fn reset_all_baselines_keeps_accumulated_totals() {
        let pool = setup().await;
        apply_tick(&pool, &plan(&[(1, Some(100), Some(100))]), 1_000)
            .await
            .unwrap();

        reset_all_baselines(&pool).await.unwrap();

        assert_eq!(totals(&pool).await, (100, 100));
        assert_eq!(baselines(&pool).await, vec![(1, 0, 0, 0)]);
        let (last_up, last_down): (i64, i64) =
            sqlx::query_as("SELECT last_up, last_down FROM clients WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((last_up, last_down), (0, 0));
    }

    #[tokio::test]
    async fn verified_reset_clears_totals_and_baselines() {
        let pool = setup().await;
        apply_tick(&pool, &plan(&[(1, Some(100), Some(100))]), 1_000)
            .await
            .unwrap();
        mark_reconcile(&pool, 1, &[1], 1_010).await.unwrap();

        reset_client_verified(&pool, 1, 1_020).await.unwrap();

        assert_eq!(totals(&pool).await, (0, 0));
        assert_eq!(baselines(&pool).await, vec![(1, 0, 0, 0)]);
        let (last_reset, last_up): (i64, i64) =
            sqlx::query_as("SELECT last_reset_time, last_up FROM clients WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((last_reset, last_up), (1_020, 0));
    }
}
