DROP TABLE configurations;

ALTER TABLE codebases
DROP CONSTRAINT fk_configurations,
DROP COLUMN configuration_uuid;