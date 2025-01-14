use std::{collections::HashMap, fs, path::PathBuf};

use crate::errors::InterpreterResult;

#[derive(Hash, PartialEq, Eq, Debug, Clone)]

pub struct NamedTag(u64, String);

impl NamedTag {
    pub fn new_nameless(tag: u64) -> Self {
        Self(tag, String::new())
    }
}

impl From<(u64, String)> for NamedTag {
    fn from(i: (u64, String)) -> Self {
        Self(i.0, i.1)
    }
}
#[derive(Debug, Clone)]
pub struct SourceMap(HashMap<NamedTag, String>);

impl SourceMap {
    /// Lookup the source location for the given named tag. Tags for a specific
    /// named instance are looked for first, falling back to position tags with
    /// an empty name if nothing more specific is available
    pub fn lookup(&self, key: (u64, String)) -> Option<&String> {
        let key = key.into();

        self.0
            .get(&key)
            .or_else(|| self.0.get(&NamedTag(key.0, "".to_string())))
    }

    pub fn from_file(
        path: &Option<PathBuf>,
    ) -> InterpreterResult<Option<Self>> {
        if let Some(path) = path {
            let v = fs::read(path)?;
            let file_contents = std::str::from_utf8(&v)?;
            let map: Self =
                super::metadata_parser::parse_metadata(file_contents)?;
            Ok(Some(map))
        } else {
            Ok(None)
        }
    }
}

impl From<HashMap<NamedTag, String>> for SourceMap {
    fn from(i: HashMap<NamedTag, String>) -> Self {
        Self(i)
    }
}
