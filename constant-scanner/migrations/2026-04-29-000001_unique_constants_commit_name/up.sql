-- Prevent duplicate rows when the same commit is scraped more than once.
ALTER TABLE constants
    ADD CONSTRAINT uq_constants_commit_name UNIQUE (commit_hash, name);
