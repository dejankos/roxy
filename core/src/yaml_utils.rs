use std::fs::File;
use std::path::Path;

use anyhow::Result;
use serde::de::DeserializeOwned;

pub fn yaml_to_struct<T, P>(path: P) -> Result<T>
    where
        T: DeserializeOwned,
        P: AsRef<Path>,
{
    Ok(serde_yaml::from_reader(File::open(path)?)?)
}