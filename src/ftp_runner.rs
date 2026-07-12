use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::{anyhow, Context};
use ftp::FtpStream;

pub fn upload_directory_to_ftp(
    ip: &str,
    port: u16,
    user: &str,
    pass: &str,
    local_dir: PathBuf,
    remote_base_dir: &str,
    progress_cb: Arc<dyn Fn(f32, &str) + Send + Sync + 'static>,
) -> Result<(), anyhow::Error> {
    progress_cb(0.0, &format!("Connecting to Xbox FTP {}:{}...", ip, port));
    let mut ftp = FtpStream::connect((ip, port))
        .map_err(|e| anyhow!("Connection failed: {:?}", e))?;

    progress_cb(0.05, "Logging in...");
    ftp.login(user, pass)
        .map_err(|e| anyhow!("Login failed: {:?}", e))?;

    progress_cb(0.10, "Scanning local files to upload...");
    let mut file_list = Vec::new();
    collect_files_recursive(&local_dir, &mut file_list)?;

    let total_files = file_list.len();
    if total_files == 0 {
        progress_cb(1.0, "Done (No files found to upload).");
        return Ok(());
    }

    progress_cb(0.15, &format!("Uploading {} files...", total_files));

    // Ensure remote base directory exists or create it
    create_remote_dir_recursive(&mut ftp, remote_base_dir)?;

    for (i, local_file_path) in file_list.iter().enumerate() {
        let relative_path = local_file_path.strip_prefix(&local_dir)?;
        let relative_path_str = relative_path.to_string_lossy().replace('\\', "/");
        
        let remote_file_path = if remote_base_dir.ends_with('/') {
            format!("{}{}", remote_base_dir, relative_path_str)
        } else {
            format!("{}/{}", remote_base_dir, relative_path_str)
        };

        // Ensure remote parent directories exist
        if let Some(parent) = Path::new(&remote_file_path).parent() {
            let parent_str = parent.to_string_lossy();
            if !parent_str.is_empty() {
                create_remote_dir_recursive(&mut ftp, &parent_str)?;
            }
        }

        let file_name = local_file_path.file_name().unwrap().to_string_lossy();
        progress_cb(
            0.15 + (i as f32 / total_files as f32) * 0.80,
            &format!("Uploading: {} ({}/{})", file_name, i + 1, total_files),
        );

        let mut file = File::open(local_file_path)
            .context("Failed to open local file for upload")?;
        
        // Go to binary mode for game files
        ftp.transfer_type(ftp::types::FileType::Image)
            .map_err(|e| anyhow!("Failed to set binary mode: {:?}", e))?;

        ftp.put(&remote_file_path, &mut file)
            .map_err(|e| anyhow!("Upload failed for file {}: {:?}", relative_path_str, e))?;
    }

    ftp.quit().ok();
    progress_cb(1.0, "Done! Files successfully uploaded to console.");
    Ok(())
}

fn collect_files_recursive(dir: &Path, list: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_files_recursive(&path, list)?;
            } else {
                list.push(path);
            }
        }
    }
    Ok(())
}

fn create_remote_dir_recursive(ftp: &mut FtpStream, path: &str) -> Result<(), anyhow::Error> {
    let clean_path = path.trim_start_matches('/');
    if clean_path.is_empty() {
        return Ok(());
    }

    let mut current_path = String::new();
    for component in clean_path.split('/') {
        if component.is_empty() {
            continue;
        }
        current_path = if current_path.is_empty() {
            component.to_string()
        } else {
            format!("{}/{}", current_path, component)
        };

        // Try creating directory, ignore error if it already exists
        let _ = ftp.mkdir(&current_path);
    }
    Ok(())
}
