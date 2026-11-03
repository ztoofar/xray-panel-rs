-- 3x-ui keeps lastOnline in client_traffics; the clients page reads it from
-- record.traffic.lastOnline, so it has to survive panel restarts.
ALTER TABLE clients ADD COLUMN last_online INTEGER NOT NULL DEFAULT 0;
