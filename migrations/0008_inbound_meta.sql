-- 3x-ui inbound metadata that is not part of the Xray runtime config.
ALTER TABLE inbounds ADD COLUMN sub_sort_index INTEGER NOT NULL DEFAULT 1;
ALTER TABLE inbounds ADD COLUMN exclude_from_sub INTEGER NOT NULL DEFAULT 0;
ALTER TABLE inbounds ADD COLUMN share_addr_strategy TEXT NOT NULL DEFAULT 'node';
ALTER TABLE inbounds ADD COLUMN share_addr TEXT NOT NULL DEFAULT '';
ALTER TABLE inbounds ADD COLUMN disable_flow INTEGER NOT NULL DEFAULT 0;
