use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use chrono::{Datelike, Local, NaiveDate, TimeZone, Timelike};
use serde::Serialize;
use tokio::sync::{Mutex, Notify};

use crate::{
    db::{
        self,
        client_traffic::{AttachmentSample, ClientSamples},
        models::{Client, Inbound},
    },
    service,
    state::{AppState, SpeedSample, TrafficPoint, TrafficSpeeds},
    traffic::counter_delta,
    xray::{identity::stat_email, stats::FamilyStats},
};

const MAX_HISTORY: usize = 240;
// 3x-ui runs a 5s traffic poll with a 20s online grace (onlineGracePeriodMs).
// Scale the same window with our configurable poll so a client stays listed
// between polls, then drops once it has really gone quiet. The cap matters
// when a slow poll was configured: a 10-minute grace made disconnected
// clients look online long after they were gone.
const MIN_ONLINE_GRACE_SECS: i64 = 20;
const MAX_ONLINE_GRACE_SECS: i64 = 60;

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        // `None` forces the first tick to initialise every counter baseline
        // from the current Xray process, even if the panel missed the start
        // event. Xray is our child, so its counters restart with the panel.
        let mut generation: Option<u64> = None;
        // Epoch of the process whose counters the stored baselines belong to.
        // The generation also moves on stop, so it cannot tell "a new process
        // started" from "the old one is going away"; this only moves on start.
        let mut stats_epoch: Option<u64> = None;
        // Monotonic timestamp of the previous committed stats sample. Rates
        // are billed deltas divided by this real interval, not by the
        // configured poll length: a slow tick or a missed RPC must not skew
        // the speed, and a wall-clock jump must not either.
        let mut last_stats_at: Option<std::time::Instant> = None;
        // 3x-ui keeps the live-display delta baseline in memory
        // (XrayAPI.StatsLastValues): it advances on every successful query and
        // is decoupled from the database, so a failed DB billing write can
        // never blank the speed columns or the WS frame. The DB baselines
        // stay authoritative for billing on their own transactional path.
        let mut stats_last: HashMap<DeltaKey, i64> = HashMap::new();
        // Per-family first-pass flag (3x-ui statsBaselined): the first
        // successful sample of a family only records baselines, since the
        // counters may already hold traffic the panel cannot attribute.
        let mut stats_baselined: HashSet<&'static str> = HashSet::new();
        // 3x-ui runs its DB accounting on a serialized writer goroutine
        // (`submitTrafficWrite`) instead of inside the stats poll, so a slow
        // write cannot stretch the gap between samples. Ours is the same task
        // with a coalescing single-slot queue: raw counters make an older
        // sample redundant, keeping the extra memory to one sample.
        let billing = BillingSink::new();
        tokio::spawn(run_billing_writer(state.clone(), billing.clone()));
        loop {
            // Xray restarts (manual, config apply, crash) drop every live
            // connection. 3x-ui keeps its online map inside the Xray process;
            // ours lives in the panel, so clear it here instead of showing
            // ghosts until the grace expires.
            let current = state.xray.generation();
            if generation != Some(current) {
                clear_online_state(&state).await;
                state.stats.invalidate_online_api_probe();
                *state.traffic_speeds.lock().await = TrafficSpeeds::default();
                generation = Some(current);
            }

            // A new process starts with zero counters, so every stored
            // baseline belongs to a dead process and must be zeroed
            // (accumulated up/down stay untouched). Epoch 0 means no child
            // has started in this panel run yet: keep the baselines, which
            // still match a surviving process if one was left behind.
            let epoch = state.xray.start_epoch();
            if epoch > 0 && stats_epoch != Some(epoch) {
                match db::client_traffic::reset_all_baselines(&state.db).await {
                    Ok(()) => {
                        stats_epoch = Some(epoch);
                        // The first sample after a restart only establishes the
                        // new process's counters; there is no interval to rate.
                        last_stats_at = None;
                        // The new counters belong to another process: the
                        // in-memory display baseline (and the family
                        // first-pass flags) restart with them.
                        stats_last.clear();
                        stats_baselined.clear();
                        tracing::info!(epoch, "xray started; traffic baselines reset");
                    }
                    Err(err) => {
                        // Do not sample counters against baselines that
                        // belong to the previous process: retry instead.
                        tracing::warn!(error = %err, "reset traffic baselines failed");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                }
            }
            let secs = poll_seconds(&state).await;
            // Tick before sleeping so a freshly started panel knows who is
            // online without waiting a full interval first.
            let started = std::time::Instant::now();
            if let Err(err) = tick(
                &state,
                secs,
                &mut last_stats_at,
                &mut stats_last,
                &mut stats_baselined,
                &billing,
            )
            .await
            {
                tracing::warn!(error = %err, "traffic tick failed");
                // Never leave a rate from an earlier, unrelated sample on the
                // panel after a failed accounting round.
                *state.traffic_speeds.lock().await = TrafficSpeeds::default();
            }
            // Keep the configured cadence when a tick ran slow (e.g. an
            // unreachable stats API): sleep only the remainder of the period.
            // A setting change wakes us early so a shorter interval applies
            // without waiting out the old sleep.
            let elapsed = started.elapsed();
            let wait = Duration::from_secs(secs).saturating_sub(elapsed);
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = state.refresh_notify.notified() => {}
            }
        }
    });
}

async fn poll_seconds(state: &AppState) -> u64 {
    db::settings::get(&state.db, "traffic_poll_secs")
        .await
        .ok()
        .flatten()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5)
        .clamp(2, 300)
}

fn online_grace_secs(poll_secs: u64) -> i64 {
    (poll_secs as i64 * 2).clamp(MIN_ONLINE_GRACE_SECS, MAX_ONLINE_GRACE_SECS)
}

async fn clear_online_state(state: &AppState) {
    state.online_clients.lock().await.clear();
    state.online_last_seen.lock().await.clear();
    state.active_inbounds.lock().await.clear();
    state.active_inbound_last_seen.lock().await.clear();
}

