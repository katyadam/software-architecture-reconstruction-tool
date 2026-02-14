CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE commits (
    commit_hash CHAR(64) PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE constants (
    constant_uuid UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    commit_hash CHAR(64) NOT NULL,
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_commits
      FOREIGN KEY (commit_hash)
      REFERENCES commits (commit_hash)
      ON DELETE CASCADE
);