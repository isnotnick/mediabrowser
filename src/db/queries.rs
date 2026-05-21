use rusqlite::{params, Connection, Result};
use super::models::{MediaFile, MediaType};

pub fn insert_or_ignore_media(
    conn: &Connection,
    path: &str,
    media_type: &MediaType,
    size_bytes: i64,
    last_modified: i64,
) -> Result<bool> {
    let affected = conn.execute(
        "INSERT OR IGNORE INTO media_files (path, media_type, size_bytes, last_modified)
         VALUES (?1, ?2, ?3, ?4)",
        params![path, media_type.to_string(), size_bytes, last_modified],
    )?;
    Ok(affected > 0)
}

pub fn remove_media_by_path(conn: &Connection, path: &str) -> Result<bool> {
    let affected = conn.execute(
        "DELETE FROM media_files WHERE path = ?1",
        params![path],
    )?;
    Ok(affected > 0)
}

pub fn update_process_status(conn: &Connection, id: i64, status: &str) -> Result<()> {
    conn.execute(
        "UPDATE media_files SET process_status = ?1 WHERE id = ?2",
        params![status, id],
    )?;
    Ok(())
}

pub fn update_thumbnail(conn: &Connection, id: i64, thumbnail_path: &str) -> Result<()> {
    conn.execute(
        "UPDATE media_files SET thumbnail_path = ?1 WHERE id = ?2",
        params![thumbnail_path, id],
    )?;
    Ok(())
}

pub fn update_vtt(conn: &Connection, id: i64, vtt_path: &str) -> Result<()> {
    conn.execute(
        "UPDATE media_files SET vtt_path = ?1 WHERE id = ?2",
        params![vtt_path, id],
    )?;
    Ok(())
}

pub fn search_media(conn: &Connection, search: Option<&str>, sort: Option<&str>) -> Result<Vec<MediaFile>> {
    let mut query = String::from("SELECT id, path, media_type, size_bytes, thumbnail_path, vtt_path, process_status, last_modified FROM media_files");
    let mut params: Vec<String> = Vec::new();

    if let Some(s) = search {
        if !s.trim().is_empty() {
            query.push_str(" WHERE path LIKE ?1");
            params.push(format!("%{}%", s));
        }
    }

    match sort.unwrap_or("last_modified_desc") {
        "name_asc" => query.push_str(" ORDER BY path ASC"),
        "name_desc" => query.push_str(" ORDER BY path DESC"),
        "size_asc" => query.push_str(" ORDER BY size_bytes ASC"),
        "size_desc" => query.push_str(" ORDER BY size_bytes DESC"),
        "last_modified_asc" => query.push_str(" ORDER BY last_modified ASC"),
        _ => query.push_str(" ORDER BY last_modified DESC"), // Default
    }

    let mut stmt = conn.prepare(&query)?;
    
    let media_iter = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
        Ok(MediaFile {
            id: row.get(0)?,
            path: row.get(1)?,
            media_type: MediaType::from_string(&row.get::<_, String>(2)?),
            size_bytes: row.get(3)?,
            thumbnail_path: row.get(4)?,
            vtt_path: row.get(5)?,
            process_status: row.get(6)?,
            last_modified: row.get(7)?,
        })
    })?;

    let mut files = Vec::new();
    for media in media_iter {
        files.push(media?);
    }
    Ok(files)
}

pub fn get_pending_media(conn: &Connection) -> Result<Vec<MediaFile>> {
    let mut stmt = conn.prepare("SELECT id, path, media_type, size_bytes, thumbnail_path, vtt_path, process_status, last_modified FROM media_files WHERE process_status = 'pending'")?;
    let media_iter = stmt.query_map([], |row| {
        Ok(MediaFile {
            id: row.get(0)?,
            path: row.get(1)?,
            media_type: MediaType::from_string(&row.get::<_, String>(2)?),
            size_bytes: row.get(3)?,
            thumbnail_path: row.get(4)?,
            vtt_path: row.get(5)?,
            process_status: row.get(6)?,
            last_modified: row.get(7)?,
        })
    })?;

    let mut files = Vec::new();
    for media in media_iter {
        files.push(media?);
    }
    Ok(files)
}

pub fn get_media_by_id(conn: &Connection, id: i64) -> Result<Option<MediaFile>> {
    let mut stmt = conn.prepare("SELECT id, path, media_type, size_bytes, thumbnail_path, vtt_path, process_status, last_modified FROM media_files WHERE id = ?1")?;
    let mut media_iter = stmt.query_map(params![id], |row| {
        Ok(MediaFile {
            id: row.get(0)?,
            path: row.get(1)?,
            media_type: MediaType::from_string(&row.get::<_, String>(2)?),
            size_bytes: row.get(3)?,
            thumbnail_path: row.get(4)?,
            vtt_path: row.get(5)?,
            process_status: row.get(6)?,
            last_modified: row.get(7)?,
        })
    })?;

    if let Some(media) = media_iter.next() {
        Ok(Some(media?))
    } else {
        Ok(None)
    }
}
