CREATE TABLE client_inbounds (
    client_id  INTEGER NOT NULL REFERENCES clients (id) ON DELETE CASCADE,
    inbound_id INTEGER NOT NULL REFERENCES inbounds (id) ON DELETE CASCADE,
    PRIMARY KEY (client_id, inbound_id)
);

CREATE INDEX idx_client_inbounds_inbound ON client_inbounds (inbound_id);

INSERT OR IGNORE INTO client_inbounds (client_id, inbound_id)
SELECT id, inbound_id FROM clients;
