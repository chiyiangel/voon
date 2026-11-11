use super::{AlbumRow, Index, Result};

impl Index {
    /// Query effective composer values, including the user's field edits.
    pub fn albums_by_composer(&self, composer: &str) -> Result<Vec<AlbumRow>> {
        Ok(self
            .albums()?
            .into_iter()
            .filter(|album| {
                album
                    .metadata
                    .composer
                    .as_ref()
                    .is_some_and(|field| field.value.trim() == composer.trim())
            })
            .collect())
    }
}
