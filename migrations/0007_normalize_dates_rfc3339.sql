-- Normalize existing "YYYY-MM-DD HH:MM:SS" timestamps to RFC 3339 UTC ("YYYY-MM-DDTHH:MM:SSZ")

UPDATE packages
SET created_at = REPLACE(created_at, ' ', 'T') || 'Z'
WHERE created_at GLOB '????-??-?? ??:??:??'
  AND created_at NOT LIKE '%T%';

UPDATE package_status
SET checked_at = REPLACE(checked_at, ' ', 'T') || 'Z'
WHERE checked_at GLOB '????-??-?? ??:??:??'
  AND checked_at NOT LIKE '%T%';