async fn tick(
    state: &AppState,
    poll_secs: u64,
    last_stats_at: &mut Option<std::time::Instant>,
    stats_last: &mut HashMap<DeltaKey, i64>,
    stats_baselined: &mut HashSet<&'static str>,
    billing: &BillingSink,
) -> anyhow::Result<()> {
    let now = db::models::now();
    let grace = online_grace_secs(poll_secs);
    // Building the 3x-ui traffic frame is not free: it serialises every active
    // client/inbound and the online sets. Most of the time no browser has the
    // WS open, so skip the whole frame instead of allocating it just to have
    // `send` return Err.
    let ws_enabled = state.ws_events.receiver_count() > 0;
    // Captured before any work: a restart anywhere between the loop's epoch
    // check and the stats query means the sample may belong to a different
    // process than the stored baselines.
    let epoch_before = state.xray.start_epoch();

    // Per-attachment identities are what let us attribute an online email to
    // the inbound it is connected through. They have to exist before the
    // online API result can be decoded.
    let clients = db::clients::list_all(&state.db).await?;
    let associations = db::client_inbounds::all(&state.db).await?;
    let inbounds = db::inbounds::list(&state.db).await?;
    let client_emails: HashSet<String> =
        clients.iter().map(|client| client.email.clone()).collect();
    let (email_by_identity, tag_by_identity) = identity_maps(&clients, &inbounds, &associations);

    // Connection-based onlines are the primary signal (3x-ui GetUsersStats):
    // they catch idle-but-connected clients that move no bytes this poll.
    let api_snapshot = match state.stats.online_users().await {
        Ok(users) => users.as_deref().map(|users| {
            decode_online_users(users, &email_by_identity, &tag_by_identity, &client_emails)
        }),
        Err(err) => {
            tracing::debug!(error = %err, "xray online users unavailable");
            None
        }
    };
    let online_ip_counts = api_snapshot
        .as_ref()
        .map(|snapshot| {
            snapshot
                .ips
                .iter()
                .map(|(email, ips)| (email.clone(), ips.len() as i64))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    let sample_at = std::time::Instant::now();
    // One QueryStats per family (3x-ui): a failing family leaves the others
    // live. An outbound tag with a name the core chokes on used to blank every
    // client's counters for the round.
    let families = match state.stats.query_families().await {
        Ok(families) => families,
        Err(err) => {
            tracing::debug!(error = %err, "xray stats unavailable");
            // A rate from an older sample would be stale; drop it instead of
            // pinning the last value in the UI.
            *state.traffic_speeds.lock().await = TrafficSpeeds::default();
            // No traffic deltas this round, but the online API may still work;
            // either way let both windows age out instead of pinning clients.
            let online_emails =
                refresh_online(state, now, grace, api_snapshot.as_ref(), &HashSet::new()).await;
            refresh_active_inbounds(state, now, grace, &[]).await;
            // Empty deltas clear the live speed in the browser while keeping
            // the connection-based online set accurate.
            broadcast_traffic(
                state,
                poll_secs.saturating_mul(1000),
                &[],
                &[],
                &online_emails,
                &clients,
                now,
            )
            .await;
            if let Err(err) =
                persist_online_heartbeat(state, &clients, &online_emails, &HashSet::new(), now)
                    .await
            {
                tracing::warn!(error = %err, "online heartbeat persist failed");
            }
            return Ok(());
        }
    };
    for family in &families {
        if let Some(error) = &family.error {
            // Partial failure: the other families keep counting this round and
            // the failed one keeps its baselines for the next success.
            tracing::debug!(family = family.family, error, "stats family unavailable");
        }
    }
    // The supervisor may restart Xray between the epoch check at the top of
    // the loop and this query. Those counters belong to another process than
    // the stored baselines; billing them would misreport one window. Discard
    // the sample and let the loop reset the baselines first.
    if state.xray.start_epoch() != epoch_before {
        tracing::debug!("xray restarted during the stats sample; discarding it");
        *state.traffic_speeds.lock().await = TrafficSpeeds::default();
        let online_emails =
            refresh_online(state, now, grace, api_snapshot.as_ref(), &HashSet::new()).await;
        refresh_active_inbounds(state, now, grace, &[]).await;
        // Clear any speed label still pinned to the previous process.
        broadcast_traffic(
            state,
            poll_secs.saturating_mul(1000),
            &[],
            &[],
            &online_emails,
            &clients,
            now,
        )
        .await;
        if let Err(err) =
            persist_online_heartbeat(state, &clients, &online_emails, &HashSet::new(), now).await
        {
            tracing::warn!(error = %err, "online heartbeat persist failed");
        }
        return Ok(());
    }
    // Deltas and rates share one window: baselines only advance when a sample
    // is applied, so a missed RPC makes the next delta span the whole gap.
    let elapsed_ms = last_stats_at
        .map(|last| {
            sample_at
                .saturating_duration_since(last)
                .as_millis()
                .min(i64::MAX as u128) as i64
        })
        .unwrap_or(0);
    // The first sample after a (re)start has no previous timestamp: its deltas
    // are real bytes but there is no window to rate them over, so no speed is
    // advertised for it. A tick that outlasts the configured poll makes the
    // deltas span longer than `poll_secs`; the frame must carry the interval the
    // deltas were actually measured over or the browser inflates the rate.
    let frame_interval = rated_interval_ms(elapsed_ms);
    let rated = frame_interval.is_some();
    let frame_interval_ms = frame_interval.unwrap_or_else(|| poll_secs.saturating_mul(1000));
    // Display path first — speeds, the WS frame and the online/active sets
    // are committed before any database work. 3x-ui's traffic job broadcasts
    // over the hub and hands the accounting to its serialized writer
    // (`submitTrafficWrite`); the previous arrangement ran the WS frame
    // behind every fallible billing write, so on a big deployment the billing
    // tail stretched the gap between two samples for seconds, and the frame
    // divided this poll's bytes by that whole backlog — the rate read far
    // below the instant throughput the NIC monitor shows.
    let mut inbound_speeds: HashMap<String, SpeedSample> = HashMap::new();
    let mut client_speeds: HashMap<String, SpeedSample> = HashMap::new();
    // 3x-ui's WS traffic frame carries this poll's deltas by tag/email; the
    // browser divides them by `clientTrafficIntervalMs`.
    let mut ws_traffics: Vec<InboundTraffic<'_>> = Vec::new();
    let mut ws_client_traffics: Vec<ClientTraffic<'_>> = Vec::new();
    let mut moved_emails: HashSet<String> = HashSet::new();
    let mut active_tags: Vec<String> = Vec::new();
    // Live display deltas (WS frame + traffic_speeds) come from the in-memory
    // baseline engine (3x-ui StatsLastValues), not from the DB: DB baselines
    // only advance on successful billing writes, so a failed write can never
    // stall or blank the speed columns. The in-memory baseline advances on
    // every successful stats query instead.
    let display = display_deltas(
        &families,
        stats_last,
        stats_baselined,
        &email_by_identity,
        &client_emails,
    );

    for client in &clients {
        let display_up = display.client_up.get(&client.email).copied().unwrap_or(0);
        let display_down = display.client_down.get(&client.email).copied().unwrap_or(0);
        if display_up > 0 || display_down > 0 {
            moved_emails.insert(client.email.clone());
        }
        if (display_up > 0 || display_down > 0) && ws_enabled && rated {
            ws_client_traffics.push(ClientTraffic {
                email: &client.email,
                up: display_up,
                down: display_down,
            });
        }
        if let Some(sample) = speed_sample(display_up, display_down, elapsed_ms) {
            client_speeds.insert(client.email.clone(), sample);
        }
    }

    for inbound in &inbounds {
        let display_up = display.inbound_up.get(&inbound.tag).copied().unwrap_or(0);
        let display_down = display.inbound_down.get(&inbound.tag).copied().unwrap_or(0);
        if display_up > 0 || display_down > 0 {
            active_tags.push(inbound.tag.clone());
        }
        if (display_up > 0 || display_down > 0) && ws_enabled && rated {
            ws_traffics.push(InboundTraffic {
                tag: &inbound.tag,
                up: display_up,
                down: display_down,
                is_inbound: true,
                is_outbound: false,
            });
        }
        if let Some(sample) = speed_sample(display_up, display_down, elapsed_ms) {
            inbound_speeds.insert(inbound.tag.clone(), sample);
        }
    }

    *state.traffic_speeds.lock().await = TrafficSpeeds {
        inbounds: inbound_speeds,
        clients: client_speeds,
    };
    // The sample is consumed here: the next rate measures from this instant
    // even if the billing tail (or the history write) runs long.
    *last_stats_at = Some(sample_at);

    let online_emails =
        refresh_online(state, now, grace, api_snapshot.as_ref(), &moved_emails).await;
    refresh_active_inbounds(state, now, grace, &active_tags).await;

    broadcast_traffic(
        state,
        frame_interval_ms,
        &ws_traffics,
        &ws_client_traffics,
        &online_emails,
        &clients,
        now,
    )
    .await;

    // Raw counters for the serialized billing writer. The display path above
    // already ran; whatever the database needs seconds to do next cannot move
    // the sample cadence or the speed columns any more.
    let user_stats = parse_user_stats(
        families
            .iter()
            .filter(|family| family.error.is_none() && family.kind() == "user")
            .flat_map(|family| family.stats.iter()),
    );
    let mut inbound_up: HashMap<String, i64> = HashMap::new();
    let mut inbound_down: HashMap<String, i64> = HashMap::new();
    let mut outbound_up: HashMap<String, i64> = HashMap::new();
    let mut outbound_down: HashMap<String, i64> = HashMap::new();

    for family in families.iter().filter(|family| family.error.is_none()) {
        for stat in &family.stats {
            if let Some((tag, uplink)) = parse_stat(&stat.name, family.kind()) {
                if family.kind() == "outbound" {
                    if uplink {
                        outbound_up.insert(tag.to_string(), stat.value);
                    } else {
                        outbound_down.insert(tag.to_string(), stat.value);
                    }
                } else if uplink {
                    inbound_up.insert(tag.to_string(), stat.value);
                } else {
                    inbound_down.insert(tag.to_string(), stat.value);
                }
            }
        }
    }

    billing
        .push(BillingSample {
            epoch: epoch_before,
            now,
            online_ip_counts,
            user_stats,
            inbound_up,
            inbound_down,
            outbound_up,
            outbound_down,
            moved_emails,
            online_emails,
        })
        .await;

    Ok(())
}

/// One poll's raw counters handed to the serialized billing writer. Values
/// are raw (not deltas): the writer compares them against the stored DB
/// baselines itself, so a newer sample subsumes any still-pending older one —
/// bytes are neither dropped nor billed twice by coalescing.
struct BillingSample {
    epoch: u64,
    now: i64,
    online_ip_counts: HashMap<String, i64>,
    user_stats: HashMap<String, (Option<i64>, Option<i64>)>,
    inbound_up: HashMap<String, i64>,
    inbound_down: HashMap<String, i64>,
    outbound_up: HashMap<String, i64>,
    outbound_down: HashMap<String, i64>,
    moved_emails: HashSet<String>,
    online_emails: HashSet<String>,
}

/// Single-slot coalescing queue between the display tick and the billing
/// writer (3x-ui's serialized traffic writer, Rust form). The slot holds at
/// most one sample, so a writer that falls behind costs bounded memory and
/// the newest counters always win.
#[derive(Clone)]
struct BillingSink {
    pending: Arc<Mutex<Option<BillingSample>>>,
    notify: Arc<Notify>,
}

impl BillingSink {
    fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
            notify: Arc::new(Notify::new()),
        }
    }

    async fn push(&self, sample: BillingSample) {
        *self.pending.lock().await = Some(sample);
        self.notify.notify_one();
    }

    async fn next(&self) -> BillingSample {
        loop {
            // Arm the notification before checking the slot: a push racing
            // between the check and the wait leaves a permit behind, so the
            // writer cannot sleep on a sample it has already been given.
            let notified = self.notify.notified();
            if let Some(sample) = self.pending.lock().await.take() {
                return sample;
            }
            notified.await;
        }
    }
}

