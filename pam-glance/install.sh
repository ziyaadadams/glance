#!/bin/bash
# Install script for pam-facerec
# Installs the PAM module and required files

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}   PAM FaceRec - Install Script${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

# Check for root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Error: This script must be run as root${NC}"
    echo "Please run: sudo ./install.sh"
    exit 1
fi

# Check if module is built
if [ ! -f target/release/libpam_facerec.so ]; then
    echo -e "${RED}Error: PAM module not built${NC}"
    echo "Please run ./build.sh first"
    exit 1
fi

# Detect Linux distribution
if [ -f /etc/os-release ]; then
    . /etc/os-release
    DISTRO=$ID
else
    DISTRO="unknown"
fi

echo -e "${BLUE}Detected distribution: $DISTRO${NC}"
echo ""

# Set PAM directory based on distro
case $DISTRO in
    arch|manjaro)
        PAM_DIR="/usr/lib/security"
        ;;
    fedora|rhel|centos)
        PAM_DIR="/usr/lib64/security"
        ;;
    *)
        # Debian, Ubuntu, and others
        if [ -d "/lib/x86_64-linux-gnu/security" ]; then
            PAM_DIR="/lib/x86_64-linux-gnu/security"
        elif [ -d "/lib/security" ]; then
            PAM_DIR="/lib/security"
        else
            PAM_DIR="/usr/lib/security"
        fi
        ;;
esac

echo -e "${YELLOW}Installing PAM module to: $PAM_DIR${NC}"

# Create directories
mkdir -p "$PAM_DIR"
mkdir -p /etc/facerec
mkdir -p /usr/share/facerec/models

# Install PAM module
cp target/release/libpam_facerec.so "$PAM_DIR/pam_facerec.so"
chmod 644 "$PAM_DIR/pam_facerec.so"
echo -e "${GREEN}✓ PAM module installed${NC}"

# Create default config if it doesn't exist
if [ ! -f /etc/facerec/config.json ]; then
    cat > /etc/facerec/config.json << 'EOF'
{
    "camera": {
        "ir_device": "/dev/video2",
        "rgb_device": "/dev/video0",
        "prefer_ir": true,
        "resolution": [640, 480]
    },
    "recognition": {
        "tolerance": 0.6,
        "ir_tolerance": 0.55,
        "rgb_tolerance": 0.6,
        "min_brightness": 20.0,
        "timeout_seconds": 5,
        "multi_pose": true,
        "pose_count": 5
    },
    "ir_emitter": {
        "enabled": true,
        "auto_configure": false,
        "device": "/dev/video2"
    },
    "registered_faces": {}
}
EOF
    chmod 644 /etc/facerec/config.json
    echo -e "${GREEN}✓ Default config created${NC}"
else
    echo -e "${YELLOW}✓ Config already exists, not overwriting${NC}"
fi

# Check for dlib models
MODELS_EXIST=0
if [ -f /usr/share/facerec/models/shape_predictor_68_face_landmarks.dat ] && \
   [ -f /usr/share/facerec/models/dlib_face_recognition_resnet_model_v1.dat ]; then
    MODELS_EXIST=1
    echo -e "${GREEN}✓ dlib models found${NC}"
else
    echo -e "${YELLOW}! dlib models not found${NC}"
    echo "  Run ./download_models.sh to download them"
fi

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}   Installation Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo -e "${BLUE}PAM Configuration:${NC}"
echo ""
echo "To enable face recognition for sudo, add this line to /etc/pam.d/sudo:"
echo ""
echo -e "  ${GREEN}auth sufficient pam_facerec.so${NC}"
echo ""
echo "Add it BEFORE the @include common-auth line:"
echo ""
cat << 'EOF'
  # /etc/pam.d/sudo
  auth    sufficient    pam_facerec.so timeout=5 prefer_ir
  @include common-auth
  @include common-account
  ...
EOF
echo ""
echo -e "${YELLOW}Available PAM options:${NC}"
echo "  timeout=N     - Authentication timeout in seconds (default: 5)"
echo "  prefer_ir     - Prefer IR camera (default)"
echo "  prefer_rgb    - Prefer RGB camera"
echo "  data_dir=PATH - Directory with face data"
echo "  config=PATH   - Path to config file"
echo "  debug         - Enable debug logging"
echo ""
echo -e "${BLUE}For login/GDM, edit /etc/pam.d/gdm-password or /etc/pam.d/login${NC}"
echo ""

if [ $MODELS_EXIST -eq 0 ]; then
    echo -e "${YELLOW}IMPORTANT: Download dlib models before using:${NC}"
    echo "  ./download_models.sh"
    echo ""
fi

echo "To register your face, use the Python GUI app:"
echo "  python3 scripts/gui_gtk.py"
echo ""
