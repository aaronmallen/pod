CREATE TABLE IF NOT EXISTS http_cache (
  url        TEXT    PRIMARY KEY NOT NULL,
  etag       TEXT,
  body       BLOB    NOT NULL,
  cached_at  INTEGER NOT NULL,
  expires_at INTEGER
);
