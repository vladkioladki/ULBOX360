use std::ffi::OsStr;
use std::io::{Read, Seek, Write};
use std::fs;
use std::fs::File;
use std::path::{PathBuf, Path};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use chrono::NaiveDate;
use bimap::BiMap;
use anyhow::{anyhow, Context};
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use libc::ENOENT;

use crate::fatx::{self, DirectoryEntry, FatxFs, FatxFsConfig, FatxFsHandle};

type Inode = u64;

struct InodeTracker {
    bimap: Mutex<BiMap<Inode, String>>,
    next_inode: Mutex<Inode>,
}

impl InodeTracker {
    fn new() -> Self {
        let mut bimap = BiMap::new();
        bimap.insert(1, String::from("/")); // root
        Self {
            bimap: Mutex::new(bimap),
            next_inode: Mutex::new(2),
        }
    }

    fn get_or_create_inode(&self, path: &str) -> Inode {
        let mut bimap = self.bimap.lock().unwrap();
        let path_string = path.to_string();
        if let Some(inode) = bimap.get_by_right(&path_string) {
            return *inode;
        }

        let mut inode_counter = self.next_inode.lock().unwrap();
        let inode = *inode_counter;
        *inode_counter += 1;

        bimap.insert(inode, path.to_string());
        inode
    }

    fn get_path(&self, inode: Inode) -> Option<String> {
        let bimap = self.bimap.lock().unwrap();
        bimap.get_by_left(&inode).cloned()
    }
}

struct FuseFatxFs {
    fatx: FatxFsHandle,
    inodes: InodeTracker,
}

fn fatx_datetime_to_systemtime(datetime: fatx::DateTime) -> SystemTime {
    let year = datetime.year() as i32;
    let month = datetime.month() as u32;
    let day = datetime.day() as u32;
    let hour = datetime.hour() as u32;
    let minute = datetime.minute() as u32;
    let second = datetime.second() as u32;

    if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
        if let Some(dt) = date.and_hms_opt(hour, minute, second) {
            return SystemTime::from(dt.and_utc());
        }
    }

    // Default epoch fallback
    SystemTime::from(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc())
}

impl FuseFatxFs {
    fn dirent_to_attr(&self, inode: u64, dirent: &DirectoryEntry) -> Option<FileAttr> {
        if dirent.is_directory() {
            return Some(FileAttr {
                ino: inode,
                size: 0,
                blocks: 0,
                atime: fatx_datetime_to_systemtime(dirent.accessed()),
                mtime: fatx_datetime_to_systemtime(dirent.modified()),
                ctime: fatx_datetime_to_systemtime(dirent.modified()),
                crtime: fatx_datetime_to_systemtime(dirent.created()),
                kind: FileType::Directory,
                perm: 0o755,
                nlink: 2,
                uid: unsafe { libc::getuid() },
                gid: unsafe { libc::getgid() },
                rdev: 0,
                flags: 0,
                blksize: 4096,
            });
        }

        if dirent.is_file() {
            return Some(FileAttr {
                ino: inode,
                size: dirent.file_size() as u64,
                blocks: (dirent.file_size() as u64 + 511) / 512,
                atime: fatx_datetime_to_systemtime(dirent.accessed()),
                mtime: fatx_datetime_to_systemtime(dirent.modified()),
                ctime: fatx_datetime_to_systemtime(dirent.modified()),
                crtime: fatx_datetime_to_systemtime(dirent.created()),
                kind: FileType::RegularFile,
                perm: 0o644,
                nlink: 1,
                uid: unsafe { libc::getuid() },
                gid: unsafe { libc::getgid() },
                rdev: 0,
                flags: 0,
                blksize: 4096,
            });
        }

        None
    }
}

const TTL: Duration = Duration::from_secs(1);

impl Filesystem for FuseFatxFs {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if let Some(root) = self.inodes.get_path(parent) {
            let mut path = PathBuf::from(root);
            path.push(name.to_str().unwrap());
            let path_str = path.to_str().unwrap();

