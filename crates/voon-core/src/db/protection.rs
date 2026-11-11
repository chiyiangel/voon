//! Validate an existing index without letting SQLite touch its original files.

use std::{fs, io::Read, path::Path};

use rusqlite::{Connection, OpenFlags};

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error(
        "此资料库由更高版本的 Voon 创建（结构版本 {found}，当前支持 {supported}）。原库已保留，请新建索引重新扫描。"
    )]
    NewerSchema { found: u32, supported: u32 },
    #[error("无法识别此文件为受支持的 Voon 资料库。原文件已保留，请新建索引重新扫描。")]
    Unrecognized,
    #[error("此位置已有文件或索引相关文件，已保留原文件。请选择新的索引文件名。")]
    AlreadyExists,
    #[error("无法读取本地索引：{0}")]
    Database(#[from] rusqlite::Error),
    #[error("无法访问本地索引文件：{0}")]
    Io(#[from] std::io::Error),
}

pub(super) fn check_existing(path: &Path, migrations: &[&str]) -> Result<(), OpenError> {
    let supported = migrations.len() as u32;
    // Refuse an already newer main file even when its sidecars cannot be
    // understood by this SQLite version. The header is read as ordinary data.
    let mut header = Vec::new();
    fs::File::open(path)?.take(100).read_to_end(&mut header)?;
    if header.len() != 100 || !header.starts_with(b"SQLite format 3\0") {
        return Err(OpenError::Unrecognized);
    }
    let found = u32::from_be_bytes(header[60..64].try_into().unwrap());
    if found > supported {
        return Err(OpenError::NewerSchema { found, supported });
    }
    // A read-only SQLite connection can still create or update WAL shared
    // memory. Inspect disposable copies, including any pending committed WAL,
    // so refusing an index leaves all original bytes and sidecars untouched.
    let temporary = tempfile::tempdir()?;
    let copy = temporary.path().join("index.sqlite3");
    fs::copy(path, &copy)?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut source = path.as_os_str().to_os_string();
        source.push(suffix);
        let destination = temporary.path().join(format!("index.sqlite3{suffix}"));
        match fs::copy(source, destination) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let connection = Connection::open_with_flags(copy, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let found = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if found > supported {
        return Err(OpenError::NewerSchema { found, supported });
    }
    let identity: u32 = connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    if found == 0 || ![0, super::APPLICATION_ID].contains(&identity) {
        return Err(OpenError::Unrecognized);
    }

    // Historical Voon libraries predate application_id. Recognize only the
    // exact supported schema produced by that migration prefix; a familiar
    // table name or version number alone is not proof of library identity.
    let expected = Connection::open_in_memory()?;
    for migration in migrations.iter().take(found as usize) {
        expected.execute_batch(migration)?;
    }
    if schema(&connection)? != schema(&expected)? {
        return Err(OpenError::Unrecognized);
    }
    Ok(())
}

fn schema(connection: &Connection) -> rusqlite::Result<Vec<(String, String, String)>> {
    let mut statement = connection.prepare(
        "SELECT type, name, sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
    )?;
    statement
        .query_map([], |row| {
            let sql: String = row.get(2)?;
            Ok((
                row.get(0)?,
                row.get(1)?,
                sql.split_whitespace().collect::<Vec<_>>().join(" "),
            ))
        })?
        .collect()
}
