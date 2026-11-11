use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
};

use super::{Index, Result};
use crate::Collection;

impl Index {
    /// Collections keep explicit parent links. Each member list includes albums
    /// in descendant Collections, so selecting a box-set parent includes all of it.
    pub fn collection_tree(&self) -> Result<Vec<Collection>> {
        let mut statement = self
            .conn
            .prepare("SELECT path, name, parent_path FROM collection ORDER BY name, path")?;
        let mut collections = statement
            .query_map([], |row| {
                Ok(Collection {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    name: row.get(1)?,
                    parent: row.get::<_, Option<String>>(2)?.map(PathBuf::from),
                    albums: vec![],
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        let positions: HashMap<_, _> = collections
            .iter()
            .enumerate()
            .map(|(position, collection)| (collection.path.clone(), position))
            .collect();
        let mut statement = self.conn.prepare("SELECT path, collection_path FROM album WHERE collection_path IS NOT NULL ORDER BY path")?;
        let albums = statement.query_map([], |row| {
            Ok((
                PathBuf::from(row.get::<_, String>(0)?),
                PathBuf::from(row.get::<_, String>(1)?),
            ))
        })?;
        for album in albums {
            let (album, parent) = album?;
            let mut parent = Some(parent);
            let mut visited = BTreeSet::new();
            while let Some(path) = parent {
                if !visited.insert(path.clone()) {
                    break;
                }
                let Some(&position) = positions.get(&path) else {
                    break;
                };
                collections[position].albums.push(album.clone());
                parent = collections[position].parent.clone();
            }
        }
        Ok(collections)
    }
}
