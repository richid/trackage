ALTER TABLE package_status ADD COLUMN description TEXT;
CREATE UNIQUE INDEX idx_package_status_dedup
  ON package_status(package_id, description)
  WHERE description IS NOT NULL;
