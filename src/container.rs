use anyhow::{bail, Result};
use std::fs::File;
use std::path::Path;

pub struct Container {
    container_name: String,
}

impl Container {
    pub fn new(container_name: String) -> Result<Self> {
        //check if file exist
        if !Path::new(&container_name).exists() {
            File::create_new(&container_name)?;
        }
        Ok(Self { container_name })
    }
}
