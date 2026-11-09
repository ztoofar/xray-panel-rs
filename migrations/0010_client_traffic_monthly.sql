-- Per-client traffic by calendar month (UTC+8, the same wall clock the panel
-- logs use). Billing adds each tick's deltas here in the same transaction that
-- advances clients.up/down, so a month bucket survives quota resets and Xray
-- restarts. History starts at this migration: the cumulative clients.up/down
-- counters cannot be split into earlier months.
CREATE TABLE client_traffic_monthly (
    client_id  INTEGER NOT NULL REFERENCES clients (id) ON DELETE CASCADE,
    -- 'YYYY-MM'
    month      TEXT NOT NULL,
    up         INTEGER NOT NULL DEFAULT 0,
    down       INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (client_id, month)
);