async fn run_billing_writer(state: AppState, sink: BillingSink) {
    loop {
        let sample = sink.next().await;
        if let Err(err) = apply_billing(&state, &sample).await {
            // The speed columns and the WS frame were committed before this
            // write; a failure here only delays accounting to the next
            // sample, which re-reads the raw counters and spans the gap.
            tracing::warn!(error = %err, "traffic billing failed");
        }
    }
}

async fn apply_billing(state: &AppState, sample: &BillingSample) -> anyhow::Result<()> {
    // Counters from a process that is already gone would bill that process's
    // whole lifetime against the fresh zeroed baselines: drop the sample, the
    // next one belongs to the new process.
    if state.xray.start_epoch() != sample.epoch {
        tracing::debug!("skipping billing sample from a previous xray process");
        return Ok(());
    }
    let now = sample.now;
    // Rows are re-read here (cheap indexed WAL reads) so the writer charges
    // against current state; the display tick never waits for this.
    let clients = db::clients::list_all(&state.db).await?;
    let associations = db::client_inbounds::all(&state.db).await?;
    let inbounds = db::inbounds::list(&state.db).await?;
    let restart_on_client_disable = db::settings::ui_settings(&state.db)
        .await
        .ok()
        .and_then(|settings| {
            settings
                .get("restartXrayOnClientDisable")
                .and_then(|value| value.as_bool())
        })
        .unwrap_or(true);

    // Split every client's counters per attachment. The old code summed all
    // attachments into one baseline, so a single reset attachment corrupted
    // the deltas of the others. Samples are applied in one transaction that
    // also advances the per-attachment baselines.
    // MASQUE rejects `:` in emails, so its clients are metered under the bare
    // panel email. Knowing which inbounds those are lets a client attached to
    // both MASQUE and a normal inbound bill both counters instead of silently
    // dropping the MASQUE half with the legacy fallback below.
    let masque_inbounds: HashSet<i64> = inbounds
        .iter()
        .filter(|inbound| inbound.protocol == "masque")
        .map(|inbound| inbound.id)
        .collect();
    let mut plans: Vec<ClientSamples> = Vec::with_capacity(clients.len());
    let mut legacy_deltas: HashMap<i64, (i64, i64)> = HashMap::new();
    let mut moved: HashSet<String> = HashSet::new();
    for client in &clients {
        let ids = associations
            .get(&client.id)
            .cloned()
            .unwrap_or_else(|| vec![client.inbound_id]);
        let client_attachment_samples =
            client_samples(&client.email, &ids, &masque_inbounds, &sample.user_stats);
        if !client_attachment_samples.is_empty() {
            plans.push(ClientSamples {
                client_id: client.id,
                samples: client_attachment_samples,
            });
            continue;
        }

        // Configs written outside the panel may meter the bare email. Keep
        // that path on the legacy aggregate columns.
        if let Some(&(up, down)) = sample.user_stats.get(&client.email) {
            let (delta_up, current_up) =
                counter_delta(up, client.last_up).unwrap_or((0, client.last_up));
            let (delta_down, current_down) =
                counter_delta(down, client.last_down).unwrap_or((0, client.last_down));
            if delta_up > 0
                || delta_down > 0
                || current_up != client.last_up
                || current_down != client.last_down
            {
                let last_online = if delta_up > 0 || delta_down > 0 {
                    now
                } else {
                    client.last_online
                };
                db::clients::add_traffic(
                    &state.db,
                    client.id,
                    delta_up,
                    delta_down,
                    current_up,
                    current_down,
                    last_online,
                )
                .await?;
                legacy_deltas.insert(client.id, (delta_up, delta_down));
            }
        }
    }

    let applied = db::client_traffic::apply_tick(&state.db, &plans, now).await?;

    let mut need_restart = false;
    for client in &clients {
        let (delta_up, delta_down) = applied
            .get(&client.id)
            .or_else(|| legacy_deltas.get(&client.id))
            .copied()
            .unwrap_or((0, 0));
        if delta_up > 0 || delta_down > 0 {
            moved.insert(client.email.clone());
        }

        let used = client.up + delta_up + client.down + delta_down;
        let over_quota = client.total > 0 && used >= client.total;
        let expired = expired_at(client.expiry_time, now);
        let over_ip_limit = client.limit_ip > 0
            && sample
                .online_ip_counts
                .get(&client.email)
                .is_some_and(|count| *count > client.limit_ip);
        if client.enable && (over_quota || expired || over_ip_limit) {
            db::clients::set_enable(&state.db, client.id, false).await?;
            tracing::info!(
                email = %client.email,
                over_quota,
                expired,
                over_ip_limit,
                "client disabled"
            );
            if restart_on_client_disable {
                need_restart = true;
            }
        }

        if client.enable && client.reset_days > 0 {
            let interval = client.reset_days.saturating_mul(86_400);
            let due = client.last_reset_time == 0
                || now.saturating_sub(client.last_reset_time) >= interval;
            if due {
                // 3x-ui zeroes only the stored total: Xray keeps counting and
                // the per-attachment baselines stay, so the bytes that follow
                // are billed exactly once and a reset can never re-bill the
                // pre-reset counter.
                db::clients::reset_traffic(&state.db, client.id).await?;
                tracing::info!(email = %client.email, days = client.reset_days, "client traffic reset");
            }
        }
    }

    for inbound in &inbounds {
        let (delta_up, delta_down, next_up, next_down) = direction_deltas(
            sample.inbound_up.get(&inbound.tag).copied(),
            sample.inbound_down.get(&inbound.tag).copied(),
            inbound.last_up,
            inbound.last_down,
        );

        if (sample.inbound_up.contains_key(&inbound.tag)
            || sample.inbound_down.contains_key(&inbound.tag))
            && (delta_up > 0
                || delta_down > 0
                || next_up != inbound.last_up
                || next_down != inbound.last_down)
        {
            db::inbounds::add_traffic(
                &state.db, inbound.id, delta_up, delta_down, next_up, next_down,
            )
            .await?;
        }

        let used = inbound.up + delta_up + inbound.down + delta_down;
        let over_quota = inbound.total > 0 && used >= inbound.total;
        let expired = expired_at(inbound.expiry_time, now);
        if inbound.enable && (over_quota || expired) {
            db::inbounds::set_enable(&state.db, inbound.id, false).await?;
            tracing::info!(tag = %inbound.tag, over_quota, expired, "inbound disabled");
            need_restart = true;
        }

        if inbound.enable && reset_due(inbound, now) {
            reset_inbound_traffic(state, inbound, &clients, &associations).await?;
        }
    }

    // Clients that moved bytes by the display engine's account count as seen
    // too: their billing write may still be pending or may have failed.
    moved.extend(sample.moved_emails.iter().cloned());
    persist_online_heartbeat(state, &clients, &sample.online_emails, &moved, now).await?;

    update_outbound_traffic(state, &sample.outbound_up, &sample.outbound_down).await?;

    if need_restart {
        if let Err(err) = service::apply(state).await {
            tracing::warn!(error = %err, "apply after enforcement failed");
        }
    }

    let (sum_up, sum_down): (i64, i64) =
        sqlx::query_as("SELECT COALESCE(SUM(up), 0), COALESCE(SUM(down), 0) FROM inbounds")
            .fetch_one(&state.db)
            .await?;

    let mut history = state.traffic_history.lock().await;
    history.push_back(TrafficPoint {
        ts: now,
        up: sum_up,
        down: sum_down,
    });
    while history.len() > MAX_HISTORY {
        history.pop_front();
    }

    Ok(())
}

