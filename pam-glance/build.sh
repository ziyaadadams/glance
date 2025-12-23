#!/bin/bash
# Build script for pam-facerec
# Builds the Rust PAM module

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}   PAM FaceRec - Build Script${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Rust/Cargo not found${NC}"
    echo "Please install Rust first: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Check for required system dependencies
echo -e "${YELLOW}Checking dependencies...${NC}"

DEPS_MISSING=0

# Check for dlib
if ! pkg-config --exists dlib 2>/dev/null; then
    if [ ! -f /usr/include/dlib/image_processing/frontal_face_detector.h ] && \
       [ ! -f /usr/local/include/dlib/image_processing/frontal_face_detector.h ]; then
        echo -e "${RED}Missing: dlib${NC}"
        echo "  Install: sudo apt install libdlib-dev"
        echo "  Or build from source: https://github.com/davisking/dlib"
        DEPS_MISSING=1
    fi
fi

# Check for OpenCV
if ! pkg-config --exists opencv4 2>/dev/null && ! pkg-config --exists opencv 2>/dev/null; then
    echo -e "${RED}Missing: OpenCV${NC}"
    echo "  Install: sudo apt install libopencv-dev"
    DEPS_MISSING=1
fi

# Check for BLAS (required for dlib)
if ! pkg-config --exists blas 2>/dev/null && ! pkg-config --exists openblas 2>/dev/null; then
    if [ ! -f /usr/lib/x86_64-linux-gnu/libblas.so ] && [ ! -f /usr/lib/libblas.so ]; then
        echo -e "${YELLOW}Warning: BLAS not found (recommended for performance)${NC}"
        echo "  Install: sudo apt install libopenblas-dev"
    fi
fi

# Check for PAM headers
if [ ! -f /usr/include/security/pam_modules.h ]; then
    echo -e "${RED}Missing: PAM development headers${NC}"
    echo "  Install: sudo apt install libpam0g-dev"
    DEPS_MISSING=1
fi

# Check for cmake (needed for dlib-face-recognition)
if ! command -v cmake &> /dev/null; then
    echo -e "${RED}Missing: cmake${NC}"
    echo "  Install: sudo apt install cmake"
    DEPS_MISSING=1
fi

# Check for clang (needed for bindgen)
if ! command -v clang &> /dev/null; then
    echo -e "${YELLOW}Warning: clang not found (may be needed for building)${NC}"
    echo "  Install: sudo apt install clang"
fi

if [ $DEPS_MISSING -eq 1 ]; then
    echo ""
    echo -e "${RED}Please install missing dependencies and try again.${NC}"
    echo ""
    echo "Quick install (Debian/Ubuntu):"
    echo "  sudo apt install build-essential cmake clang pkg-config"
    echo "  sudo apt install libdlib-dev libopencv-dev libopenblas-dev libpam0g-dev"
    exit 1
fi

echo -e "${GREEN}All dependencies found!${NC}"
echo ""

# Build the project
echo -e "${YELLOW}Building PAM module (release mode)...${NC}"
echo "This may take several minutes on first build."
echo ""

# Set environment variables for build
export OPENCV_LINK_LIBS="opencv_core,opencv_imgproc,opencv_videoio,opencv_highgui"

# Build in release mode
cargo build --release

# Check if build succeeded
if [ ! -f target/release/libpam_facerec.so ]; then
    echo -e "${RED}Build failed: libpam_facerec.so not found${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}   Build Successful!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "PAM module built: target/release/libpam_facerec.so"
echo ""
echo "Next steps:"
echo "  1. Download dlib models: ./download_models.sh"
echo "  2. Install the module: sudo ./install.sh"
echo "  3. Register your face using the Python GUI"
echo "  4. Configure PAM (see install.sh for details)"
echo ""
