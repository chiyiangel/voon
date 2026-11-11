//! Local, path-keyed organization. These operations never access the library.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use rusqlite::{OptionalExtension, params};

use super::{Index, Result, p};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserTagRow {
    pub id: i64,
    pub name: String,
    pub albums: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManualCollectionRow {
    pub id: i64,
    pub name: String,
    /// Album paths in the user's saved order, including currently offline roots.
    pub albums: Vec<PathBuf>,
}

pub(super) fn valid_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "名称不能为空"),
        )));
    }
    Ok(name)
}

impl Index {
    pub fn manual_collections(&self) -> Result<Vec<ManualCollectionRow>> {
        let mut statement = self.conn.prepare(
            "SELECT c.id, c.name, m.album_path FROM manual_collection c
             LEFT JOIN manual_collection_album m ON m.collection_id = c.id
             ORDER BY c.name, m.position, m.album_path",
        )?;
        let mut collections: Vec<ManualCollectionRow> = vec![];
        let mut positions = HashMap::new();
        for row in statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })? {
            let (id, name, album) = row?;
            let position = *positions.entry(id).or_insert_with(|| {
                collections.push(ManualCollectionRow {
                    id,
                    name,
                    albums: vec![],
                });
                collections.len() - 1
            });
            if let Some(album) = album {
                collections[position].albums.push(album.into());
            }
        }
        Ok(collections)
    }

    pub fn set_manual_collection_for_albums(
        &mut self,
        collection: i64,
        albums: &[PathBuf],
        present: bool,
    ) -> Result<()> {
        let transaction = self.conn.transaction()?;
        transaction.query_row(
            "SELECT id FROM manual_collection WHERE id = ?1",
            [collection],
            |_| Ok(()),
        )?;
        let mut next: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM manual_collection_album WHERE collection_id = ?1", [collection], |row| row.get(0),
        )?;
        for album in albums {
            if present {
                next += transaction.execute(
                    "INSERT OR IGNORE INTO manual_collection_album (collection_id, album_path, position) VALUES (?1, ?2, ?3)",
                    params![collection, p(album), next],
                )? as i64;
            } else {
                transaction.execute("DELETE FROM manual_collection_album WHERE collection_id = ?1 AND album_path = ?2", params![collection, p(album)])?;
            }
        }
        transaction.commit()
    }

    /// Reordering must be an exact permutation. Validation and all positions
    /// are committed together, so a malformed order cannot drop membership.
    pub fn set_manual_collection_order(
        &mut self,
        collection: i64,
        albums: &[PathBuf],
    ) -> Result<()> {
        let transaction = self.conn.transaction()?;
        transaction.query_row(
            "SELECT id FROM manual_collection WHERE id = ?1",
            [collection],
            |_| Ok(()),
        )?;
        let mut statement = transaction
            .prepare("SELECT album_path FROM manual_collection_album WHERE collection_id = ?1")?;
        let existing: HashSet<PathBuf> = statement
            .query_map([collection], |row| {
                Ok(PathBuf::from(row.get::<_, String>(0)?))
            })?
            .collect::<Result<_>>()?;
        drop(statement);
        let proposed: HashSet<_> = albums.iter().cloned().collect();
        if proposed.len() != albums.len() || proposed != existing {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "合集顺序必须包含全部成员，且不能重复",
                ),
            )));
        }
        let mut update = transaction.prepare("UPDATE manual_collection_album SET position = ?3 WHERE collection_id = ?1 AND album_path = ?2")?;
        for (position, album) in albums.iter().enumerate() {
            update.execute(params![collection, p(album), position as i64])?;
        }
        drop(update);
        transaction.commit()
    }

    pub fn rename_manual_collection(&self, collection: i64, name: &str) -> Result<()> {
        if self.conn.execute(
            "UPDATE manual_collection SET name = ?2 WHERE id = ?1",
            params![collection, valid_name(name)?],
        )? == 0
        {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
    }

    pub fn delete_manual_collection(&self, collection: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM manual_collection WHERE id = ?1", [collection])?;
        Ok(())
    }

    pub fn create_user_tag(&self, name: &str) -> Result<i64> {
        let name = valid_name(name)?;
        self.conn
            .execute("INSERT OR IGNORE INTO user_tag (name) VALUES (?1)", [name])?;
        self.conn
            .query_row("SELECT id FROM user_tag WHERE name = ?1", [name], |row| {
                row.get(0)
            })
    }

    /// A single query loads the catalogue and memberships for cached browsing.
    pub fn user_tag_catalog(&self) -> Result<Vec<UserTagRow>> {
        let mut statement = self.conn.prepare(
            "SELECT t.id, t.name, m.album_path FROM user_tag t
             LEFT JOIN album_user_tag m ON m.tag_id = t.id ORDER BY t.name, m.album_path",
        )?;
        let mut tags: Vec<UserTagRow> = vec![];
        let mut positions = HashMap::new();
        for row in statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })? {
            let (id, name, album) = row?;
            let position = *positions.entry(id).or_insert_with(|| {
                tags.push(UserTagRow {
                    id,
                    name,
                    albums: vec![],
                });
                tags.len() - 1
            });
            if let Some(album) = album {
                tags[position].albums.push(album.into());
            }
        }
        Ok(tags)
    }

    pub fn set_user_tag_for_albums(
        &mut self,
        albums: &[PathBuf],
        name: &str,
        present: bool,
    ) -> Result<()> {
        let name = valid_name(name)?;
        let transaction = self.conn.transaction()?;
        if present {
            transaction.execute("INSERT OR IGNORE INTO user_tag (name) VALUES (?1)", [name])?;
        }
        let id = transaction
            .query_row("SELECT id FROM user_tag WHERE name = ?1", [name], |row| {
                row.get::<_, i64>(0)
            })
            .optional()?;
        if let Some(id) = id {
            let query = if present {
                "INSERT OR IGNORE INTO album_user_tag (album_path, tag_id) VALUES (?1, ?2)"
            } else {
                "DELETE FROM album_user_tag WHERE album_path = ?1 AND tag_id = ?2"
            };
            let mut statement = transaction.prepare(query)?;
            for album in albums {
                statement.execute(params![p(album), id])?;
            }
        }
        transaction.commit()
    }

    pub fn rename_user_tag(&self, old: &str, name: &str) -> Result<()> {
        let (old, name) = (valid_name(old)?, valid_name(name)?);
        if self.conn.execute(
            "UPDATE user_tag SET name = ?2 WHERE name = ?1",
            params![old, name],
        )? == 0
        {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
    }

    pub fn delete_user_tag(&self, name: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM user_tag WHERE name = ?1", [valid_name(name)?])?;
        Ok(())
    }
}
