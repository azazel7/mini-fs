use anyhow::{bail, Ok, Result};
use bincode::Options;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;
use std::path::Path;
use std::{fs::File, io::Write};

use crate::sector::{self, EmptySector, FileData, FileMetadata, Sector};

pub struct Container {
    container_name: String,
    file: File,
    metadata: ContainerMetadata,
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

            let mut buff = Vec::with_capacity(size_of::<ContainerMetadata>());
            bincode::serialize_into(&mut buff, &metadata)?;
            buff.resize(size_of::<ContainerMetadata>(), 0);
            file.write_all(&buff)?;

            let mut buff = Vec::with_capacity(size_of::<Sector>());
            bincode::serialize_into(&mut buff, &first_sector)?;
            buff.resize(size_of::<Sector>(), 0);
            file.write_all(&buff)?;

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
            let metadata: ContainerMetadata = bincode::deserialize(&buff[..])?;
            eprintln!("Meta data {:?}", metadata);
            (file, metadata)
        };
        Ok(Self {
            container_name,
            file,
            metadata,
        })
    }
    fn read_sector(&mut self, sector_id: u64) -> Result<Sector> {
        if sector_id >= self.metadata.sector_count {
            bail!("Seeking out-of-bound sector {sector_id}");
        }
        //Skip the metadata and seek
        let offset = size_of::<ContainerMetadata>() as u64 + sector_id * size_of::<Sector>() as u64;
        let offset = SeekFrom::Start(offset);
        self.file.seek(offset)?;

        //Read the sector
        let mut buff = [0; size_of::<Sector>()];
        let read_count = self.file.read(&mut buff)?;
        if read_count < size_of::<Sector>() {
            bail!("Reading not enough byte for sector {sector_id}.");
        }

        //Deserialize
        let sector : Sector = bincode::deserialize(&buff[..])?;
        Ok(sector)
    }
    fn write_sector(&mut self, sector_id : u64, sector : &Sector) -> Result<u64> {
        if sector_id >= self.metadata.sector_count {
            bail!("Seeking out-of-bound sector {sector_id}");
        }
        //Skip the metadata and seek
        let offset = size_of::<ContainerMetadata>() as u64 + sector_id * size_of::<Sector>() as u64;
        let offset = SeekFrom::Start(offset);
        self.file.seek(offset)?;

        //Write the sector
        let bin = bincode::serialize(sector)?;
        let written = self.file.write(&bin)?.try_into()?;
        self.file.flush()?;
        if written < size_of::<Sector>() as u64 {
            bail!("Could write all bytes of sector {sector_id}");
        }
        Ok(written)
    }
}
