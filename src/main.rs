pub mod fatx;
mod iso2god_runner;
mod xiso_runner;
mod fatx_runner;
mod ftp_runner;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use slint::ComponentHandle;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    // Shared FUSE Mount Session state
    let mount_session: Arc<Mutex<Option<fuser::BackgroundSession>>> = Arc::new(Mutex::new(None));

    // Create a Tokio runtime for async tasks (XISO packing/unpacking)
    let tokio_runtime = tokio::runtime::Runtime::new()
        .expect("Failed to initialize Tokio runtime");

    // ==========================================
    // ISO to GOD Tab
    // ==========================================
    // Shared state queues for batch converting
    let god_iso_queue: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
    let xiso_source_queue: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(Vec::new()));

    // ==========================================
    // ISO to GOD Tab
    // ==========================================
    let ui_weak = ui.as_weak();
    let god_queue_clone = god_iso_queue.clone();
    ui.on_select_god_iso(move || {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Xbox 360 ISO", &["iso"])
            .pick_files()
        {
            let mut queue = god_queue_clone.lock().unwrap();
            *queue = paths;
            if let Some(ui) = ui_weak.upgrade() {
                if queue.len() == 1 {
                    ui.set_god_iso_path(queue[0].to_string_lossy().to_string().into());
                } else {
                    ui.set_god_iso_path(format!("{} ISO files selected (Batch Mode)", queue.len()).into());
                }
            }
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_select_god_dest(move || {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_god_dest_path(path.to_string_lossy().to_string().into());
            }
        }
    });

    let ui_weak = ui.as_weak();
    let god_queue_clone = god_iso_queue.clone();
    ui.on_convert_to_god(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let dest_dir = PathBuf::from(ui.get_god_dest_path().as_str());
            let trim = ui.get_god_trim();
            let threads_str = ui.get_god_threads().to_string();
            let num_threads: usize = threads_str.parse().unwrap_or(4);

            let mut files = god_queue_clone.lock().unwrap().clone();
            if files.is_empty() {
                let direct_path = PathBuf::from(ui.get_god_iso_path().as_str());
                if !direct_path.to_string_lossy().is_empty() && direct_path.exists() {
                    files.push(direct_path);
                }
            }

            if files.is_empty() || dest_dir.to_string_lossy().is_empty() {
                ui.set_god_status("Error: Please select source ISO and destination path.".into());
                return;
            }

            ui.set_god_progress(0.0);
            ui.set_god_status("Initializing Batch...".into());

            let ui_thread_weak = ui_weak.clone();
            std::thread::spawn(move || {
                let total_files = files.len();
                for (idx, source_iso) in files.into_iter().enumerate() {
                    let file_name = source_iso.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let ui_cb_weak = ui_thread_weak.clone();
                    
                    let progress_cb = Arc::new(move |file_progress: f32, msg: &str| {
                        let overall_progress = (idx as f32 + file_progress) / total_files as f32;
                        let status_msg = format!("[{}/{}] {}: {}", idx + 1, total_files, file_name, msg);
                        let _ = ui_cb_weak.upgrade_in_event_loop(move |ui| {
                            ui.set_god_progress(overall_progress);
                            ui.set_god_status(status_msg.into());
                        });
                    });

                    progress_cb(0.0, "Starting...");
                    match iso2god_runner::convert_iso_to_god(source_iso, dest_dir.clone(), trim, num_threads, progress_cb.clone()) {
                        Ok(_) => {
                            progress_cb(1.0, "Done.");
                        }
                        Err(e) => {
                            progress_cb(0.0, &format!("Failed: {:?}", e));
                            std::thread::sleep(std::time::Duration::from_secs(3));
                        }
                    }
                }
                
                let _ = ui_thread_weak.upgrade_in_event_loop(move |ui| {
                    ui.set_god_progress(1.0);
                    ui.set_god_status("Batch conversion completed successfully!".into());
                });
            });
        }
    });

    // ==========================================
    // XISO Packer/Unpacker Tab
    // ==========================================
    let ui_weak = ui.as_weak();
    let xiso_queue_clone = xiso_source_queue.clone();
    ui.on_select_xiso_source(move |is_file| {
        if is_file {
            if let Some(paths) = rfd::FileDialog::new()
                .add_filter("Xbox ISO", &["iso", "xiso"])
                .pick_files()
            {
                let mut queue = xiso_queue_clone.lock().unwrap();
                *queue = paths;
                if let Some(ui) = ui_weak.upgrade() {
                    if queue.len() == 1 {
                        ui.set_xiso_source_path(queue[0].to_string_lossy().to_string().into());
                    } else {
                        ui.set_xiso_source_path(format!("{} XISO files selected (Batch Mode)", queue.len()).into());
                    }
                }
            }
        } else {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                let mut queue = xiso_queue_clone.lock().unwrap();
                *queue = vec![path.clone()];
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_xiso_source_path(path.to_string_lossy().to_string().into());
                }
            }
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_select_xiso_dest(move |is_file| {
        let path = if is_file {
            rfd::FileDialog::new()
                .add_filter("Xbox ISO", &["iso", "xiso"])
                .save_file()
        } else {
            rfd::FileDialog::new().pick_folder()
        };

        if let Some(p) = path {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_xiso_dest_path(p.to_string_lossy().to_string().into());
            }
        }
    });

    let ui_weak = ui.as_weak();
    let xiso_queue_clone = xiso_source_queue.clone();
    let tokio_handle = tokio_runtime.handle().clone();
    ui.on_unpack_xiso(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let dest_dir = PathBuf::from(ui.get_xiso_dest_path().as_str());

            let mut files = xiso_queue_clone.lock().unwrap().clone();
            if files.is_empty() {
                let direct_path = PathBuf::from(ui.get_xiso_source_path().as_str());
                if !direct_path.to_string_lossy().is_empty() && direct_path.exists() {
                    files.push(direct_path);
                }
            }

            if files.is_empty() || dest_dir.to_string_lossy().is_empty() {
                ui.set_xiso_status("Error: Select source XISO files and destination folder.".into());
                return;
            }

            ui.set_xiso_progress(0.0);
            ui.set_xiso_status("Initializing Batch Unpack...".into());

            let ui_thread_weak = ui_weak.clone();
            tokio_handle.spawn(async move {
                let total_files = files.len();
                for (idx, source_iso) in files.into_iter().enumerate() {
                    let file_stem = source_iso.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    let current_dest_dir = if total_files > 1 {
                        dest_dir.join(&file_stem)
                    } else {
                        dest_dir.clone()
                    };

                    let ui_cb_weak = ui_thread_weak.clone();
                    let stem_clone = file_stem.clone();
                    let progress_cb = Arc::new(move |file_progress: f32, msg: &str| {
                        let overall_progress = (idx as f32 + file_progress) / total_files as f32;
                        let status_msg = format!("[{}/{}] {}: {}", idx + 1, total_files, stem_clone, msg);
                        let _ = ui_cb_weak.upgrade_in_event_loop(move |ui| {
                            ui.set_xiso_progress(overall_progress);
                            ui.set_xiso_status(status_msg.into());
                        });
                    });

                    progress_cb(0.0, "Unpacking...");
                    match xiso_runner::unpack_xiso(source_iso, current_dest_dir, progress_cb.clone()).await {
                        Ok(_) => {
                            progress_cb(1.0, "Done.");
                        }
                        Err(e) => {
                            progress_cb(0.0, &format!("Failed: {}", e));
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        }
                    }
                }

                let _ = ui_thread_weak.upgrade_in_event_loop(move |ui| {
                    ui.set_xiso_progress(1.0);
                    ui.set_xiso_status("Batch unpack completed successfully!".into());
                });
            });
        }
    });

    let ui_weak = ui.as_weak();
    let tokio_handle = tokio_runtime.handle().clone();
    ui.on_pack_xiso(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let source_dir = PathBuf::from(ui.get_xiso_source_path().as_str());
            let dest_iso = PathBuf::from(ui.get_xiso_dest_path().as_str());

            if source_dir.to_string_lossy().is_empty() || dest_iso.to_string_lossy().is_empty() {
                ui.set_xiso_status("Error: Select source folder and destination ISO file.".into());
                return;
            }

            ui.set_xiso_progress(0.0);
            ui.set_xiso_status("Packing folder to XISO...".into());

            let ui_thread_weak = ui_weak.clone();
            tokio_handle.spawn(async move {
                let progress_cb = Arc::new(move |progress: f32, msg: &str| {
                    let msg_str = msg.to_string();
                    let _ = ui_thread_weak.upgrade_in_event_loop(move |ui| {
                        ui.set_xiso_progress(progress);
                        ui.set_xiso_status(msg_str.into());
                    });
                });

                match xiso_runner::pack_xiso(source_dir, dest_iso, progress_cb.clone()).await {
                    Ok(_) => {}
                    Err(e) => {
                        progress_cb(0.0, &format!("Error: {}", e));
                    }
                }
            });
        }
    });

    // ==========================================
    // FATX Mounter Tab
    // ==========================================
    let ui_weak = ui.as_weak();
    ui.on_select_fatx_img(move || {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Xbox Disk Image/Device", &["img", "bin", "raw"])
            .pick_file()
        {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_fatx_img_path(path.to_string_lossy().to_string().into());
            }
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_select_fatx_mount(move || {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_fatx_mount_path(path.to_string_lossy().to_string().into());
            }
        }
    });

    let ui_weak = ui.as_weak();
    let mount_session_clone = mount_session.clone();
    ui.on_mount_fatx(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let img_path = ui.get_fatx_img_path().as_str().to_string();
            let mount_path = ui.get_fatx_mount_path().as_str().to_string();

            if img_path.is_empty() || mount_path.is_empty() {
                ui.set_fatx_status("Error: Select image file and mount point.".into());
                return;
            }

            ui.set_fatx_status("Mounting FUSE filesystem...".into());

            // Default to Xbox 360 Partition 3 (Data) offset: 0x130EB0000 (5,115,674,624 bytes)
            let offset = 0x130EB0000; 
            let size = 2_000_000_000_000; // Auto-detected inside fs.rs

            let mount_session_thread = mount_session_clone.clone();
            let ui_thread_weak = ui_weak.clone();

            std::thread::spawn(move || {
                match fatx_runner::mount_fatx(img_path, mount_path, offset, size) {
                    Ok(session) => {
                        let mut guard = mount_session_thread.lock().unwrap();
                        *guard = Some(session);
                        
                        let _ = ui_thread_weak.upgrade_in_event_loop(move |ui| {
                            ui.set_fatx_is_mounted(true);
                            ui.set_fatx_status("Mounted! Open file manager to browse.".into());
                        });
                    }
                    Err(e) => {
                        let _ = ui_thread_weak.upgrade_in_event_loop(move |ui| {
                            ui.set_fatx_status(format!("Mount failed: {:?}", e).into());
                        });
                    }
                }
            });
        }
    });

    let ui_weak = ui.as_weak();
    let mount_session_clone = mount_session.clone();
    ui.on_unmount_fatx(move || {
        let mut guard = mount_session_clone.lock().unwrap();
        if guard.is_some() {
            // Dropping the BackgroundSession will trigger an unmount
            *guard = None;
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_fatx_is_mounted(false);
                ui.set_fatx_status("Unmounted successfully.".into());
            }
        }
    });

    // ==========================================
    // Aurora FTP Upload Tab
    // ==========================================
    let ui_weak = ui.as_weak();
    ui.on_select_ftp_local(move || {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_ftp_local_path(path.to_string_lossy().to_string().into());
            }
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_start_ftp_upload(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let ip = ui.get_ftp_ip().as_str().to_string();
            let port_str = ui.get_ftp_port().as_str().to_string();
            let port: u16 = port_str.parse().unwrap_or(21);
            let user = ui.get_ftp_user().as_str().to_string();
            let pass = ui.get_ftp_pass().as_str().to_string();
            let local_path = PathBuf::from(ui.get_ftp_local_path().as_str());

            if ip.is_empty() || local_path.to_string_lossy().is_empty() {
                ui.set_ftp_status("Error: Input console IP and select local folder.".into());
                return;
            }

            ui.set_ftp_progress(0.0);
            ui.set_ftp_status("Uploading...".into());

            // Default remote base directory for Aurora scan path (usually on Hdd1)
            let remote_dir = "/Hdd1/Games/";

            let ui_thread_weak = ui_weak.clone();
            std::thread::spawn(move || {
                let progress_cb = Arc::new(move |progress: f32, msg: &str| {
                    let msg_str = msg.to_string();
                    let _ = ui_thread_weak.upgrade_in_event_loop(move |ui| {
                        ui.set_ftp_progress(progress);
                        ui.set_ftp_status(msg_str.into());
                    });
                });

                match ftp_runner::upload_directory_to_ftp(&ip, port, &user, &pass, local_path, remote_dir, progress_cb.clone()) {
                    Ok(_) => {}
                    Err(e) => {
                        progress_cb(0.0, &format!("Upload failed: {:?}", e));
                    }
                }
            });
        }
    });

    ui.run()
}
