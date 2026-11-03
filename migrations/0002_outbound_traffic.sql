CREATE TABLE outbound_traffics (
    tag        TEXT    PRIMARY KEY,
    up         INTEGER NOT NULL DEFAULT 0,
    down       INTEGER NOT NULL DEFAULT 0,
    last_up    INTEGER NOT NULL DEFAULT 0,
    last_down  INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL DEFAULT 0
);
