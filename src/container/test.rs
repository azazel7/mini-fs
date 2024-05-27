#[cfg(test)]
mod tests {
    use crate::container::Container;
    use crate::sector::{self, FileData, FileMetadata, Sector, DATA_CHUNK_SIZE};
    use fuser::FileType;
    use std::ffi::{OsStr, OsString};
    use std::str::FromStr;
    use std::{collections::HashSet, fs::remove_file};

    #[test]
    fn append_empty_sector() {
        let container_name = "/tmp/canard_append_empty";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();
        let sector_count = container.metadata.sector_count;
        container.append_empty_sector().unwrap();
        assert_eq!(container.metadata.sector_count, sector_count + 1);
        assert_eq!(container.metadata.last_empty_sector, Some(sector_count));
        remove_file(container_name).unwrap();
    }
    #[test]
    fn read_write_sector() {
        let container_name = "/tmp/canard_read_write_sector";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();
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
        remove_file(container_name).unwrap();
    }
    #[test]
    fn free_sector() {
        let container_name = "/tmp/canard_free_sector";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();
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

        remove_file(container_name).unwrap();
    }
    #[test]
    fn delete_file() {
        let container_name = "/tmp/canard_delete_file";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();
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

        remove_file(container_name).unwrap();
    }
    #[test]
    fn get_empty_sector() {
        let container_name = "/tmp/canard_get_empty_sector";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();
        container.append_empty_sector().unwrap();
        assert_eq!(container.metadata.sector_count, 2);
        let empty_sector = container.get_empty_sector().unwrap();
        assert_eq!(empty_sector, 1);
        remove_file(container_name).unwrap();
    }
    #[test]
    fn find_ino_sector() {
        let container_name = "/tmp/canard_find_ino_sector";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();
        let new_inode = container
            .create(1, OsStr::new("loutre.txt"), sector::FileType::Regular)
            .unwrap();
        let (sector_id, _sector) = container.find_ino_sector(1).unwrap();
        assert_eq!(sector_id, 0); //Root directory
        let ret = container.find_ino_sector(new_inode);
        assert!(ret.is_ok());
        let ret = container.find_ino_sector(37);
        assert!(ret.is_err());
        remove_file(container_name).unwrap();
    }
    #[test]
    fn getattr() {
        let container_name = "/tmp/canard_getattr";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();

        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        let new_inode = container
            .create(1, OsStr::new("loutre.txt"), sector::FileType::Regular)
            .unwrap();

        let attr = container.getattr(new_inode).unwrap().unwrap();
        assert_eq!(attr.filetype, FileType::RegularFile);
        let attr = container.getattr(1).unwrap().unwrap();
        assert_eq!(attr.filetype, FileType::Directory);
        let filetype = container.getattr(37);
        assert!(filetype.is_err());
        remove_file(container_name).unwrap();
    }
    #[test]
    fn readdir() {
        let container_name = "/tmp/canard_readdir";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();

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

        remove_file(container_name).unwrap();
    }
    #[test]
    fn lookup() {
        let container_name = "/tmp/canard_lookup";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();

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

        remove_file(container_name).unwrap();
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
    #[test]
    fn lookup_name() {
        let container_name = "/tmp/canard_lookup_name";
        let _ = remove_file(container_name);
        let mut container = Container::new(container_name.to_string()).unwrap();

        let inode1 = container
            .create(1, OsStr::new("loutre.txt"), sector::FileType::Regular)
            .unwrap();
        let inode2 = container
            .create(1, OsStr::new("canard.txt"), sector::FileType::Regular)
            .unwrap();
        let inode_dir = container
            .create(1, OsStr::new("ocean"), sector::FileType::Directory)
            .unwrap();
        let inode3 = container
            .create(
                inode_dir,
                OsStr::new("saumon.txt"),
                sector::FileType::Regular,
            )
            .unwrap();

        let name1 = container.lookup_name(inode1).unwrap();
        assert_eq!(name1, OsString::from_str("loutre.txt").unwrap());
        let name2 = container.lookup_name(inode2).unwrap();
        assert_eq!(name2, OsString::from_str("canard.txt").unwrap());
        let name3 = container.lookup_name(inode3).unwrap();
        assert_eq!(name3, OsString::from_str("saumon.txt").unwrap());
        let name_dir = container.lookup_name(inode_dir).unwrap();
        assert_eq!(name_dir, OsString::from_str("ocean").unwrap());
        remove_file(container_name).unwrap();
    }
}
