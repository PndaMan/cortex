-- User-added music stations that stream from a pasted URL (typically a YouTube
-- video or livestream). Only the URL is stored — audio is streamed on demand
-- via the mpv sidecar (see src-tauri/src/mpv.rs), never bundled or downloaded
-- into the database.
CREATE TABLE IF NOT EXISTS custom_stations (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    url        TEXT NOT NULL,
    kind       TEXT NOT NULL DEFAULT 'youtube',  -- 'youtube' | 'live'
    position   INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