            if let Ok(dirent) = self.fatx.stat(path_str) {
                let inode = self.inodes.get_or_create_inode(path_str);
                if let Some(attr) = self.dirent_to_attr(inode, &dirent) {
                    reply.entry(&TTL, &attr, 0);
                    return;
                }
            }
        }
        reply.error(ENOENT);
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if let Some(path) = self.inodes.get_path(ino) {
            if let Ok(dirent) = self.fatx.stat(&path) {
                if let Some(attr) = self.dirent_to_attr(ino, &dirent) {
                    reply.attr(&TTL, &attr);
                    return;
                }
            }
        }
        reply.error(ENOENT);
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        if let Some(path) = self.inodes.get_path(ino) {
            if let Ok(mut file) = self.fatx.open(&path) {
                let file_size = file.file_size();
                let read_offset = offset as u64;
                if read_offset >= file_size as u64 {
                    reply.data(&[]);
                    return;
                }
                
                let limit = std::cmp::min(size as u64, file_size as u64 - read_offset);
                let mut data = vec![0u8; limit as usize];
                
                if file.seek(std::io::SeekFrom::Start(read_offset)).is_ok() {
                    let mut total_read = 0;
                    while total_read < data.len() {
                        match file.read(&mut data[total_read..]) {
                            Ok(0) => break,
                            Ok(n) => total_read += n,
                            Err(_) => {
                                reply.error(libc::EIO);
                                return;
                            }
                        }
                    }
                    reply.data(&data[..total_read]);
                    return;
                }
            }
        }
        reply.error(ENOENT);
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        if let Some(dir_path_str) = self.inodes.get_path(ino) {
            if let Ok(dir_iter) = self.fatx.read_dir(&dir_path_str) {
                let dir_path = PathBuf::from(dir_path_str);
                let mut entries = vec![(ino, FileType::Directory, String::from("."))];

                if ino == 1 {
                    entries.push((1, FileType::Directory, String::from("..")));
                } else {
                    let parent_path = dir_path.parent().unwrap_or(&dir_path);
                    let parent_path_str = parent_path.to_str().unwrap();
                    let parent_inode = self.inodes.get_or_create_inode(parent_path_str);
                    entries.push((parent_inode, FileType::Directory, String::from("..")));
                }

                for dirent in dir_iter.flatten() {
                    if dirent.is_file() || dirent.is_directory() {
                        let mut child_path = dir_path.clone();
                        child_path.push(dirent.file_name());
                        let child_path_str = child_path.to_str().unwrap();

                        let child_inode = self.inodes.get_or_create_inode(child_path_str);
                        let ftype = if dirent.is_file() {
                            FileType::RegularFile
                        } else {
                            FileType::Directory
                        };

                        entries.push((child_inode, ftype, dirent.file_name()));
                    }
                }

                for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
                    if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                        break;
                    }
                }
                reply.ok();
                return;
            }
        }
        reply.error(ENOENT);
    }
}

pub fn mount_fatx(
    device_path: String,
    mount_point: String,
    offset: u64,
    size: u64,
) -> Result<fuser::BackgroundSession, anyhow::Error> {
    let mut config = FatxFsConfig::new(device_path);
    config.partition_offset_bytes = offset;
    config.partition_size_bytes = size;

    let fatx_handle = FatxFs::open_device(&config)
        .map_err(|e| anyhow!("Failed to open FATX volume: {:?}", e))?;

    let fs = FuseFatxFs {
        fatx: fatx_handle,
        inodes: InodeTracker::new(),
    };

    let options = vec![
        MountOption::RO,
        MountOption::FSName("fatx".to_string()),
        MountOption::AutoUnmount,
    ];

    let session = fuser::spawn_mount2(fs, mount_point, &options)
        .context("Failed to spawn FUSE background session")?;

    Ok(session)
}

pub fn extract_fatx_directory_recursive(
    device_path: &str,
    offset: u64,
    size: u64,
    remote_path: &str,
    local_dest: &Path,
    progress_cb: impl Fn(f32, &str),
) -> Result<(), anyhow::Error> {
    let mut config = FatxFsConfig::new(device_path.to_string());
    config.partition_offset_bytes = offset;
    config.partition_size_bytes = size;

    let mut fatx_handle = FatxFs::open_device(&config)
        .map_err(|e| anyhow!("Failed to open FATX volume: {:?}", e))?;

    // We will do a depth-first or breadth-first traversal
    let mut dir_queue = vec![remote_path.to_string()];
    let mut file_list = Vec::new();

    progress_cb(0.0, "Scanning FATX directories...");

    while let Some(current_dir) = dir_queue.pop() {
        let entries = fatx_handle.read_dir(&current_dir)
            .map_err(|_| anyhow!("Failed to read FATX directory: {}", current_dir))?;

        for entry in entries.flatten() {
            let name = entry.file_name();
            let full_remote_path = if current_dir.ends_with('/') {
                format!("{}{}", current_dir, name)
            } else {
                format!("{}/{}", current_dir, name)
            };

            if entry.is_directory() {
                dir_queue.push(full_remote_path);
            } else if entry.is_file() {
                file_list.push((full_remote_path, entry.file_size()));
            }
        }
    }

    let total_files = file_list.len();
    if total_files == 0 {
        progress_cb(1.0, "Done (No files found to extract).");
        return Ok(());
    }

    for (i, (remote_file_path, file_size)) in file_list.iter().enumerate() {
        let relative_path = remote_file_path.trim_start_matches('/');
        let local_file_path = local_dest.join(relative_path);

        if let Some(parent) = local_file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        progress_cb(
            i as f32 / total_files as f32,
            &format!("Extracting: {} ({} bytes)", relative_path, file_size),
        );

        let mut fatx_file = fatx_handle.open(remote_file_path)
            .map_err(|_| anyhow!("Failed to open file in FATX: {}", remote_file_path))?;

        let mut local_file = File::create(&local_file_path)?;
        let mut buffer = vec![0u8; 64 * 1024];

        loop {
            match fatx_file.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    local_file.write_all(&buffer[..n])?;
                }
                Err(e) => return Err(anyhow!("Read error on FATX: {:?}", e)),
            }
        }
    }

    progress_cb(1.0, "Extraction completed successfully.");
    Ok(())
}
