use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use anyhow::{anyhow, Context};

pub async fn unpack_xiso(
    iso_path: PathBuf,
    dest_dir: PathBuf,
    progress_cb: Arc<dyn Fn(f32, &str) + Send + Sync + 'static>,
) -> Result<(), anyhow::Error> {
    progress_cb(0.0, "Opening XISO image...");
    let file = File::open(&iso_path)
        .context("Failed to open XISO file")?;

    progress_cb(0.05, "Scanning XISO volume structure...");
    let mut dev = xdvdfs::blockdev::OffsetWrapper::new(file).await
        .map_err(|_| anyhow!("Invalid or unsupported XISO image format (offsets mismatch)"))?;

    let volume = xdvdfs::read::read_volume(&mut dev).await
        .map_err(|_| anyhow!("Failed to parse XDVDFS volume descriptor"))?;

    progress_cb(0.10, "Traversing file tree...");
    let tree = volume.root_table.file_tree(&mut dev).await
        .map_err(|_| anyhow!("Failed to walk XDVDFS directory tree"))?;

    let total_entries = tree.len();
    if total_entries == 0 {
        progress_cb(1.0, "Done (ISO is empty).");
        return Ok(());
    }

    progress_cb(0.15, &format!("Extracting {} entries...", total_entries));

    for (i, (dir, node)) in tree.iter().enumerate() {
        let name_str = node.name_str::<io::Error>()?;
        let clean_dir = dir.trim_start_matches('/');
        let target_dir = if clean_dir.is_empty() {
            dest_dir.clone()
        } else {
            dest_dir.join(clean_dir)
        };

        fs::create_dir_all(&target_dir)
            .context("Failed to create target directory")?;

        let target_path = target_dir.join(&*name_str);

        if node.node.dirent.attributes.directory() {
            fs::create_dir_all(&target_path)
                .context("Failed to create sub-directory")?;
        } else {
            let mut dest_file = File::create(&target_path)
                .context("Failed to create output file")?;
            let size = node.node.dirent.data.size() as u64;
            
            if size > 0 {
                use xdvdfs::blockdev::BlockDeviceRead;
                let file_offset = node.node.dirent.data.offset::<std::io::Error>(0)
                    .map_err(|_| anyhow!("Failed to get file sector offset"))?;
                
                let mut bytes_read = 0;
                let mut buffer = [0u8; 64 * 1024]; // 64KB buffer
                
                while bytes_read < size {
                    let chunk_size = std::cmp::min(buffer.len() as u64, size - bytes_read) as usize;
                    dev.read(file_offset + bytes_read, &mut buffer[..chunk_size]).await
                        .map_err(|e| anyhow!("Failed to read file chunk: {:?}", e))?;
                    dest_file.write_all(&buffer[..chunk_size])
                        .context("Failed to write chunk to output file")?;
                    bytes_read += chunk_size as u64;
                }
            }
        }

        let ratio = (i + 1) as f32 / total_entries as f32;
        let current_progress = 0.15 + ratio * 0.80;
        progress_cb(current_progress, &format!("Extracted: {}/{} ({})", i + 1, total_entries, name_str));
    }

    progress_cb(1.0, "Done! XISO successfully unpacked.");
    Ok(())
}

pub async fn pack_xiso(
    source_dir: PathBuf,
    dest_iso_path: PathBuf,
    progress_cb: Arc<dyn Fn(f32, &str) + Send + Sync + 'static>,
) -> Result<(), anyhow::Error> {
    progress_cb(0.0, "Scanning files to pack...");
    
    let mut fs_impl = xdvdfs::write::fs::StdFilesystem::create(&source_dir);
    let image_file = File::create(&dest_iso_path)
        .context("Failed to create output ISO file")?;
    let mut image_buf = io::BufWriter::with_capacity(4 * 1024 * 1024, image_file);

    progress_cb(0.10, "Creating XDVDFS layout...");

    let progress_cb_inner = progress_cb.clone();
    let res = xdvdfs::write::img::create_xdvdfs_image(
        &mut fs_impl,
        &mut image_buf,
        move |info| {
            use xdvdfs::write::img::ProgressInfo;
            match info {
                ProgressInfo::DiscoveredDirectory(len) => {
                    progress_cb_inner(0.20, &format!("Discovered directories (listing size: {})", len));
                }
                ProgressInfo::FileCount(count) => {
                    progress_cb_inner(0.30, &format!("Processing {} files...", count));
                }
                ProgressInfo::DirCount(count) => {
                    progress_cb_inner(0.35, &format!("Allocating directory structures (count: {})...", count));
                }
                ProgressInfo::DirAdded(name, sector) => {
                    progress_cb_inner(0.50, &format!("Added Directory: {} (Sector: {})", name, sector));
                }
                ProgressInfo::FileAdded(name, sector) => {
                    progress_cb_inner(0.70, &format!("Added File: {} (Sector: {})", name, sector));
                }
                ProgressInfo::FinishedPacking => {
                    progress_cb_inner(0.95, "Finishing volume descriptor...");
                }
                _ => {}
            }
        }
    ).await;

    match res {
        Ok(_) => {
            progress_cb(1.0, "Done! XISO image successfully built.");
            Ok(())
        }
        Err(e) => {
            Err(anyhow!("Failed to build XISO image: {:?}", e))
        }
    }
}
