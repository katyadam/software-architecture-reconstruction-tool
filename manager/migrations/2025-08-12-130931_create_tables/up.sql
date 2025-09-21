CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE projects (
    project_uuid UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    owner TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE codebases (
    codebase_uuid UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    branch TEXT NOT NULL,
    project_uuid UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_projects
      FOREIGN KEY (project_uuid)
      REFERENCES projects (project_uuid)
      ON DELETE CASCADE
);