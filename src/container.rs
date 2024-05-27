use anyhow::{bail, Ok, Result};
use fuser::{FileType, ReplyDirectory};
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;
use std::path::Path;
use std::str::FromStr;
use std::usize;
use std::{fs::File, io::Write};

use crate::sector::{self, DirData, EmptySector, FileData, FileMetadata, Sector, DATA_CHUNK_SIZE};

use sector::FILE_NAME_SIZE;

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
    last_empty_sector: Option<u64>,
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
                last_empty_sector: None,
                next_ino: 2,
            };
            let first_sector = Sector::DirMetadata(FileMetadata::new(1, None));

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
        let sector: Sector = bincode::deserialize(&buff[..])?;
        Ok(sector)
    }
    fn write_metadata(&mut self) -> Result<()> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut buff = Vec::with_capacity(size_of::<ContainerMetadata>());
        bincode::serialize_into(&mut buff, &self.metadata)?;
        buff.resize(size_of::<ContainerMetadata>(), 0);
        self.file.write_all(&buff)?;
        Ok(())
    }
    fn write_sector(&mut self, sector_id: u64, sector: &Sector) -> Result<u64> {
        if sector_id >= self.metadata.sector_count {
            bail!("Seeking out-of-bound sector {sector_id}");
        }
        //Skip the metadata and seek
        let offset = size_of::<ContainerMetadata>() as u64 + sector_id * size_of::<Sector>() as u64;
        let offset = SeekFrom::Start(offset);
        self.file.seek(offset)?;

        //Write the sector
        let mut buff = Vec::with_capacity(size_of::<Sector>());
        bincode::serialize_into(&mut buff, sector)?;
        buff.resize(size_of::<Sector>(), 0);
        self.file.write_all(&buff)?;
        self.file.flush()?;
        Ok(size_of::<Sector>() as u64)
    }
    fn append_empty_sector(&mut self) -> Result<u64> {
        let mut empty_sector = EmptySector::default();
        //Set previous if any
        if let Some(last_sector) = self.metadata.last_empty_sector {
            empty_sector.set_previous(last_sector);
        }
        //Place the cursor
        let offset = size_of::<ContainerMetadata>() as u64
            + self.metadata.sector_count * size_of::<Sector>() as u64; //TODO maybe add a -1
        let offset = SeekFrom::Start(offset);
        self.file.seek(offset)?;
        //Write the empty sector
        let mut buff = Vec::with_capacity(size_of::<Sector>());
        bincode::serialize_into(&mut buff, &Sector::Empty(empty_sector))?;
        buff.resize(size_of::<Sector>(), 0);
        self.file.write_all(&buff)?;
        self.file.flush()?;

        //Modify the previous last_empty_sector if any
        if let Some(last_empty_sector_id) = self.metadata.last_empty_sector {
            //Read and check for emptyness
            let Sector::Empty(mut last_empty_sector) = self.read_sector(last_empty_sector_id)?
            else {
                bail!("Last empty sector {last_empty_sector_id} is not empty.");
            };
            last_empty_sector.set_next(self.metadata.sector_count);
            self.write_sector(last_empty_sector_id, &Sector::Empty(last_empty_sector))?;
        }

        //If this one is the first empty sector update the list
        if self.metadata.first_empty_sector.is_none() {
            self.metadata.first_empty_sector = Some(self.metadata.sector_count);
        }
        self.metadata.last_empty_sector = Some(self.metadata.sector_count);
        self.metadata.sector_count += 1;
        self.write_metadata()?;
        Ok(1)
    }
    fn get_empty_sector(&mut self) -> Result<u64> {
        if let Some(empty_sector_id) = self.metadata.first_empty_sector {
            let Sector::Empty(empty_sector_data) = self.read_sector(empty_sector_id)? else {
                bail!("Empty sector is not true empty sector");
            };
            if self.metadata.last_empty_sector == self.metadata.first_empty_sector {
                self.metadata.first_empty_sector = None;
                self.metadata.last_empty_sector = None;
            } else {
                self.metadata.first_empty_sector = empty_sector_data.next();
            }
            self.write_metadata()?;
            return Ok(empty_sector_id);
        }
        self.append_empty_sector()?;
        if let Some(empty_sector_id) = self.metadata.first_empty_sector {
            let Sector::Empty(empty_sector_data) = self.read_sector(empty_sector_id)? else {
                bail!("Empty sector is not true empty sector");
            };
            if self.metadata.last_empty_sector == self.metadata.first_empty_sector {
                self.metadata.first_empty_sector = None;
                self.metadata.last_empty_sector = None;
            } else {
                self.metadata.first_empty_sector = empty_sector_data.next();
            }
            self.write_metadata()?;
            return Ok(empty_sector_id);
        } else {
            bail!("No empty sector available");
        }
    }
    fn get_empty_entry(&mut self, dir_metadata: &FileMetadata) -> Result<Option<(u64, usize)>> {
        let mut next_sector = dir_metadata.first_sector();

        //Iterate through all sector of directory
        while let Some(sector_id) = next_sector {
            let base_sector = self.read_sector(sector_id)?;
            let Sector::DirData(sector) = &base_sector else {
                bail!(
                    "Directory sector is not DirData (inode {}, sector {sector_id})",
                    dir_metadata.ino()
                );
            };
            //Look for an empty entry
            for (i, entry) in sector.entries().iter().enumerate() {
                if entry.empty {
                    //Write the entry
                    return Ok(Some((sector_id, i)));
                }
            }
            next_sector = sector.next_sector();
        }
        Ok(None)
    }
    fn find_ino_sector(&mut self, ino: u64) -> Result<(u64, Sector)> {
        for i in 0..self.metadata.sector_count {
            let sector = self.read_sector(i)?;
            if let Sector::DirMetadata(ref dir_metadata) = sector {
                if dir_metadata.ino() == ino {
                    return Ok((i, sector));
                }
            } else if let Sector::FileMetadata(ref file_metadata) = sector {
                if file_metadata.ino() == ino {
                    return Ok((i, sector));
                }
            }
        }
        bail!("Inode {ino} not found");
    }
    fn new_inode(&mut self) -> Result<u64> {
        if self.metadata.next_ino == u64::max_value() {
            bail!("No more inode");
        }
        self.metadata.next_ino += 1;
        Ok(self.metadata.next_ino - 1)
    }
    fn free_sector(&mut self, sector_id: u64) -> Result<()> {
        let mut empty_sector = EmptySector::default();
        if let Sector::Empty(_) = self.read_sector(sector_id)? {
            //Sector is already empty
            return Ok(());
        }
        if let Some(first_empty_sector_id) = self.metadata.first_empty_sector {
            let mut base_first_empty_sector = self.read_sector(first_empty_sector_id)?;
            let Sector::Empty(first_empty_sector) = &mut base_first_empty_sector else {
                bail!("First empty sector ({first_empty_sector_id}) is not an empty sector.");
            };
            first_empty_sector.set_previous(sector_id);
            self.write_sector(first_empty_sector_id, &base_first_empty_sector)?;
            empty_sector.set_next(first_empty_sector_id);
        }
        self.write_sector(sector_id, &Sector::Empty(empty_sector))?;
        self.metadata.first_empty_sector = Some(sector_id);
        if self.metadata.last_empty_sector.is_none() {
            self.metadata.last_empty_sector = Some(sector_id);
        }
        self.write_metadata()?;
        Ok(())
    }
    fn delete_file(&mut self, ino: u64) -> Result<()> {
        let (metadata_sector_id, metadata_sector) = self.find_ino_sector(ino)?;
        let Sector::FileMetadata(file_metadata) = &metadata_sector else {
            bail!("Inode {ino} is not a file.");
        };
        let mut current_sector_id = file_metadata.first_sector();
        self.free_sector(metadata_sector_id)?;

        while let Some(sector_id) = current_sector_id {
            let Sector::FileData(file_data) = self.read_sector(sector_id)? else {
                bail!("Sector is not of type FileData.");
            };
            self.free_sector(sector_id)?;
            current_sector_id = file_data.next();
        }

        Ok(())
    }
    pub fn opendir(&mut self, ino: u64) -> Result<u64> {
        let (sector_id, sector) = self.find_ino_sector(ino)?;
        Ok(1)
    }
    pub fn readdir(
        &mut self,
        ino: u64,
        _fh: u64,
        offset: i64,
    ) -> Result<Vec<(u64, FileType, String)>> {
        let mut index = 0;

        let (_sector_id, sector) = self.find_ino_sector(ino)?;
        let Sector::DirMetadata(dir_metadata) = sector else {
            bail!("Inode {ino} is not a directory.");
        };
        let mut next_sector = dir_metadata.first_sector();
        let mut entry_list = Vec::new();

        entry_list.push((ino, FileType::Directory, ".".to_string()));
        entry_list.push((ino, FileType::Directory, "..".to_string()));

        //Iterate through all sector of directory
        while let Some(sector_id) = next_sector {
            let base_sector = self.read_sector(sector_id)?;
            let Sector::DirData(sector) = base_sector else {
                eprintln!("{base_sector:?}");
                bail!("Directory sector is not DirData (inode {ino}, sector {sector_id})");
            };
            for entry in sector.entries() {
                if entry.empty {
                    continue;
                }
                if index >= offset {
                    let filetype = match entry.filetype {
                        sector::FileType::Regular => FileType::RegularFile,
                        sector::FileType::Directory => FileType::Directory,
                    };
                    entry_list.push((entry.ino, filetype, entry.name.to_string()));
                }
                index += 1;
            }
            next_sector = sector.next_sector();
        }

        Ok(entry_list)
    }
    pub fn create(&mut self, parent: u64, name: &OsStr, filetype: sector::FileType) -> Result<u64> {
        let Some(name) = name.to_str() else {
            bail!("Invalid name");
        };
        if name.len() >= FILE_NAME_SIZE {
            bail!("Name is too long (max {FILE_NAME_SIZE})");
        }
        let (metadata_sector_id, mut metadata_sector) = self.find_ino_sector(parent)?;
        let Sector::DirMetadata(dir_metadata) = &mut metadata_sector else {
            bail!("Inode {parent} is not a directory.");
        };
        let new_inode = self.new_inode()?;
        let empty_sector_id_file_metadata = self.get_empty_sector()?;
        //TODO check if name already exist
        let findings = self.get_empty_entry(dir_metadata)?;

        let (sector_id, idx) = if let Some(a) = findings {
            a
        } else {
            //append_empty_sector
            let empty_sector_id = self.get_empty_sector()?;
            //set the sector data
            let mut sector = DirData::new();
            if let Some(previous_id) = dir_metadata.first_sector() {
                sector.set_previous(previous_id);
            }
            self.write_sector(empty_sector_id, &Sector::DirData(sector))?;
            //modify metadata to emplace it at the front of the sector list
            dir_metadata.set_first_sector(empty_sector_id);
            dir_metadata.increase_length_sector();
            self.write_sector(metadata_sector_id, &metadata_sector)?;
            (empty_sector_id, 0)
        };

        //Read the sector in memory
        let mut base_sector = self.read_sector(sector_id)?;
        let Sector::DirData(sector) = &mut base_sector else {
            bail!("Directory sector is not DirData (inode {parent}, sector {sector_id})");
        };

        //Should always be valid because it should have failed earlier otherwise (no new empty sector)
        let entry = sector.entries_mut().get_mut(idx).unwrap();

        //Write the entry
        entry.ino = new_inode;
        entry.empty = false;
        let std::result::Result::Ok(heapless_name) =
            heapless::String::<FILE_NAME_SIZE>::from_str(name)
        else {
            bail!("Error heapless::String::from_str for the filename.");
        };
        entry.name = heapless_name;
        entry.filetype = filetype;

        //Write to container
        self.write_sector(sector_id, &base_sector)?;

        // write metadata of new file
        let empty_sector_id = empty_sector_id_file_metadata;
        let sector = FileMetadata::new(new_inode, Some(parent));
        let sector = match filetype {
            sector::FileType::Regular => Sector::FileMetadata(sector),
            sector::FileType::Directory => Sector::DirMetadata(sector),
        };
        self.write_sector(empty_sector_id, &sector)?;

        Ok(new_inode)
    }
    pub fn getattr(&mut self, ino: u64) -> Result<Option<FileType>> {
        let (_sector_id, sector) = self.find_ino_sector(ino)?;
        if let Sector::DirMetadata(_) = sector {
            return Ok(Some(FileType::Directory));
        } else if let Sector::FileMetadata(_) = sector {
            return Ok(Some(FileType::RegularFile));
        }
        Ok(None)
    }
    pub fn lookup(&mut self, parent: u64, name: &OsStr) -> Result<Option<(u64, FileType)>> {
        let (_metadata_sector_id, mut metadata_sector) = self.find_ino_sector(parent)?;
        let Sector::DirMetadata(dir_metadata) = &mut metadata_sector else {
            bail!("Inode {parent} is not a directory.");
        };
        let mut next_sector = dir_metadata.first_sector();

        //Iterate through all sector of directory
        while let Some(sector_id) = next_sector {
            let base_sector = self.read_sector(sector_id)?;
            let Sector::DirData(sector) = &base_sector else {
                bail!("Directory sector is not DirData (inode {parent}, sector {sector_id})");
            };
            //Look for used entry
            for entry in sector.entries() {
                if !entry.empty {
                    let ename = OsString::from(entry.name.to_string());
                    if ename == *name {
                        let filetype = match entry.filetype {
                            sector::FileType::Directory => FileType::Directory,
                            sector::FileType::Regular => FileType::RegularFile,
                        };
                        return Ok(Some((entry.ino, filetype)));
                    }
                }
            }
            next_sector = sector.next_sector();
        }
        Ok(None)
    }
    pub fn unlink(&mut self, parent: u64, name: &OsStr) -> Result<()> {
        let (_metadata_sector_id, mut metadata_sector) = self.find_ino_sector(parent)?;
        let Sector::DirMetadata(dir_metadata) = &mut metadata_sector else {
            bail!("Inode {parent} is not a directory.");
        };
        let mut next_sector = dir_metadata.first_sector();

        let mut ino = None;
        //Iterate through all sector of directory
        while let Some(sector_id) = next_sector {
            let mut base_sector = self.read_sector(sector_id)?;
            let Sector::DirData(sector) = &mut base_sector else {
                bail!("Directory sector is not DirData (inode {parent}, sector {sector_id})");
            };
            //Look for entry with the right name
            for entry in sector.entries_mut() {
                if !entry.empty {
                    let ename = OsString::from(entry.name.to_string());
                    if ename == *name {
                        if entry.filetype == sector::FileType::Directory {
                            bail!("{name:?} is a directory.");
                        }
                        //Free entry and set ino
                        ino = Some(entry.ino);
                        entry.empty = true;
                        entry.ino = 0;
                        entry.name = heapless::String::new();
                        break;
                    }
                }
            }
            next_sector = sector.next_sector();
            if ino.is_some() {
                //The file has been found so we write it back
                self.write_sector(sector_id, &base_sector)?;
                break;
            }
        }
        //If the name has not been found, return Ok. No problem encountered
        let Some(ino) = ino else {
            return Ok(());
        };
        self.delete_file(ino)?;
        Ok(())
    }
    pub fn write(&mut self, ino: u64, offset: i64, data: &[u8]) -> Result<u64> {
        //TODO What is offset? The offset base on the beginning of a file or the hyphothetical
        //cursor?
        if offset < 0 {
            bail!("Writing at a negative offset (offset={offset})");
        }
        let offset = offset as u64;
        let (metadata_sector_id, mut metadata_sector) = self.find_ino_sector(ino)?;
        let Sector::FileMetadata(file_metadata) = &mut metadata_sector else {
            bail!("Inode {ino} is not a directory.");
        };
        let offset = if offset > file_metadata.length_byte() {
            file_metadata.length_byte()
        } else {
            offset
        };

        let mut current_sector_id = file_metadata.first_sector();
        let mut file_index = 0;
        let mut data_index = 0;
        let mut sector_index = 0;

        let mut previous_sector_id = None;

        let mut starting_sector = None;
        //find offset
        while let Some(sector_id) = current_sector_id {
            let mut sector = self.read_sector(sector_id)?;
            let Sector::FileData(sector_data) = &mut sector else {
                bail!("Sector {sector_id} (ino {ino} is not a FileData {sector:?}");
            };
            if offset >= file_index + DATA_CHUNK_SIZE as u64 {
                current_sector_id = sector_data.next();
                file_index += DATA_CHUNK_SIZE as u64;
                continue;
            }

            //Check if offset starts at this sector
            if offset >= file_index && offset < file_index + DATA_CHUNK_SIZE as u64 {
                starting_sector = Some(sector_id);
                sector_index = (offset - file_index) as usize;
                previous_sector_id = Some(sector_id);
                break;
            }
        }

        let mut current_sector_id = if let Some(a) = starting_sector {
            a
        } else {
            //Get an empty sector if needed
            let empty_sector_id = self.get_empty_sector()?;
            file_metadata.increase_length_sector();
            let mut next_sector = FileData::new();
            //Setup the sector
            if let Some(previous_sector_id) = previous_sector_id {
                next_sector.set_previous(previous_sector_id);
            } else {
                file_metadata.set_first_sector(empty_sector_id);
            }
            //Write it
            self.write_sector(empty_sector_id, &Sector::FileData(next_sector))?;
            empty_sector_id
        };
        let mut total_data_diff = 0;

        //Loop through the sectors to write the data
        loop {
            let mut sector = self.read_sector(current_sector_id)?;
            let Sector::FileData(sector_data) = &mut sector else {
                bail!("Sector {current_sector_id} (ino {ino} is not a FileData {sector:?}");
            };
            //Remaining qty to write from data
            let data_qty = (data.len() - data_index) as usize;
            //Maximum qty writable in that sector
            let write_qty = data_qty.min(DATA_CHUNK_SIZE - sector_index as usize) as usize;

            //Write the data into the FileData struct
            let slice = &data[data_index..data_index + write_qty as usize];
            sector_data.write(slice, sector_index, sector_index + write_qty);
            let prev_data_length = sector_data.data_length() as usize;

            let length_diff = if sector_index + write_qty < prev_data_length {
                0
            } else {
                sector_index + write_qty - prev_data_length as usize
            };
            total_data_diff += length_diff;
            sector_data.set_data_length((prev_data_length + length_diff) as u64);

            //Update index
            data_index += write_qty;
            if data_index == data.len() {
                //We are done writing
                self.write_sector(current_sector_id, &sector)?;
                break;
            }
            sector_index = 0;
            file_index += DATA_CHUNK_SIZE as u64;

            //Append a new sector if needed
            if sector_data.next().is_none() {
                let empty_sector_id = self.get_empty_sector()?;
                file_metadata.increase_length_sector();
                sector_data.set_next(empty_sector_id);
                let mut next_sector = FileData::new();
                next_sector.set_previous(current_sector_id);
                self.write_sector(empty_sector_id, &Sector::FileData(next_sector))?;
            }
            let next_sector_id = sector_data.next().unwrap();
            self.write_sector(current_sector_id, &sector)?;
            current_sector_id = next_sector_id;
        }

        file_metadata.increase_length_byte(total_data_diff as u64);
        self.write_sector(metadata_sector_id, &metadata_sector)?;

        Ok(data.len().try_into()?)
    }
    pub fn read(&mut self, ino: u64, offset: i64, size: u64, data: &mut Vec<u8>) -> Result<u64> {
        //TODO What is offset? The offset base on the beginning of a file or the hyphothetical
        //cursor?
        if offset < 0 {
            bail!("Reading at a negative offset (offset={offset})");
        }
        let offset = offset as u64;
        let (_metadata_sector_id, mut metadata_sector) = self.find_ino_sector(ino)?;
        let Sector::FileMetadata(file_metadata) = &mut metadata_sector else {
            bail!("Inode {ino} is not a directory.");
        };
        if offset > file_metadata.length_byte() {
            return Ok(0);
        }

        let mut current_sector_id = file_metadata.first_sector();
        let mut file_index = 0;
        let mut sector_index = 0;

        let mut starting_sector = None;
        //find offset
        while let Some(sector_id) = current_sector_id {
            let mut sector = self.read_sector(sector_id)?;
            let Sector::FileData(sector_data) = &mut sector else {
                bail!("Sector {sector_id} (ino {ino} is not a FileData {sector:?}");
            };
            if offset >= file_index + DATA_CHUNK_SIZE as u64 {
                current_sector_id = sector_data.next();
                file_index += DATA_CHUNK_SIZE as u64;
                continue;
            }

            //Check if offset starts at this sector
            if offset >= file_index && offset < file_index + DATA_CHUNK_SIZE as u64 {
                starting_sector = Some(sector_id);
                sector_index = (offset - file_index) as usize;
                break;
            }
        }

        let mut current_sector_id = if let Some(a) = starting_sector {
            a
        } else {
            bail!("Couldn't find the offset");
        };

        //Loop through the sectors to read the data
        loop {
            let mut sector = self.read_sector(current_sector_id)?;
            let Sector::FileData(sector_data) = &mut sector else {
                bail!("Sector {current_sector_id} (ino {ino} is not a FileData {sector:?}");
            };
            //Remaining qty to read from data
            let data_qty = size as usize - data.len();
            //Maximum qty readable in that sector
            let read_qty = data_qty.min(sector_data.data_length() as usize - sector_index as usize);

            //Read the data from FileData struct
            let slice = &sector_data.data()[sector_index..sector_index + read_qty];
            data.extend_from_slice(slice);

            //Update index
            if size == data.len() as u64 {
                //We are done reading
                break;
            }
            sector_index = 0;
            file_index += DATA_CHUNK_SIZE as u64;

            //Append a new sector if needed
            if let Some(next_sector_id) = sector_data.next() {
                current_sector_id = next_sector_id;
            } else {
                //EOF
                break;
            }
        }

        Ok(data.len().try_into()?)
    }
}