struct OnlineSnapshot {
    emails: HashSet<String>,
    ips: HashMap<String, Vec<crate::xray::stats::OnlineIp>>,
    inbounds: HashMap<String, Vec<String>>,
}

/// Memory-lean baseline key for the live display engine. A stat name is
/// ~50 bytes of which ~30 are fixed family/traffic/direction text; keeping
/// only the family byte, the direction and the variable key (identity or
/// tag) keeps the bounded map small on the low-memory target even at tens of
/// thousands of clients.
#[derive(Hash, PartialEq, Eq, Clone)]
struct DeltaKey {
    kind: u8,
    uplink: bool,
    key: String,
}

/// One poll's display deltas, keyed the way the UI reads them: inbounds by
/// tag, clients by panel email.
#[derive(Default)]
struct DisplayDeltas {
    inbound_up: HashMap<String, i64>,
    inbound_down: HashMap<String, i64>,
    client_up: HashMap<String, i64>,
    client_down: HashMap<String, i64>,
}

// Advance the in-memory delta baseline and produce this poll's display
// deltas. Mirrors 3x-ui GetXrayTraffic (internal/xray/api.go GetTraffic):
// - the first successful sample of a family only records baselines, since
//   the counters may already hold traffic the panel cannot attribute;
// - a counter the panel has not seen, or one that moved backwards because
//   the core restarted, counts from zero so no traffic is dropped;
// - a failed family keeps its baselines and yields nothing this round —
//   the next success measures the gap, not the whole counters again;
// - baselines for counters that no longer exist are pruned once the map has
//   drifted past twice the live set (3x-ui), so the steady-state hot path
//   stays allocation-free.
fn display_deltas(
    families: &[FamilyStats],
    stats_last: &mut HashMap<DeltaKey, i64>,
    baselined: &mut HashSet<&'static str>,
    email_by_identity: &HashMap<String, String>,
    client_emails: &HashSet<String>,
) -> DisplayDeltas {
    let mut out = DisplayDeltas::default();
    for family in families {
        if family.error.is_some() {
            // The family failed: its baselines are not gone, so the next
            // success must not count the whole counters again.
            continue;
        }
        let kind = family.kind();
        let kind_byte = kind.as_bytes()[0];
        // HashSet::insert reports whether the family was new, i.e. baseline
        // pass: record baselines only, no deltas this round.
        let baseline_pass = baselined.insert(family.family);
        for stat in &family.stats {
            let Some((key, uplink)) = parse_stat(&stat.name, kind) else {
                // Non-traffic counters need no display baseline.
                continue;
            };
            let previous = stats_last.insert(
                DeltaKey {
                    kind: kind_byte,
                    uplink,
                    key: key.to_string(),
                },
                stat.value,
            );
            let last = match previous {
                Some(previous) if stat.value >= previous => previous,
                // Unseen or backward counter counts from zero (3x-ui).
                _ => 0,
            };
            if baseline_pass {
                continue;
            }
            let value = stat.value - last;
            match kind {
                "inbound" => {
                    if value > 0 {
                        let slot = if uplink {
                            &mut out.inbound_up
                        } else {
                            &mut out.inbound_down
                        };
                        *slot.entry(key.to_string()).or_insert(0) += value;
                    }
                }
                // Outbound tags have no live display: the outbounds page reads
                // stored totals, not per-poll speeds.
                "outbound" => {}
                _ => {
                    // Map the per-attachment identity back to the panel email;
                    // configs written outside the panel meter the bare email,
                    // accepted whenever it is a live client (online view does
                    // the same).
                    let Some(email) = email_by_identity
                        .get(key)
                        .cloned()
                        .or_else(|| client_emails.contains(key).then(|| key.to_string()))
                    else {
                        continue;
                    };
                    if value > 0 {
                        let slot = if uplink {
                            &mut out.client_up
                        } else {
                            &mut out.client_down
                        };
                        *slot.entry(email).or_insert(0) += value;
                    }
                }
            }
        }
    }

    let failures = families.iter().any(|family| family.error.is_some());
    let live: usize = families
        .iter()
        .filter(|family| family.error.is_none())
        .map(|family| family.stats.len())
        .sum();
    if !failures && live > 0 && stats_last.len() > 2 * live {
        let live_keys: HashSet<DeltaKey> = families
            .iter()
            .filter(|family| family.error.is_none())
            .flat_map(|family| {
                family.stats.iter().filter_map(|stat| {
                    parse_stat(&stat.name, family.kind()).map(|(key, uplink)| DeltaKey {
                        kind: family.kind().as_bytes()[0],
                        uplink,
                        key: key.to_string(),
                    })
                })
            })
            .collect();
        stats_last.retain(|key, _| live_keys.contains(key));
    }
    out
}

// The generated config meters each (inbound, client) attachment under its own
// identity (`<inbound id>:<email>`). These maps turn those identities back
// into the panel email and the inbound tag it belongs to.
fn identity_maps(
    clients: &[Client],
    inbounds: &[Inbound],
    associations: &HashMap<i64, Vec<i64>>,
) -> (HashMap<String, String>, HashMap<String, String>) {
    let tags: HashMap<i64, &str> = inbounds
        .iter()
        .map(|inbound| (inbound.id, inbound.tag.as_str()))
        .collect();
    let mut email_by_identity = HashMap::new();
    let mut tag_by_identity = HashMap::new();
    for client in clients {
        let ids = associations
            .get(&client.id)
            .cloned()
            .unwrap_or_else(|| vec![client.inbound_id]);
        for id in ids {
            let Some(tag) = tags.get(&id) else {
                continue;
            };
            let identity = stat_email(id, &client.email);
            email_by_identity.insert(identity.clone(), client.email.clone());
            tag_by_identity.insert(identity, (*tag).to_string());
        }
    }
    (email_by_identity, tag_by_identity)
}

