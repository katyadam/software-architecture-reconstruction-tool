CREATE TABLE configurations (
    configuration_uuid UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    codebase_uuid UUID NOT NULL,
    configuration_data JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_codebases
      FOREIGN KEY (codebase_uuid)
      REFERENCES codebases (codebase_uuid)
      ON DELETE CASCADE
);