#[cfg(test)]
mod tests {
    use crate::sector::FileData;

    use super::*;
    use std::{collections::HashSet, fs::remove_file};

    #[test]
    fn append_empty_sector() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        let sector_count = container.metadata.sector_count;
        container.append_empty_sector().unwrap();
        assert_eq!(container.metadata.sector_count, sector_count + 1);
        assert_eq!(container.metadata.last_empty_sector, Some(sector_count));
        remove_file("/tmp/canard").unwrap();
    }
    #[test]
    fn read_write_sector() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        assert_eq!(container.metadata.sector_count, 4); //3 empty+the root = 4
        let sector = container.read_sector(1).unwrap();
        if let Sector::Empty(sector) = sector {
            assert_eq!(sector.previous(), None);
            assert_eq!(sector.next(), Some(2));
        }
        let sector = container.read_sector(2).unwrap();
        if let Sector::Empty(sector) = sector {
            assert_eq!(sector.previous(), Some(1));
            assert_eq!(sector.next(), Some(3));
        }
        let sector = container.read_sector(3).unwrap();
        if let Sector::Empty(sector) = sector {
            assert_eq!(sector.previous(), Some(2));
            assert_eq!(sector.next(), None);
        }
        remove_file("/tmp/canard").unwrap();
    }
    #[test]
    fn free_sector() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        container
            .write_sector(1, &Sector::FileData(FileData::new()))
            .unwrap();
        container
            .write_sector(2, &Sector::DirMetadata(FileMetadata::new(2, None)))
            .unwrap();
        container
            .write_sector(3, &Sector::FileData(FileData::new()))
            .unwrap();
        container
            .write_sector(4, &Sector::DirMetadata(FileMetadata::new(3, None)))
            .unwrap();
        //NOTE since we forced a writing, the metadata are not up to date
        //write_sector are not doing any checking of what is written
        container.metadata.first_empty_sector = None;
        container.metadata.last_empty_sector = None;

        assert!(matches!(
            container.read_sector(1).unwrap(),
            Sector::FileData(_)
        ));
        assert!(matches!(
            container.read_sector(2).unwrap(),
            Sector::DirMetadata(_)
        ));
        assert!(matches!(
            container.read_sector(3).unwrap(),
            Sector::FileData(_)
        ));
        assert!(matches!(
            container.read_sector(4).unwrap(),
            Sector::DirMetadata(_)
        ));

        container.free_sector(2).unwrap();

        assert!(matches!(
            container.read_sector(1).unwrap(),
            Sector::FileData(_)
        ));
        assert!(matches!(
            container.read_sector(2).unwrap(),
            Sector::Empty(_)
        ));
        assert!(matches!(
            container.read_sector(3).unwrap(),
            Sector::FileData(_)
        ));
        assert!(matches!(
            container.read_sector(4).unwrap(),
            Sector::DirMetadata(_)
        ));
        assert_eq!(container.metadata.first_empty_sector, Some(2));
        assert_eq!(container.metadata.last_empty_sector, Some(2));

        container.free_sector(3).unwrap();

        assert!(matches!(
            container.read_sector(1).unwrap(),
            Sector::FileData(_)
        ));
        assert!(matches!(
            container.read_sector(2).unwrap(),
            Sector::Empty(_)
        ));
        assert!(matches!(
            container.read_sector(3).unwrap(),
            Sector::Empty(_)
        ));
        assert!(matches!(
            container.read_sector(4).unwrap(),
            Sector::DirMetadata(_)
        ));
        assert_eq!(container.metadata.first_empty_sector, Some(3));
        assert_eq!(container.metadata.last_empty_sector, Some(2));

        //Try to double free but everything should remain the same
        container.free_sector(3).unwrap();

        assert!(matches!(
            container.read_sector(1).unwrap(),
            Sector::FileData(_)
        ));
        assert!(matches!(
            container.read_sector(2).unwrap(),
            Sector::Empty(_)
        ));
        assert!(matches!(
            container.read_sector(3).unwrap(),
            Sector::Empty(_)
        ));
        assert!(matches!(
            container.read_sector(4).unwrap(),
            Sector::DirMetadata(_)
        ));
        assert_eq!(container.metadata.first_empty_sector, Some(3));
        assert_eq!(container.metadata.last_empty_sector, Some(2));

        remove_file("/tmp/canard").unwrap();
    }
    #[test]
    fn delete_file() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        let mut file_data = FileData::new();
        file_data.set_next(4);
        container
            .write_sector(1, &Sector::FileData(file_data))
            .unwrap();
        let mut file_metadata = FileMetadata::new(7, None);
        file_metadata.set_first_sector(1);
        container
            .write_sector(2, &Sector::FileMetadata(file_metadata))
            .unwrap();
        let mut file_data = FileData::new();
        file_data.set_previous(4);
        container
            .write_sector(3, &Sector::FileData(file_data))
            .unwrap();
        let mut file_data = FileData::new();
        file_data.set_next(3);
        file_data.set_previous(1);
        container
            .write_sector(4, &Sector::FileData(file_data))
            .unwrap();
        //NOTE since we forced a writing, the metadata are not up to date
        //write_sector are not doing any checking of what is written
        container.metadata.first_empty_sector = None;
        container.metadata.last_empty_sector = None;

        assert!(matches!(
            container.read_sector(1).unwrap(),
            Sector::FileData(_)
        ));
        assert!(matches!(
            container.read_sector(2).unwrap(),
            Sector::FileMetadata(_)
        ));
        assert!(matches!(
            container.read_sector(3).unwrap(),
            Sector::FileData(_)
        ));
        assert!(matches!(
            container.read_sector(4).unwrap(),
            Sector::FileData(_)
        ));

        container.delete_file(7).unwrap();

        assert!(matches!(
            container.read_sector(1).unwrap(),
            Sector::Empty(_)
        ));
        assert!(matches!(
            container.read_sector(2).unwrap(),
            Sector::Empty(_)
        ));
        assert!(matches!(
            container.read_sector(3).unwrap(),
            Sector::Empty(_)
        ));
        assert!(matches!(
            container.read_sector(4).unwrap(),
            Sector::Empty(_)
        ));

        remove_file("/tmp/canard").unwrap();
    }
    #[test]
    fn get_empty_sector() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        container.append_empty_sector().unwrap();
        assert_eq!(container.metadata.sector_count, 2);
        let empty_sector = container.get_empty_sector().unwrap();
        assert_eq!(empty_sector, 1);
        remove_file("/tmp/canard").unwrap();
    }
    #[test]
    fn find_ino_sector() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        let new_inode = container
            .create(1, OsStr::new("loutre.txt"), sector::FileType::Regular)
            .unwrap();
        let (sector_id, _sector) = container.find_ino_sector(1).unwrap();
        assert_eq!(sector_id, 0); //Root directory
        let ret = container.find_ino_sector(new_inode);
        assert!(ret.is_ok());
        let ret = container.find_ino_sector(37);
        assert!(ret.is_err());
        remove_file("/tmp/canard").unwrap();
    }
    #[test]
    fn getattr() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        let new_inode = container
            .create(1, OsStr::new("loutre.txt"), sector::FileType::Regular)
            .unwrap();

        let filetype = container.getattr(new_inode).unwrap();
        assert_eq!(filetype, Some(FileType::RegularFile));
        let filetype = container.getattr(1).unwrap();
        assert_eq!(filetype, Some(FileType::Directory));
        let filetype = container.getattr(37);
        assert!(filetype.is_err());
        remove_file("/tmp/canard").unwrap();
    }
    #[test]
    fn readdir() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        let inode1 = container
            .create(1, OsStr::new("loutre.txt"), sector::FileType::Regular)
            .unwrap();
        let inode2 = container
            .create(1, OsStr::new("canard.txt"), sector::FileType::Regular)
            .unwrap();

        let entries = container.readdir(1, 1, 0).unwrap();
        let entries_names = entries.iter().map(|e| e.2.clone()).collect::<HashSet<_>>();
        let entries_inode = entries.iter().map(|e| e.0.clone()).collect::<HashSet<_>>();
        assert_eq!(entries.len(), 4); //".", "..", "loutre.txt", "canard.txt"
        assert!(entries_names.contains("."));
        assert!(entries_names.contains(".."));
        assert!(entries_names.contains("loutre.txt"));
        assert!(entries_names.contains("canard.txt"));
        assert!(entries_inode.contains(&inode1));
        assert!(entries_inode.contains(&inode2));

        let inode3 = container
            .create(1, OsStr::new("baleine.txt"), sector::FileType::Regular)
            .unwrap();
        let entries = container.readdir(1, 1, 0).unwrap();
        let entries_names = entries.iter().map(|e| e.2.clone()).collect::<HashSet<_>>();
        let entries_inode = entries.iter().map(|e| e.0.clone()).collect::<HashSet<_>>();
        assert_eq!(entries.len(), 5); //  "baleine.txt"
        assert!(entries_names.contains("."));
        assert!(entries_names.contains(".."));
        assert!(entries_names.contains("loutre.txt"));
        assert!(entries_names.contains("canard.txt"));
        assert!(entries_names.contains("baleine.txt"));
        assert!(entries_inode.contains(&inode1));
        assert!(entries_inode.contains(&inode2));
        assert!(entries_inode.contains(&inode3));

        remove_file("/tmp/canard").unwrap();
    }
    #[test]
    fn lookup() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        let inode1 = container
            .create(1, OsStr::new("loutre.txt"), sector::FileType::Regular)
            .unwrap();
        let inode2 = container
            .create(1, OsStr::new("canard.txt"), sector::FileType::Regular)
            .unwrap();

        let finding = container.lookup(1, OsStr::new("loutre.txt")).unwrap();
        assert!(finding.is_some());
        let (ino, filetype) = finding.unwrap();
        assert_eq!(ino, inode1);
        assert_eq!(filetype, FileType::RegularFile);

        let finding = container.lookup(1, OsStr::new("canard.txt")).unwrap();
        assert!(finding.is_some());
        let (ino, filetype) = finding.unwrap();
        assert_eq!(ino, inode2);
        assert_eq!(filetype, FileType::RegularFile);

        remove_file("/tmp/canard").unwrap();
    }
    #[test]
    fn write() {
        let container_name = "/tmp/canard_write";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();

        let file_inode = container
            .create(1, OsStr::new("canard.txt"), sector::FileType::Regular)
            .unwrap();

        //Phase 1, First simple write
        let data = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let written = container.write(file_inode, 0, &data).unwrap();
        assert_eq!(written, 10);
        let (_metadata_sector_id, Sector::FileMetadata(file_metadata)) =
            container.find_ino_sector(file_inode).unwrap()
        else {
            panic!("Sector is not FileMetadata.");
        };
        assert_eq!(file_metadata.length_byte(), 10);
        assert!(file_metadata.first_sector().is_some());
        let sector_id = file_metadata.first_sector().unwrap();
        let Sector::FileData(sector_data) = container.read_sector(sector_id).unwrap() else {
            panic!("Sector is not FileData.");
        };
        assert_eq!(data, &sector_data.data()[0..10]);

        //Phase 2, new write
        let data = [17; DATA_CHUNK_SIZE];
        let written = container.write(file_inode, 10, &data).unwrap();
        assert_eq!(written, DATA_CHUNK_SIZE as u64);
        let (_metadata_sector_id, Sector::FileMetadata(file_metadata)) =
            container.find_ino_sector(file_inode).unwrap()
        else {
            panic!("Sector is not FileMetadata.");
        };
        assert_eq!(file_metadata.length_byte(), DATA_CHUNK_SIZE as u64 + 10);
        assert!(file_metadata.first_sector().is_some());
        let sector_id = file_metadata.first_sector().unwrap();
        let Sector::FileData(sector_data) = container.read_sector(sector_id).unwrap() else {
            panic!("Sector is not FileData.");
        };
        let mut sector_1_data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        sector_1_data.resize(DATA_CHUNK_SIZE, 17);
        assert_eq!(
            &sector_1_data[0..DATA_CHUNK_SIZE],
            &sector_data.data()[0..DATA_CHUNK_SIZE]
        );

        assert!(sector_data.next().is_some());
        let sector_id = sector_data.next().unwrap();
        let Sector::FileData(sector_data) = container.read_sector(sector_id).unwrap() else {
            panic!("Sector is not FileData.");
        };
        let sector_2_data = vec![17; 10];
        assert_eq!(sector_data.data_length(), 10);
        assert_eq!(&sector_2_data[0..10], &sector_data.data()[0..10]);

        //Phase 3, write in the middle
        let data = vec![42; 10];
        let written = container
            .write(file_inode, DATA_CHUNK_SIZE as i64 - 5, &data)
            .unwrap();
        assert_eq!(written, 10);
        let (_metadata_sector_id, Sector::FileMetadata(file_metadata)) =
            container.find_ino_sector(file_inode).unwrap()
        else {
            panic!("Sector is not FileMetadata.");
        };
        assert_eq!(file_metadata.length_byte(), DATA_CHUNK_SIZE as u64 + 10);
        assert!(file_metadata.first_sector().is_some());
        let sector_id = file_metadata.first_sector().unwrap();
        let Sector::FileData(sector_data) = container.read_sector(sector_id).unwrap() else {
            panic!("Sector is not FileData.");
        };
        let mut sector_1_data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        sector_1_data.resize(DATA_CHUNK_SIZE - 5, 17);
        sector_1_data.resize(DATA_CHUNK_SIZE, 42);
        assert_eq!(
            &sector_1_data[0..DATA_CHUNK_SIZE],
            &sector_data.data()[0..DATA_CHUNK_SIZE]
        );

        assert!(sector_data.next().is_some());
        let sector_id = sector_data.next().unwrap();
        let Sector::FileData(sector_data) = container.read_sector(sector_id).unwrap() else {
            panic!("Sector is not FileData.");
        };
        let sector_2_data = vec![42, 42, 42, 42, 42, 17, 17, 17, 17, 17];
        assert_eq!(sector_data.data_length(), 10);
        assert_eq!(&sector_2_data[0..10], &sector_data.data()[0..10]);

        //Phase 4, big write
        let data = vec![91; DATA_CHUNK_SIZE * 3];
        let written = container
            .write(file_inode, DATA_CHUNK_SIZE as i64 - 5, &data)
            .unwrap();
        assert_eq!(written, DATA_CHUNK_SIZE as u64 * 3);
        let (_metadata_sector_id, Sector::FileMetadata(file_metadata)) =
            container.find_ino_sector(file_inode).unwrap()
        else {
            panic!("Sector is not FileMetadata.");
        };
        assert_eq!(
            file_metadata.length_byte(),
            (DATA_CHUNK_SIZE as u64 * 4) - 5
        );
        assert!(file_metadata.first_sector().is_some());
        let sector_id = file_metadata.first_sector().unwrap();
        let Sector::FileData(sector_data) = container.read_sector(sector_id).unwrap() else {
            panic!("Sector is not FileData.");
        };
        let mut sector_1_data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        sector_1_data.resize(DATA_CHUNK_SIZE - 5, 17);
        sector_1_data.resize(DATA_CHUNK_SIZE, 91);
        assert_eq!(
            &sector_1_data[0..DATA_CHUNK_SIZE],
            &sector_data.data()[0..DATA_CHUNK_SIZE]
        );
        assert!(sector_data.next().is_some());
        let sector_id = sector_data.next().unwrap();
        let Sector::FileData(sector_data) = container.read_sector(sector_id).unwrap() else {
            panic!("Sector is not FileData.");
        };
        let sector_2_data = vec![91; DATA_CHUNK_SIZE];
        assert_eq!(
            &sector_2_data[0..DATA_CHUNK_SIZE],
            &sector_data.data()[0..DATA_CHUNK_SIZE]
        );

        assert!(sector_data.next().is_some());
        let sector_id = sector_data.next().unwrap();
        let Sector::FileData(sector_data) = container.read_sector(sector_id).unwrap() else {
            panic!("Sector is not FileData.");
        };
        let sector_3_data = vec![91; DATA_CHUNK_SIZE];
        assert_eq!(
            &sector_3_data[0..DATA_CHUNK_SIZE],
            &sector_data.data()[0..DATA_CHUNK_SIZE]
        );

        assert!(sector_data.next().is_some());
        let sector_id = sector_data.next().unwrap();
        let Sector::FileData(sector_data) = container.read_sector(sector_id).unwrap() else {
            panic!("Sector is not FileData.");
        };
        let sector_4_data = vec![91; DATA_CHUNK_SIZE - 5];
        assert_eq!(sector_data.data_length(), DATA_CHUNK_SIZE as u64 - 5);
        assert_eq!(
            &sector_4_data[0..DATA_CHUNK_SIZE - 5],
            &sector_data.data()[0..DATA_CHUNK_SIZE - 5]
        );

        remove_file(container_name).unwrap();
    }
    #[test]
    fn read() {
        let container_name = "/tmp/canard_read";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();

        let file_inode = container
            .create(1, OsStr::new("canard.txt"), sector::FileType::Regular)
            .unwrap();

        //Phase 1, First simple write
        let mut data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        data.resize(DATA_CHUNK_SIZE + data.len(), 17);
        data.resize((DATA_CHUNK_SIZE * 4) - 5, 91);
        let written = container.write(file_inode, 0, &data).unwrap();
        assert_eq!(written, data.len() as u64);

        let to_test = vec![
            (0, 10),
            (10, DATA_CHUNK_SIZE),
            (DATA_CHUNK_SIZE - 5, 10),
            (DATA_CHUNK_SIZE - 5, DATA_CHUNK_SIZE * 10),
        ];

        for (offset, size) in to_test {
            eprintln!(
                "Read Section offset={offset}, size={size} (data size {})",
                data.len()
            );
            let size = size as u64;
            let mut read_data = Vec::new();
            let read = container
                .read(file_inode, offset as i64, size, &mut read_data)
                .unwrap();

            let expected_read = if offset as u64 + size > data.len() as u64 {
                data.len() as u64 - offset as u64
            } else {
                size
            };
            assert_eq!(read, expected_read);
            let src_slice = &data[offset as usize..(offset as u64 + read) as usize];
            let read_slice = &read_data[0..read as usize];
            assert_eq!(src_slice, read_slice);
        }
        remove_file(container_name).unwrap();
    }
}
