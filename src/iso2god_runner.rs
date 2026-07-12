use std::fs::{self, File};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use anyhow::{Context, Error};
use rayon::prelude::*;

use iso2god::executable::TitleInfo;
use iso2god::{game_list, god, iso};

pub fn convert_iso_to_god(
    source_iso: PathBuf,
    dest_dir: PathBuf,
    trim: bool,
    num_threads: usize,
    progress_cb: Arc<dyn Fn(f32, &str) + Send + Sync + 'static>,
) -> Result<(), Error> {
    progress_cb(0.0, "Initializing thread pool...");
    
    // Create thread pool for this specific conversion
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .context("Failed to build Rayon thread pool")?;

    progress_cb(0.01, "Extracting ISO metadata...");

    let source_iso_file = File::open(&source_iso).context("Error opening source ISO file")?;
    let source_iso_file_meta = fs::metadata(&source_iso).context("Error reading source ISO file metadata")?;

    let mut source_iso_reader = iso::IsoReader::read(source_iso_file).context("Error reading source ISO")?;
    let title_info = TitleInfo::from_image(&mut source_iso_reader).context("Error reading image executable")?;

    let exe_info = title_info.execution_info;
    let content_type = title_info.content_type;

    let title_id = format!("{:08X}", exe_info.title_id);
    let game_title = game_list::find_title_by_id(exe_info.title_id)
        .unwrap_or_else(|| "Unknown Game".to_string());
    
    let info_msg = format!("Title: {} (ID: {})", game_title, title_id);
    progress_cb(0.02, &info_msg);

    let data_size = if trim {
        source_iso_reader.get_max_used_prefix_size()
    } else {
        let root_offset = source_iso_reader.volume_descriptor.root_offset;
        source_iso_file_meta.len() - root_offset
    };

    let block_count = data_size.div_ceil(god::BLOCK_SIZE);
    let part_count = block_count.div_ceil(god::BLOCKS_PER_PART);

    let file_layout = god::FileLayout::new(&dest_dir, &exe_info, content_type);

    progress_cb(0.03, "Clearing data directory...");
    ensure_empty_dir(&file_layout.data_dir_path()).context("Error clearing data directory")?;

    let progress = Arc::new(AtomicUsize::new(0));

    progress_cb(0.04, &format!("Writing part files: 0/{}", part_count));

    let progress_cb_clone = progress_cb.clone();
    let source_iso_clone = source_iso.clone();
    let dest_dir_clone = dest_dir.clone();
    let progress_counter = progress.clone();
    let root_offset = source_iso_reader.volume_descriptor.root_offset;
    let exe_info_clone = exe_info.clone();
    
    pool.install(move || {
        (0..part_count as usize).into_par_iter().try_for_each(|part_index| {
            let mut iso_data_volume = File::open(&source_iso_clone)?;
            iso_data_volume.seek(SeekFrom::Start(root_offset))?;

            let file_layout = god::FileLayout::new(&dest_dir_clone, &exe_info_clone, content_type);
            let part_file = file_layout.part_file_path(part_index as u64);
            let part_file = File::options()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&part_file)
                .context("Error creating part file")?;

            god::write_part(iso_data_volume, part_index as u64, part_file)
                .context("Error writing part file")?;

            let cur = 1 + progress_counter.fetch_add(1, Ordering::Relaxed);
            
            // Map progress to 0.05 - 0.90 range
            let ratio = cur as f32 / part_count as f32;
            let current_progress = 0.05 + ratio * 0.85;
            let msg = format!("Writing part files: {}/{}", cur, part_count);
            progress_cb_clone(current_progress, &msg);

            Ok::<_, anyhow::Error>(())
        })
    })?;

    progress_cb(0.91, "Calculating MHT hash chain...");
    let mut mht = read_part_mht(&file_layout, part_count - 1).context("Error reading part file MHT")?;

    for prev_part_index in (0..part_count - 1).rev() {
        let mut prev_mht = read_part_mht(&file_layout, prev_part_index).context("Error reading part file MHT")?;
        prev_mht.add_hash(&mht.digest());
        write_part_mht(&file_layout, prev_part_index, &prev_mht).context("Error writing part file MHT")?;
        mht = prev_mht;
    }

    let last_part_size = fs::metadata(file_layout.part_file_path(part_count - 1))
        .map(|m| m.len())
        .context("Error reading part file")?;

    progress_cb(0.95, "Writing con header...");
    let con_header = god::ConHeaderBuilder::new()
        .with_execution_info(&exe_info)
        .with_block_counts(block_count as u32, 0)
        .with_data_parts_info(
            part_count as u32,
            last_part_size + (part_count - 1) * god::BLOCK_SIZE * 0xa290,
        )
        .with_content_type(content_type)
        .with_mht_hash(&mht.digest())
        .with_game_title(&game_title);

    let con_header = con_header.finalize();
    let mut con_header_file = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(file_layout.con_header_file_path())
        .context("Cannot open con header file")?;

    con_header_file.write_all(&con_header).context("Error writing con header file")?;

    progress_cb(1.0, &format!("Done! Game converted: {}", game_title));

    Ok(())
}

fn ensure_empty_dir(path: &Path) -> Result<(), Error> {
    if fs::metadata(path).is_ok() {
        fs::remove_dir_all(path)?;
    };
    fs::create_dir_all(path)?;
    Ok(())
}

fn read_part_mht(file_layout: &god::FileLayout, part_index: u64) -> Result<god::HashList, Error> {
    let part_file = file_layout.part_file_path(part_index);
    let mut part_file = File::options().read(true).open(part_file)?;
    god::HashList::read(&mut part_file)
}

fn write_part_mht(
    file_layout: &god::FileLayout,
    part_index: u64,
    mht: &god::HashList,
) -> Result<(), Error> {
    let part_file = file_layout.part_file_path(part_index);
    let mut part_file = File::options().write(true).open(part_file)?;
    mht.write(&mut part_file)?;
    Ok(())
}
