use rusqlite::{Connection, Result};

pub fn init_db(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS media_files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            media_type TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            thumbnail_path TEXT,
            vtt_path TEXT,
            process_status TEXT NOT NULL DEFAULT 'pending',
            last_modified INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;

    // Index to make searching by path faster
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_media_files_path ON media_files (path)",
        [],
    )?;

    Ok(())
}