fn decode_online_users(
    users: &[crate::xray::OnlineUser],
    email_by_identity: &HashMap<String, String>,
    tag_by_identity: &HashMap<String, String>,
    client_emails: &HashSet<String>,
) -> OnlineSnapshot {
    let mut emails = HashSet::new();
    let mut ips: HashMap<String, HashMap<String, crate::xray::stats::OnlineIp>> = HashMap::new();
    let mut inbounds: HashMap<String, HashSet<String>> = HashMap::new();
    for user in users {
        // Only identities that map back to a live client may enter the online
        // set. A fallback to the raw email used to admit deleted users and
        // stale stat identities, inflating the dashboard and the IP counts;
        // the raw form is still accepted when it is a real client email
        // (configs written outside the panel).
        let email = match email_by_identity.get(&user.email) {
            Some(email) => email.clone(),
            None if client_emails.contains(&user.email) => user.email.clone(),
            None => {
                tracing::debug!(identity = %user.email, "ignoring unknown online identity");
                continue;
            }
        };
        emails.insert(email.clone());
        for ip in &user.ips {
            ips.entry(email.clone())
                .or_default()
                .insert(ip.ip.clone(), ip.clone());
        }
        if let Some(tag) = tag_by_identity.get(&user.email) {
            inbounds.entry(email).or_default().insert(tag.clone());
        }
    }
    OnlineSnapshot {
        emails,
        ips: ips
            .into_iter()
            .map(|(email, by_ip)| (email, by_ip.into_values().collect()))
            .collect(),
        inbounds: inbounds
            .into_iter()
            .map(|(email, tags)| (email, tags.into_iter().collect()))
            .collect(),
    }
}

// Per-attachment samples for one client. Xray meters each normal attachment
// under `<inbound id>:<email>`; MASQUE rejects that identity and uses the raw
// email instead, so it is looked up once and attributed to one MASQUE
// attachment. Without that split a client on both inbound kinds lost the
// MASQUE counter whenever the other attachment reported first.
fn client_samples(
    email: &str,
    inbound_ids: &[i64],
    masque_inbounds: &HashSet<i64>,
    user_stats: &HashMap<String, (Option<i64>, Option<i64>)>,
) -> Vec<AttachmentSample> {
    let mut samples = Vec::with_capacity(inbound_ids.len());
    let mut masque_inbound: Option<i64> = None;
    for inbound_id in inbound_ids {
        if masque_inbounds.contains(inbound_id) {
            masque_inbound.get_or_insert(*inbound_id);
            continue;
        }
        let identity = stat_email(*inbound_id, email);
        if let Some(&(up, down)) = user_stats.get(&identity) {
            samples.push(AttachmentSample {
                inbound_id: *inbound_id,
                up,
                down,
            });
        }
    }
    if let Some(inbound_id) = masque_inbound {
        if let Some(&(up, down)) = user_stats.get(email) {
            samples.push(AttachmentSample {
                inbound_id,
                up,
                down,
            });
        }
    }
    samples
}

// Raw counters per stat identity: identity -> (uplink, downlink). A direction
// is `None` when xray did not return it; that must not be coerced to zero.
fn parse_user_stats<'a>(
    stats: impl Iterator<Item = &'a crate::xray::stats::proto::Stat>,
) -> HashMap<String, (Option<i64>, Option<i64>)> {
    let mut out: HashMap<String, (Option<i64>, Option<i64>)> = HashMap::new();
    for stat in stats {
        let Some((key, uplink)) = parse_stat(&stat.name, "user") else {
            continue;
        };
        let entry = out.entry(key.to_string()).or_insert((None, None));
        if uplink {
            entry.0 = Some(stat.value);
        } else {
            entry.1 = Some(stat.value);
        }
    }
    out
}

// Idle-but-connected clients never write traffic, so persist their heartbeat
// separately; active clients already wrote `now` in apply_tick. One batched
// UPDATE per tick, like 3x-ui's BumpClientsLastOnline.
async fn persist_online_heartbeat(
    state: &AppState,
    clients: &[Client],
    online_emails: &HashSet<String>,
    moved_emails: &HashSet<String>,
    now: i64,
) -> anyhow::Result<()> {
    let touch_ids: Vec<i64> = clients
        .iter()
        .filter(|client| {
            online_emails.contains(&client.email) && !moved_emails.contains(&client.email)
        })
        .map(|client| client.id)
        .collect();
    db::clients::touch_last_online(&state.db, &touch_ids, now).await?;
    Ok(())
}

// Union of the connection-based online users and the emails whose counters
// moved this poll, then age everything past the grace window, mirroring
// 3x-ui GetOnlineUsers + traffic deltas.
async fn refresh_online(
    state: &AppState,
    now: i64,
    grace: i64,
    api: Option<&OnlineSnapshot>,
    moved_emails: &HashSet<String>,
) -> HashSet<String> {
    let online = {
        let mut seen = state.online_last_seen.lock().await;
        fold_online_signals(
            &mut seen,
            now,
            grace,
            api.map(|snapshot| &snapshot.emails),
            moved_emails,
        )
    };

    let mut snapshot = state.online_clients.lock().await;
    snapshot.clear();
    for email in &online {
        let (ips, inbounds) = api
            .map(|api| {
                (
                    api.ips.get(email).cloned().unwrap_or_default(),
                    api.inbounds.get(email).cloned().unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        snapshot.insert(
            email.clone(),
            crate::xray::OnlineUser {
                email: email.clone(),
                ips,
                inbounds,
            },
        );
    }
    online
}

fn fold_online_signals(
    seen: &mut HashMap<String, i64>,
    now: i64,
    grace: i64,
    api_emails: Option<&HashSet<String>>,
    moved_emails: &HashSet<String>,
) -> HashSet<String> {
    for email in moved_emails {
        seen.insert(email.clone(), now);
    }
    if let Some(api_emails) = api_emails {
        for email in api_emails {
            seen.insert(email.clone(), now);
        }
    }
    // Half-open window, like 3x-ui's `now-ts < grace`: a client idle for
    // exactly the grace period is offline.
    seen.retain(|_, seen_at| now.saturating_sub(*seen_at) < grace);
    seen.keys().cloned().collect()
}

async fn refresh_active_inbounds(state: &AppState, now: i64, grace: i64, moved_tags: &[String]) {
    let mut active = state.active_inbound_last_seen.lock().await;
    for tag in moved_tags {
        active.insert(tag.clone(), now);
    }
    active.retain(|_, seen_at| now.saturating_sub(*seen_at) < grace);
    *state.active_inbounds.lock().await = active.keys().cloned().collect();
}

// One per-poll delta, serialised directly. The frame can carry thousands of
// entries on a busy panel, so building a serde_json::Value tree first doubled
// the allocation count and the peak heap for every tick.
#[derive(Serialize)]
struct InboundTraffic<'a> {
    #[serde(rename = "Tag")]
    tag: &'a str,
    #[serde(rename = "Up")]
    up: i64,
    #[serde(rename = "Down")]
    down: i64,
    #[serde(rename = "IsInbound")]
    is_inbound: bool,
    #[serde(rename = "IsOutbound")]
    is_outbound: bool,
}

#[derive(Serialize)]
struct ClientTraffic<'a> {
    email: &'a str,
    up: i64,
    down: i64,
}

#[derive(Serialize)]
struct TrafficFrame<'a, 'b> {
    #[serde(rename = "type")]
    kind: &'static str,
    payload: TrafficPayload<'a, 'b>,
    time: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrafficPayload<'a, 'b> {
    traffics: &'a [InboundTraffic<'a>],
    client_traffics: &'a [ClientTraffic<'a>],
    online_clients: &'a [&'a String],
    online_by_guid: HashMap<&'static str, &'a [&'a String]>,
    active_inbounds: HashMap<&'static str, &'b [String]>,
    last_online_map: &'a HashMap<&'a str, i64>,
    client_traffic_interval_ms: u64,
}

