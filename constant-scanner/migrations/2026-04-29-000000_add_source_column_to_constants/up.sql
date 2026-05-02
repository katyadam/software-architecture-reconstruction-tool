-- Add nullable source column to constants for auditing scraped entries.
-- Format: "scraper:dotenv:/abs/path/to/.env" or "scraper:docker_compose:/abs/path/to/docker-compose.yml"
ALTER TABLE constants ADD COLUMN source TEXT NULL;
