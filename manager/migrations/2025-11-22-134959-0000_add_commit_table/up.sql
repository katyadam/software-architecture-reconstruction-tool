CREATE TABLE commits (
    commit_hash CHAR(64) PRIMARY KEY,
    commit_message TEXT NOT NULL,
    codebase_uuid UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_codebases
      FOREIGN KEY (codebase_uuid)
      REFERENCES codebases (codebase_uuid)
      ON DELETE CASCADE
);