// 3x-ui pushes one `traffic` frame per poll. The browser's existing handlers
// divide these deltas by `client_traffic_interval_ms` and clear tags absent
// from the frame, so the live speed no longer depends on when its REST poll
// lands.
async fn broadcast_traffic<'a>(
    state: &AppState,
    interval_ms: u64,
    traffics: &'a [InboundTraffic<'a>],
    client_traffics: &'a [ClientTraffic<'a>],
    online_emails: &HashSet<String>,
    clients: &'a [Client],
    now: i64,
) {
    if state.ws_events.receiver_count() == 0 {
        return;
    }
    let online: Vec<&String> = online_emails.iter().collect();
    let active = state.active_inbounds.lock().await.clone();
    // Only clients seen online this tick need a fresh timestamp; the browser
    // merges the map, and the REST endpoint returns milliseconds, so keep the
    // same unit here or the inbound info modal renders a 1970 timestamp.
    let last_online: HashMap<&str, i64> = clients
        .iter()
        .filter(|client| online_emails.contains(&client.email))
        .map(|client| (client.email.as_str(), now.saturating_mul(1000)))
        .collect();
    let frame = TrafficFrame {
        kind: "traffic",
        payload: TrafficPayload {
            traffics,
            client_traffics,
            online_clients: &online,
            online_by_guid: HashMap::from([("", online.as_slice())]),
            active_inbounds: HashMap::from([("", active.as_slice())]),
            last_online_map: &last_online,
            client_traffic_interval_ms: interval_ms,
        },
        time: chrono::Utc::now().timestamp_millis(),
    };
    let Ok(message) = serde_json::to_string(&frame) else {
        return;
    };
    // Err only means nobody is connected; the next tick tries again.
    let _ = state.ws_events.send(message);
}

async fn reset_inbound_traffic(
    state: &AppState,
    inbound: &Inbound,
    clients: &[Client],
    associations: &HashMap<i64, Vec<i64>>,
) -> anyhow::Result<()> {
    let members: Vec<&Client> = clients
        .iter()
        .filter(|client| {
            associations
                .get(&client.id)
                .map(|ids| ids.contains(&inbound.id))
                .unwrap_or(client.inbound_id == inbound.id)
        })
        .collect();

    // A client's stored total is an aggregate across every attachment, so an
    // inbound-level reset must clear it whole; the baselines stay so the next
    // sample bills only what happened after this point (3x-ui's model).
    for client in &members {
        db::clients::reset_traffic(&state.db, client.id).await?;
    }
    db::inbounds::reset_traffic(&state.db, inbound.id).await?;
    tracing::info!(tag = %inbound.tag, "scheduled traffic reset");
    Ok(())
}

async fn update_outbound_traffic(
    state: &AppState,
    up: &HashMap<String, i64>,
    down: &HashMap<String, i64>,
) -> anyhow::Result<()> {
    let rows = db::outbounds::list_traffic(&state.db).await?;
    let mut known: HashMap<String, (i64, i64)> = rows
        .into_iter()
        .map(|row| (row.tag, (row.last_up, row.last_down)))
        .collect();
    let tags: HashSet<String> = up.keys().chain(down.keys()).cloned().collect();

    for tag in tags {
        let (last_up, last_down) = known.remove(&tag).unwrap_or((0, 0));
        let (delta_up, delta_down, next_up, next_down) = direction_deltas(
            up.get(&tag).copied(),
            down.get(&tag).copied(),
            last_up,
            last_down,
        );
        if delta_up > 0 || delta_down > 0 || next_up != last_up || next_down != last_down {
            db::outbounds::upsert_traffic(
                &state.db, &tag, delta_up, delta_down, next_up, next_down,
            )
            .await?;
        }
    }
    Ok(())
}

// The UI/API write expiry as Unix milliseconds; older rows (native API) may
// hold seconds. Normalize both before comparing against `now_secs`.
fn expired_at(expiry: i64, now_secs: i64) -> bool {
    if expiry <= 0 {
        return false;
    }
    let expiry_ms = if expiry < 100_000_000_000 {
        expiry.saturating_mul(1000)
    } else {
        expiry
    };
    expiry_ms <= now_secs.saturating_mul(1000)
}

// The divisor the browser must use for this poll's deltas. `None` means there
// was no previous sample (panel/Xray just started), so the bytes may be billed
// but must not be advertised as a rate: the window they span is unknown.
fn rated_interval_ms(elapsed_ms: i64) -> Option<u64> {
    (elapsed_ms > 0).then_some(elapsed_ms as u64)
}

// Bytes per second for one poll window. `elapsed_ms` is the real time between
// the two samples the deltas were measured over, so a delayed tick does not
// inflate the rate the way a fixed divisor would.
fn speed_sample(up: i64, down: i64, elapsed_ms: i64) -> Option<SpeedSample> {
    if elapsed_ms <= 0 {
        return None;
    }
    let rate = |delta: i64| -> i64 {
        if delta <= 0 {
            0
        } else {
            delta.saturating_mul(1000) / elapsed_ms
        }
    };
    let sample = SpeedSample {
        up: rate(up),
        down: rate(down),
    };
    (sample.up > 0 || sample.down > 0).then_some(sample)
}

// One inbound/outbound tag reports uplink and downlink independently. A
// missing or invalid direction keeps its old baseline while the other one is
// still billed, so a transiently absent stat cannot later be billed twice.
fn direction_deltas(
    up: Option<i64>,
    down: Option<i64>,
    last_up: i64,
    last_down: i64,
) -> (i64, i64, i64, i64) {
    let (delta_up, next_up) = counter_delta(up, last_up).unwrap_or((0, last_up));
    let (delta_down, next_down) = counter_delta(down, last_down).unwrap_or((0, last_down));
    (delta_up, delta_down, next_up, next_down)
}

fn parse_stat<'a>(name: &'a str, kind: &str) -> Option<(&'a str, bool)> {
    let mut parts = name.split(">>>");
    if parts.next()? != kind {
        return None;
    }
    let key = parts.next()?;
    if parts.next()? != "traffic" {
        return None;
    }
    let direction = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    match direction {
        "uplink" => Some((key, true)),
        "downlink" => Some((key, false)),
        _ => None,
    }
}

