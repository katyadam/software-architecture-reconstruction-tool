ALTER TABLE commits DROP CONSTRAINT fk_configurations;
ALTER TABLE commits DROP COLUMN configuration_uuid;

ALTER TABLE codebases
ADD COLUMN configuration_uuid UUID NOT NULL,
ADD CONSTRAINT fk_configurations
    FOREIGN KEY (configuration_uuid)
    REFERENCES configurations (configuration_uuid)
    ON DELETE CASCADE;

ALTER TABLE configurations
ADD COLUMN project_uuid UUID NOT NULL,
ADD CONSTRAINT fk_projects
    FOREIGN KEY (project_uuid)
    REFERENCES projects (project_uuid)
    ON DELETE CASCADE;