ALTER TABLE commits
ADD COLUMN configuration_uuid UUID NOT NULL,
ADD CONSTRAINT fk_configurations
    FOREIGN KEY (configuration_uuid)
    REFERENCES configurations (configuration_uuid)
    ON DELETE CASCADE;

ALTER TABLE codebases DROP CONSTRAINT fk_configurations;
ALTER TABLE codebases DROP COLUMN configuration_uuid;

ALTER TABLE configurations DROP CONSTRAINT fk_projects;
ALTER TABLE configurations DROP COLUMN project_uuid;