fn reset_due(inbound: &Inbound, now: i64) -> bool {
    if inbound.traffic_reset == "never" || inbound.last_traffic_reset_time <= 0 {
        return false;
    }
    let Some(last) = Local
        .timestamp_opt(inbound.last_traffic_reset_time, 0)
        .single()
    else {
        return false;
    };
    let Some(current) = Local.timestamp_opt(now, 0).single() else {
        return false;
    };

    match inbound.traffic_reset.as_str() {
        "hourly" => current.hour() != last.hour() || current.date_naive() != last.date_naive(),
        "daily" => current.date_naive() != last.date_naive(),
        "weekly" => current.iso_week() != last.iso_week(),
        "monthly" => {
            let month_changed = current.year() != last.year() || current.month() != last.month();
            let days = days_in_month(current.year(), current.month());
            let target = inbound.traffic_reset_day.clamp(1, days as i64) as u32;
            month_changed && current.day() >= target
        }
        _ => false,
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_divides_the_delta_by_the_real_sample_interval() {
        assert_eq!(
            speed_sample(1_000, 2_000, 5_000),
            Some(SpeedSample { up: 200, down: 400 })
        );
        // A delayed tick spans a longer window, so the same bytes are a lower
        // rate rather than the old fixed-divisor inflation.
        assert_eq!(
            speed_sample(1_000, 0, 10_000),
            Some(SpeedSample { up: 100, down: 0 })
        );
        // Idle or unmeasured windows produce no entry, which clears the UI.
        assert_eq!(speed_sample(0, 0, 5_000), None);
        assert_eq!(speed_sample(100, 100, 0), None);
    }

    #[test]
    fn ws_frame_divisor_is_the_real_window_not_the_configured_poll() {
        // A 2s poll whose tick took 3s must tell the browser 3s: dividing the
        // same deltas by 2s made every live speed read 50% high.
        assert_eq!(rated_interval_ms(3_027), Some(3_027));
        // No previous sample (panel/Xray just started): bill, but advertise no
        // rate, otherwise the startup counters look like a huge burst.
        assert_eq!(rated_interval_ms(0), None);
        assert_eq!(rated_interval_ms(-1), None);
    }

    #[test]
    fn grace_scales_with_poll_and_never_undercuts_xui() {
        // 3x-ui's 5s poll uses a 20s window.
        assert_eq!(online_grace_secs(5), 20);
        assert_eq!(online_grace_secs(10), 20);
        assert_eq!(online_grace_secs(30), 60);
        // A slow poll stays capped: 2×300s would keep ghosts for 10 minutes.
        assert_eq!(online_grace_secs(300), 60);
    }

    #[test]
    fn online_window_unions_live_connections_and_traffic_movers() {
        let moved = HashSet::from(["burst".to_string()]);
        let mut seen = HashMap::new();
        let api = HashSet::from(["idle".to_string()]);
        let online = fold_online_signals(&mut seen, 1_000, 20, Some(&api), &moved);
        assert_eq!(
            online,
            HashSet::from(["burst".to_string(), "idle".to_string()])
        );
    }

    #[test]
    fn online_window_drops_disconnects_after_grace_like_xui() {
        let no_moved = HashSet::new();
        let mut seen = HashMap::from([("old".to_string(), 980)]);
        // Idle exactly `grace` seconds: half-open window means offline.
        let api: HashSet<String> = HashSet::new();
        let online = fold_online_signals(&mut seen, 1_000, 20, Some(&api), &no_moved);
        assert!(online.is_empty());
        assert!(!seen.contains_key("old"));

        // One second inside the window: still online.
        let mut seen = HashMap::from([("edge".to_string(), 981)]);
        let online = fold_online_signals(&mut seen, 1_000, 20, Some(&api), &no_moved);
        assert_eq!(online, HashSet::from(["edge".to_string()]));
    }

    #[test]
    fn user_stats_keep_each_attachment_separate() {
        use crate::xray::stats::proto::Stat;

        let stats = [
            Stat {
                name: "user>>>1:alice>>>traffic>>>uplink".into(),
                value: 10,
            },
            Stat {
                name: "user>>>1:alice>>>traffic>>>downlink".into(),
                value: 20,
            },
            Stat {
                name: "user>>>2:alice>>>traffic>>>uplink".into(),
                value: 5,
            },
            Stat {
                name: "user>>>bob>>>traffic>>>downlink".into(),
                value: 3,
            },
        ];
        let parsed = parse_user_stats(stats.iter());
        assert_eq!(parsed.get("1:alice"), Some(&(Some(10), Some(20))));
        // Downlink was not reported for this attachment: None, not zero.
        assert_eq!(parsed.get("2:alice"), Some(&(Some(5), None)));
        assert_eq!(parsed.get("bob"), Some(&(None, Some(3))));
    }

    #[test]
    fn masque_bare_email_counter_survives_a_normal_attachment() {
        let user_stats = HashMap::from([
            ("1:alice".to_string(), (Some(10_i64), Some(20_i64))),
            ("alice".to_string(), (Some(7_i64), Some(3_i64))),
        ]);
        let masque = HashSet::from([9_i64]);
        let samples = client_samples("alice", &[1, 9], &masque, &user_stats);

        assert_eq!(samples.len(), 2);
        assert_eq!(
            (samples[0].inbound_id, samples[0].up, samples[0].down),
            (1, Some(10), Some(20))
        );
        assert_eq!(
            (samples[1].inbound_id, samples[1].up, samples[1].down),
            (9, Some(7), Some(3))
        );

        // Two MASQUE attachments share one bare-email counter, so it must be
        // billed once, not once per attachment.
        let samples = client_samples("alice", &[9, 10], &HashSet::from([9, 10]), &user_stats);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].inbound_id, 9);
    }

    #[test]
    fn online_identities_map_to_email_and_inbound_tags() {
        use crate::xray::stats::OnlineIp;
        use crate::xray::OnlineUser;

        let email_by_identity = HashMap::from([
            ("1:alice".to_string(), "alice".to_string()),
            ("2:alice".to_string(), "alice".to_string()),
        ]);
        let tag_by_identity = HashMap::from([
            ("1:alice".to_string(), "in-a".to_string()),
            ("2:alice".to_string(), "in-b".to_string()),
        ]);
        let users = vec![
            OnlineUser {
                email: "1:alice".into(),
                ips: vec![OnlineIp {
                    ip: "1.2.3.4".into(),
                    last_seen: 1,
                }],
                inbounds: Vec::new(),
            },
            OnlineUser {
                email: "2:alice".into(),
                ips: vec![
                    OnlineIp {
                        ip: "1.2.3.4".into(),
                        last_seen: 2,
                    },
                    OnlineIp {
                        ip: "5.6.7.8".into(),
                        last_seen: 2,
                    },
                ],
                inbounds: Vec::new(),
            },
        ];
        let client_emails = HashSet::from(["alice".to_string()]);
        let snapshot =
            decode_online_users(&users, &email_by_identity, &tag_by_identity, &client_emails);
        assert_eq!(snapshot.emails, HashSet::from(["alice".to_string()]));
        assert_eq!(snapshot.ips["alice"].len(), 2, "IPs are deduped");
        let mut tags = snapshot.inbounds["alice"].clone();
        tags.sort();
        assert_eq!(tags, vec!["in-a".to_string(), "in-b".to_string()]);
    }

    #[test]
    fn unknown_online_identities_are_ignored() {
        use crate::xray::stats::OnlineIp;
        use crate::xray::OnlineUser;

        let email_by_identity = HashMap::from([("1:alice".to_string(), "alice".to_string())]);
        let tag_by_identity = HashMap::new();
        let users = vec![
            OnlineUser {
                email: "1:alice".into(),
                ips: Vec::new(),
                inbounds: Vec::new(),
            },
            // Deleted client / stale identity from the running core.
            OnlineUser {
                email: "999:ghost".into(),
                ips: vec![OnlineIp {
                    ip: "6.6.6.6".into(),
                    last_seen: 1,
                }],
                inbounds: Vec::new(),
            },
            // A plain email that still exists in the panel is accepted.
            OnlineUser {
                email: "bob".into(),
                ips: Vec::new(),
                inbounds: Vec::new(),
            },
        ];
        let client_emails = HashSet::from(["alice".to_string(), "bob".to_string()]);
        let snapshot =
            decode_online_users(&users, &email_by_identity, &tag_by_identity, &client_emails);
        assert_eq!(
            snapshot.emails,
            HashSet::from(["alice".to_string(), "bob".to_string()])
        );
        assert!(!snapshot.ips.contains_key("ghost"));
    }

    #[test]
    fn expiry_handles_milliseconds_and_legacy_seconds() {
        let now = 1_700_000_000;
        assert!(expired_at(1_699_999_999, now));
        assert!(expired_at(1_699_999_999_000, now));
        assert!(!expired_at(1_700_000_001, now));
        assert!(!expired_at(1_700_000_001_000, now));
        assert!(!expired_at(0, now));
        assert!(!expired_at(-86_400_000, now));
    }

    #[test]
    fn missing_or_invalid_directions_keep_their_baselines() {
        // Downlink moved, uplink was not reported: only down is billed.
        assert_eq!(
            direction_deltas(None, Some(260), 100, 200),
            (0, 60, 100, 260)
        );
        // The mirror case: a missing downlink must not reset last_down.
        assert_eq!(
            direction_deltas(Some(150), None, 100, 200),
            (50, 0, 150, 200)
        );
        assert_eq!(direction_deltas(None, None, 100, 200), (0, 0, 100, 200));
        // Negative samples are treated like missing ones.
        assert_eq!(
            direction_deltas(Some(-1), Some(0), 100, 200),
            (0, 0, 100, 0)
        );
    }

    #[test]
    fn parses_user_and_inbound_stats_only() {
        assert_eq!(
            parse_stat("user>>>a@b>>>traffic>>>uplink", "user"),
            Some(("a@b", true))
        );
        assert_eq!(
            parse_stat("inbound>>>in-443>>>traffic>>>downlink", "inbound"),
            Some(("in-443", false))
        );
        assert_eq!(parse_stat("user>>>a@b>>>traffic>>>uplink", "inbound"), None);
        assert_eq!(parse_stat("garbage", "user"), None);
    }

    use crate::xray::stats::{FAMILY_INBOUND, FAMILY_USER};

    fn family(family: &'static str, stats: Vec<(&str, i64)>) -> FamilyStats {
        FamilyStats {
            family,
            stats: stats
                .into_iter()
                .map(|(name, value)| crate::xray::stats::proto::Stat {
                    name: name.into(),
                    value,
                })
                .collect(),
            error: None,
        }
    }

    fn empty_maps() -> (HashMap<String, String>, HashSet<String>) {
        (HashMap::new(), HashSet::new())
    }

    #[test]
    fn first_pass_records_baselines_only_like_xui() {
        let mut stats_last = HashMap::new();
        let mut baselined = HashSet::new();
        let (email_by_identity, client_emails) = empty_maps();
        let families = vec![family(
            FAMILY_INBOUND,
            vec![("inbound>>>in-a>>>traffic>>>uplink", 100)],
        )];
        let display = display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );
        // Counters may already hold unattributable traffic: no deltas.
        assert!(display.inbound_up.is_empty());
        assert!(baselined.contains(&FAMILY_INBOUND));
        assert_eq!(stats_last.len(), 1);

        // The next sample against those baselines shows the delta.
        let families = vec![family(
            FAMILY_INBOUND,
            vec![("inbound>>>in-a>>>traffic>>>uplink", 160)],
        )];
        let display = display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );
        assert_eq!(display.inbound_up.get("in-a"), Some(&60));
    }

    #[test]
    fn unseen_and_backward_counters_count_from_zero_like_xui() {
        let mut stats_last = HashMap::new();
        let mut baselined = HashSet::new();
        let (email_by_identity, client_emails) = empty_maps();
        let families = vec![family(
            FAMILY_INBOUND,
            vec![("inbound>>>in-a>>>traffic>>>uplink", 100)],
        )];
        display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );

        // A counter created by a user's first use has no baseline: it counts
        // from zero, so nothing is dropped for a whole poll.
        let families = vec![family(
            FAMILY_INBOUND,
            vec![
                ("inbound>>>in-a>>>traffic>>>uplink", 150),
                ("inbound>>>in-b>>>traffic>>>uplink", 40),
            ],
        )];
        let display = display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );
        assert_eq!(display.inbound_up.get("in-a"), Some(&50));
        assert_eq!(display.inbound_up.get("in-b"), Some(&40));

        // A counter that moved backwards (core restart without an epoch
        // event) also counts from zero instead of dropping the traffic.
        let families = vec![family(
            FAMILY_INBOUND,
            vec![("inbound>>>in-a>>>traffic>>>uplink", 10)],
        )];
        let display = display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );
        assert_eq!(display.inbound_up.get("in-a"), Some(&10));
    }

    #[test]
    fn failed_family_keeps_baselines_and_the_next_success_spans_the_gap() {
        let mut stats_last = HashMap::new();
        let mut baselined = HashSet::new();
        let email_by_identity = HashMap::from([("1:alice".to_string(), "alice".to_string())]);
        let client_emails = HashSet::from(["alice".to_string()]);

        let families = vec![family(
            FAMILY_USER,
            vec![("user>>>1:alice>>>traffic>>>uplink", 100)],
        )];
        display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );

        // The user family fails for one poll: nothing displayed for it, no
        // baseline touched for that family.
        let failed = FamilyStats {
            family: FAMILY_USER,
            stats: Vec::new(),
            error: Some("boom".into()),
        };
        let display = display_deltas(
            &[failed],
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );
        assert!(display.client_up.is_empty());

        // Recovering: the delta spans the failed gap (30), rather than
        // recounting the whole counter (130) because the family baseline was
        // dropped.
        let families = vec![family(
            FAMILY_USER,
            vec![("user>>>1:alice>>>traffic>>>uplink", 130)],
        )];
        let display = display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );
        assert_eq!(display.client_up.get("alice"), Some(&30));
    }

    #[test]
    fn display_client_deltas_collapse_identities_into_the_panel_email() {
        let mut stats_last = HashMap::new();
        let mut baselined = HashSet::new();
        let email_by_identity = HashMap::from([("1:alice".to_string(), "alice".to_string())]);
        // A bare-email counter (config written outside the panel) is accepted
        // like the online view does.
        let client_emails = HashSet::from(["alice".to_string(), "bob".to_string()]);

        let families = vec![family(
            FAMILY_USER,
            vec![
                ("user>>>1:alice>>>traffic>>>uplink", 60),
                ("user>>>bob>>>traffic>>>downlink", 20),
                // Not a panel client's counter: nothing to display it under.
                ("user>>>ghost>>>traffic>>>uplink", 500),
            ],
        )];
        display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );

        let families = vec![family(
            FAMILY_USER,
            vec![
                ("user>>>1:alice>>>traffic>>>uplink", 80),
                ("user>>>bob>>>traffic>>>downlink", 35),
                ("user>>>ghost>>>traffic>>>uplink", 600),
            ],
        )];
        let display = display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );
        assert_eq!(display.client_up.get("alice"), Some(&20));
        assert_eq!(display.client_down.get("bob"), Some(&15));
        assert!(!display.client_up.contains_key("ghost"));
    }

    #[test]
    fn drifted_baselines_are_pruned_but_never_after_a_failure() {
        let mut stats_last = HashMap::new();
        let mut baselined = HashSet::new();
        let (email_by_identity, client_emails) = empty_maps();

        let families = vec![family(
            FAMILY_INBOUND,
            vec![("inbound>>>in-a>>>traffic>>>uplink", 100)],
        )];
        display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );
        // Simulate deleted clients whose baselines linger in the map.
        let stale = DeltaKey {
            kind: b'u',
            uplink: true,
            key: "9:ghost".into(),
        };
        stats_last.insert(stale.clone(), 7);
        assert_eq!(stats_last.len(), 2);
        // 2 live entries > 2x1 live stat: no prune yet.
        display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );
        assert_eq!(stats_last.len(), 2);
        // One more stale key tips it over 2x the live set.
        stats_last.insert(
            DeltaKey {
                kind: b'u',
                uplink: false,
                key: "9:ghost".into(),
            },
            8,
        );
        let display = display_deltas(
            &families,
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );
        assert_eq!(stats_last.len(), 1);
        assert!(
            display.inbound_up.is_empty(),
            "equal counters mean no delta"
        );
        assert!(stats_last.contains_key(&DeltaKey {
            kind: b'i',
            uplink: true,
            key: "in-a".into()
        }));

        // With any family failure the map is not rebuilt: missing stats are
        // not deleted ones.
        let failed = FamilyStats {
            family: FAMILY_USER,
            stats: Vec::new(),
            error: Some("boom".into()),
        };
        stats_last.insert(stale, 7);
        display_deltas(
            &[failed],
            &mut stats_last,
            &mut baselined,
            &email_by_identity,
            &client_emails,
        );
        assert_eq!(stats_last.len(), 2);
    }

    fn billing_sample(now: i64) -> BillingSample {
        BillingSample {
            epoch: 0,
            now,
            online_ip_counts: HashMap::new(),
            user_stats: HashMap::new(),
            inbound_up: HashMap::new(),
            inbound_down: HashMap::new(),
            outbound_up: HashMap::new(),
            outbound_down: HashMap::new(),
            moved_emails: HashSet::new(),
            online_emails: HashSet::new(),
        }
    }

    #[tokio::test]
    async fn billing_slot_keeps_only_the_newest_sample() {
        let sink = BillingSink::new();
        sink.push(billing_sample(1)).await;
        sink.push(billing_sample(2)).await;
        // The writer was still busy applying an earlier round: the stale raw
        // sample is replaced, not queued. Raw counters subsume older ones,
        // so coalescing bills nothing twice and drops nothing.
        assert_eq!(sink.next().await.now, 2);
    }

    #[tokio::test]
    async fn billing_slot_wakes_a_waiter() {
        let sink = BillingSink::new();
        let waiter = {
            let sink = sink.clone();
            tokio::spawn(async move { sink.next().await.now })
        };
        // Works whether the waiter parked before or after this push: the
        // notification stores a permit, and `next` arms it before checking.
        sink.push(billing_sample(7)).await;
        assert_eq!(waiter.await.unwrap(), 7);
    }
}
