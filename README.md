# ULBOX360
An All-in-One GUI utility for modded Xbox 360 and Xbox Original game backup management on Linux
Built entirely in **Rust** using the **Slint** GUI framework and linked with system **Qt6** widgets for native Breeze KDE styling and Wayland performance.
---
![AI-Generated](https://img.shields.io/badge/Code-AI--Assisted%20%3E50%25-blue?style=flat-square)

## Features

_Since I am not an experienced developer, code reviews, refactoring, and bug fixes from the community are highly encouraged and appreciated!_

### 1. ISO to GOD (Games on Demand)
* Converts raw Xbox 360 ISO backups into Games on Demand (`GOD`) containers directly recognizable by modded consoles.
* **Padding Trimming**: Optional zero-byte sector trimming to dramatically reduce size on target HDD.
* **Parallel Core Allocation**: Rayon-powered multi-threaded data slicing.

### 2. XISO Unpacker & Packer
* Pack local directories into Xbox 360/Xbox Original compatible XISO images.
* Extract files from existing XISOs directly.
* Full support for Xbox Original **XDVDFS** filesystem structures (ideal for backwards compatibility backups in `/Hdd1/Compatibility/Xbox1/`).

### 3. FATX FUSE Mounter & Direct Browser
* **Native Mount**: Mount FATX raw dump images or block storage devices directly to any mountpoint directory on Linux using FUSE.
* **Custom Sector Offsets**: Supports preset Xbox 360 partition offsets (e.g. Partition 3 Game Data at `0x130EB0000`, Partition 1 Cache, or Xbox Original E/F partitions) or user-defined custom hexadecimal offsets.
* **Safety Bound Checks**: Automatically caps reading sizes to avoid virtual boundaries errors.

### 4. Aurora FTP Sync
* Wirelessly or wired upload extracted games or GOD content directly to the console hard drive.
* Recursive directory listing, connection verification, and automated folder creations.

---

## Installation

### Fedora / RedHat
Install the required system dependencies for Slint compile hooks and FUSE 3 support:
```bash
sudo dnf install fontconfig-devel fuse3-devel
```

### Ubuntu / Debian
```bash
sudo apt-get install libfontconfig1-dev libfuse3-dev
```

---

## Usage

### Downloading the AppImage
Download the precompiled AppImage from the releases tab, make it executable, and run it:
```bash
chmod +x ULBOX360-x86_64.AppImage
./ULBOX360-x86_64.AppImage
```

### Building from Source
Ensure you have the latest Rust toolchain installed, then run:
```bash
cargo build --release
```
The compiled binary will be located in `target/release/ulbox360`.

### Packaging into AppImage
To compile and package the app yourself into a portable AppImage container, run the provided build script:
```bash
./build_appimage.sh
```

---

## Credits & Upstream Libraries

ULBOX360 is built using the following outstanding Rust libraries:
* **[Slint](https://github.com/slint-ui/slint)** - Next-generation native UI toolkit.
* **[iso2god-rs](https://github.com/iliazeus/iso2god-rs)** - Pure Rust port of the ISO2GOD Xbox 360 engine.
* **[xdvdfs](https://github.com/xenia-project/xdvdfs)** - Pure Rust implementation of the Xbox DVD Filesystem.
* **[fatx](https://github.com/mborgerson/fatx)** - Rust implementation of the FATX filesystem parser.
* **[fuser](https://github.com/cgreene/fuser)** - FUSE implementation for Rust.
* **[rfd](https://github.com/emilk/rfd)** - Native OS file dialogs wrapper.

---

## License

This project is licensed under the [MIT License](LICENSE).
