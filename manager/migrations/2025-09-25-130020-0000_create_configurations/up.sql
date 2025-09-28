CREATE TABLE configurations (
    configuration_uuid UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_uuid UUID NOT NULL,
    configuration_data JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_projects
      FOREIGN KEY (project_uuid)
      REFERENCES projects (project_uuid)
      ON DELETE CASCADE
);

ALTER TABLE codebases
ADD COLUMN configuration_uuid UUID NOT NULL,
ADD CONSTRAINT fk_configurations
    FOREIGN KEY (configuration_uuid)
    REFERENCES configurations (configuration_uuid)
    ON DELETE CASCADE;