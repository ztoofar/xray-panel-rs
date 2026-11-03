CREATE TABLE users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT    NOT NULL UNIQUE,
    password_hash TEXT    NOT NULL,
    created_at    INTEGER NOT NULL
);

CREATE TABLE sessions (
    token_hash TEXT    PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    csrf       TEXT    NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE INDEX idx_sessions_expires ON sessions (expires_at);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE inbounds (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id                 INTEGER NOT NULL DEFAULT 1,
    up                      INTEGER NOT NULL DEFAULT 0,
    down                    INTEGER NOT NULL DEFAULT 0,
    last_up                 INTEGER NOT NULL DEFAULT 0,
    last_down               INTEGER NOT NULL DEFAULT 0,
    total                   INTEGER NOT NULL DEFAULT 0,
    remark                  TEXT    NOT NULL DEFAULT '',
    enable                  INTEGER NOT NULL DEFAULT 1,
    expiry_time             INTEGER NOT NULL DEFAULT 0,
    traffic_reset           TEXT    NOT NULL DEFAULT 'never',
    traffic_reset_day       INTEGER NOT NULL DEFAULT 1,
    last_traffic_reset_time INTEGER NOT NULL DEFAULT 0,
    listen                  TEXT    NOT NULL DEFAULT '',
    port                    INTEGER NOT NULL,
    protocol                TEXT    NOT NULL,
    settings                TEXT    NOT NULL DEFAULT '{}',
    stream_settings         TEXT    NOT NULL DEFAULT '{}',
    sniffing                TEXT    NOT NULL DEFAULT '{}',
    tag                     TEXT    NOT NULL UNIQUE,
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL
);

CREATE TABLE clients (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    inbound_id  INTEGER NOT NULL REFERENCES inbounds (id) ON DELETE CASCADE,
    email       TEXT    NOT NULL UNIQUE,
    enable      INTEGER NOT NULL DEFAULT 1,
    up          INTEGER NOT NULL DEFAULT 0,
    down        INTEGER NOT NULL DEFAULT 0,
    last_up     INTEGER NOT NULL DEFAULT 0,
    last_down   INTEGER NOT NULL DEFAULT 0,
    total       INTEGER NOT NULL DEFAULT 0,
    expiry_time INTEGER NOT NULL DEFAULT 0,
    settings    TEXT    NOT NULL DEFAULT '{}',
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE INDEX idx_clients_inbound ON clients (inbound_id);

CREATE TABLE api_tokens (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT    NOT NULL,
    token_hash TEXT    NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);
