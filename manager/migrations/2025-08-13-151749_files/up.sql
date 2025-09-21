CREATE TABLE file_records (
    id BIGSERIAL PRIMARY KEY,
    file_path TEXT NOT NULL,
    codebase_uuid UUID NOT NULL,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_codebases
      FOREIGN KEY (codebase_uuid)
      REFERENCES codebases (codebase_uuid)
      ON DELETE CASCADE
);