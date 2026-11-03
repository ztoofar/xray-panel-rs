-- Per-attachment traffic baselines. The panel meters every (client, inbound)
-- pair under its own Xray stat identity (`<inbound_id>:<email>`), so the
-- incremental counters have to be tracked per attachment instead of one
-- aggregated baseline per client. Aggregated deltas are still summed into
-- clients.up/down, so the API and UI do not change.
CREATE TABLE client_inbound_traffic (
    client_id       INTEGER NOT NULL REFERENCES clients (id) ON DELETE CASCADE,
    inbound_id      INTEGER NOT NULL REFERENCES inbounds (id) ON DELETE CASCADE,
    last_up         INTEGER NOT NULL DEFAULT 0,
    last_down       INTEGER NOT NULL DEFAULT 0,
    -- Set when a scheduled reset only partly succeeded (or its outcome is
    -- unknown). The next full sample adopts the current counters as the new
    -- baseline without billing the gap.
    needs_reconcile INTEGER NOT NULL DEFAULT 0,
    updated_at      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (client_id, inbound_id)
);

CREATE INDEX idx_client_inbound_traffic_inbound
    ON client_inbound_traffic (inbound_id);

-- Existing associations start from zero. Xray is a child process of the
-- panel and its counters restart with it, so the first traffic tick after an
-- upgrade initialises every baseline anyway; this backfill only keeps the
-- table complete. Existing up/down totals are not touched.
INSERT OR IGNORE INTO client_inbound_traffic
    (client_id, inbound_id, last_up, last_down, needs_reconcile, updated_at)
SELECT client_id, inbound_id, 0, 0, 0, unixepoch()
FROM client_inbounds;
