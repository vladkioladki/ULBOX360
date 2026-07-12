#!/bin/bash
set -e

echo "=== Step 1: Compiling ULBOX360 in Release Mode ==="
cargo build --release

echo "=== Step 2: Preparing AppDir Directory ==="
rm -rf AppDir
mkdir -p AppDir/usr/bin

# Copy target binary
cp target/release/ulbox360 AppDir/usr/bin/

# Copy desktop file and icon
cp ulbox360.desktop AppDir/
cp ulbox360.svg AppDir/

# Create AppRun entry point
cat << 'EOF' > AppDir/AppRun
#!/bin/sh
SELF=$(readlink -f "$0")
HERE=$(dirname "$SELF")
export PATH="${HERE}/usr/bin:${PATH}"

# Execute application passing all args
exec ulbox360 "$@"
EOF
chmod +x AppDir/AppRun

echo "=== Step 3: Fetching appimagetool ==="
if [ ! -f appimagetool ] || grep -q "Not Found" appimagetool; then
    echo "Downloading appimagetool..."
    rm -f appimagetool
    curl -L -o appimagetool https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
    chmod +x appimagetool
fi

echo "=== Step 4: Building AppImage ==="
# Using --appimage-extract-and-run to ensure compatibility in various environments without requiring root loop devices
ARCH=x86_64 ./appimagetool --appimage-extract-and-run AppDir ULBOX360-x86_64.AppImage

echo "=== Step 5: Relocating Release & Cleaning Up ==="
mkdir -p dist
mv ULBOX360-x86_64.AppImage dist/
rm -rf AppDir

echo "=== Build Complete: dist/ULBOX360-x86_64.AppImage ==="
ls -lh dist/ULBOX360-x86_64.AppImage
