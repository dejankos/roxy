use std::fs;
use std::fs::File;
use std::path::Path;

use anyhow::bail;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TestStruct {
    pub test: u8
}

pub fn load_config(path: impl AsRef<Path>) -> Result<TestStruct> {
    let str_content = load_file_content(path)?;
    Ok(serde_yaml::from_str(str_content.as_str())?)
}


fn load_file_content(path: impl AsRef<Path>) -> Result<String> {
    Ok(fs::read_to_string(path)?)
}