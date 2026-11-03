-- Clients created before subId support need one so the share/QR modal can
-- resolve their links.
UPDATE clients
SET settings = json_set(settings, '$.subId', lower(hex(randomblob(8))))
WHERE json_extract(settings, '$.subId') IS NULL
   OR json_extract(settings, '$.subId') = '';
