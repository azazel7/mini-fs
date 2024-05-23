use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;
use std::{fs::File, io::Write};
use std::mem::size_of;

use crate::sector::{FileMetadata, Sector};

pub struct Container {
    container_name: String,
    file : File,
    metadata : ContainerMetadata,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ContainerMetadata {
    root_dir_sector: u64,
    sector_count: u64,
    first_empty_sector: Option<u64>,
    next_ino: u64,
}

impl Container {
    pub fn new(container_name: String) -> Result<Self> {
        //check if file exist
        let (file, metadata) = if !Path::new(&container_name).exists() {
            //Initialize the container
            let mut file = File::create_new(&container_name)?;
            let metadata = ContainerMetadata {
                root_dir_sector: 0,
                sector_count: 1,
                first_empty_sector: None,
                next_ino: 2,
            };
            let first_sector = Sector::FileMetadata(FileMetadata::new(1, None));

            let bin = bincode::serialize(&metadata)?;
            file.write(&bin)?;
            let bin = bincode::serialize(&first_sector)?;
            file.write(&bin)?;
            file.flush()?;
            (file, metadata)
        } else {
            //Load an existing container
            let mut file = OpenOptions::new()
                .write(true)
                .read(true)
                .open(&container_name)?;
            let mut buff = [0; size_of::<ContainerMetadata>()];
            let read_count = file.read(&mut buff)?;
            if read_count < size_of::<ContainerMetadata>() {
                bail!("The file {container_name} is smaller than the container metadata.");
            }
            let metadata : ContainerMetadata = bincode::deserialize(&buff[..])?;
            (file, metadata)
        };
        Ok(Self { container_name , file, metadata})
    }
}
