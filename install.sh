#!/bin/bash

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

print_status() { echo -e "${GREEN}[✓]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[!]${NC} $1"; }
print_error() { echo -e "${RED}[✗]${NC} $1"; }
print_info() { echo -e "${BLUE}[i]${NC} $1"; }
print_step() { echo -e "${CYAN}[→]${NC} ${BOLD}$1${NC}"; }

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$DIR"
ACTUAL_USER="${SUDO_USER:-$USER}"
ACTUAL_HOME=$(eval echo "~$ACTUAL_USER")

echo ""
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║                  Glance - Facial Recognition for Linux           ║"
echo "║                     Windows Hello-like Experience                ║"
echo "║                                                                  ║"
echo "║  Features:                                                       ║"
echo "║    • IR Camera Support (Windows Hello compatible)                ║"
echo "║    • RGB Camera Fallback                                         ║"
echo "║    • GTK4/Libadwaita Modern UI                                   ║"
echo "║    • PAM Integration for sudo/login/lock screen                  ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""

if [ "$EUID" -ne 0 ]; then 
    print_error "Please run as root: sudo ./install.sh"
    exit 1
fi

print_info "Installing for user: $ACTUAL_USER"
print_info "Project root: $PROJECT_ROOT"
echo ""

# =============================================================================
# Step 1: Camera permissions
# =============================================================================
print_step "Step 1: Setting up camera permissions..."

NEED_RELOGIN=false
if ! groups "$ACTUAL_USER" | grep -q '\bvideo\b'; then
    usermod -aG video "$ACTUAL_USER"
    print_status "Added $ACTUAL_USER to 'video' group"
    NEED_RELOGIN=true
else
    print_status "$ACTUAL_USER already in 'video' group"
fi

# =============================================================================
# Step 2: System dependencies
# =============================================================================
echo ""
print_step "Step 2: Installing system dependencies..."

apt-get update -qq

# Core build dependencies
apt-get install -y -qq \
    cmake \
    build-essential \
    pkg-config \
    libopenblas-dev \
    liblapack-dev \
    libjpeg-dev \
    libpng-dev \
    curl \
    wget \
    bzip2 \
    git \
    2>/dev/null

# GTK4/Libadwaita dependencies
apt-get install -y -qq \
    libcairo2-dev \
    libgirepository1.0-dev \
    libglib2.0-dev \
    gir1.2-gtk-4.0 \
    gir1.2-adw-1 \
    2>/dev/null

# OpenCV and Rust dependencies
apt-get install -y -qq \
    libopencv-dev \
    libclang-dev \
    llvm-dev \
    2>/dev/null

print_status "System dependencies installed"

# =============================================================================
# Step 3: IR Camera support
# =============================================================================
echo ""
print_step "Step 3: Installing IR camera support..."

apt-get install -y -qq \
    v4l-utils \
    ir-keytable \
    2>/dev/null

if ! command -v linux-enable-ir-emitter &> /dev/null; then
    print_info "Installing linux-enable-ir-emitter..."
    
    pip3 install --break-system-packages -q linux-enable-ir-emitter 2>/dev/null || {
        print_warning "pip install failed, trying from source..."
        cd /tmp
        if [ -d "linux-enable-ir-emitter" ]; then
            rm -rf linux-enable-ir-emitter
        fi
        git clone https://github.com/EmixamPP/linux-enable-ir-emitter.git 2>/dev/null || true
        if [ -d "linux-enable-ir-emitter" ]; then
            cd linux-enable-ir-emitter
            pip3 install --break-system-packages . 2>/dev/null || true
            cd /tmp
            rm -rf linux-enable-ir-emitter
        fi
    }
    
    if command -v linux-enable-ir-emitter &> /dev/null; then
        print_status "linux-enable-ir-emitter installed"
    else
        print_warning "linux-enable-ir-emitter installation failed (optional)"
    fi
else
    print_status "linux-enable-ir-emitter already installed"
fi

cd "$PROJECT_ROOT"

# =============================================================================
# Step 4: Detect cameras
# =============================================================================
echo ""
print_step "Step 4: Detecting cameras..."

echo ""
print_info "Available cameras:"
for dev in /dev/video*; do
    if [ -c "$dev" ]; then
        NAME=$(v4l2-ctl -d "$dev" --info 2>/dev/null | grep "Card type" | cut -d: -f2 | xargs)
        if [ -n "$NAME" ]; then
            echo "  $dev: $NAME"
        fi
    fi
done 2>/dev/null || true
echo ""

IR_CAMERA=""
for i in 0 1 2 3 4 5 6 7 8 9; do
    DEV="/dev/video$i"
    if [ -c "$DEV" ]; then
        INFO=$(v4l2-ctl -d "$DEV" --info 2>/dev/null || true)
        NAME=$(echo "$INFO" | grep -i "Card type" | cut -d: -f2 | xargs)
        
        if echo "$NAME" | grep -qi "IR\|infrared\|depth"; then
            IR_CAMERA="$DEV"
            print_status "Found IR camera: $DEV ($NAME)"
            break
        fi
    fi
done

if [ -n "$IR_CAMERA" ] && command -v linux-enable-ir-emitter &> /dev/null; then
    echo ""
    print_info "IR camera detected. Would you like to configure the IR emitter?"
    echo "  This is needed for Windows Hello-style cameras."
    echo ""
    read -p "Configure IR emitter now? [y/N]: " CONFIGURE_IR
    
    if [[ "$CONFIGURE_IR" =~ ^[Yy] ]]; then
        print_info "Running IR emitter configuration..."
        print_warning "Follow the on-screen prompts. Look at the camera when asked."
        echo ""
        
        linux-enable-ir-emitter configure || {
            print_warning "IR emitter configuration failed. You can run it later with:"
            echo "  sudo linux-enable-ir-emitter configure"
        }
        
        if [ -f /etc/linux-enable-ir-emitter.conf ]; then
            linux-enable-ir-emitter boot enable 2>/dev/null || true
            print_status "IR emitter configured to start on boot"
        fi
    else
        print_info "Skipping IR emitter configuration. Run later with:"
        echo "  sudo linux-enable-ir-emitter configure"
    fi
fi

# =============================================================================
# Step 5: Create data directories
# =============================================================================
echo ""
print_step "Step 5: Creating data directories..."

mkdir -p /usr/share/glance/models
mkdir -p /etc/glance
mkdir -p /var/lib/glance
mkdir -p "$ACTUAL_HOME/.local/share/glance"

chown -R "$ACTUAL_USER:$ACTUAL_USER" /var/lib/glance
chown -R "$ACTUAL_USER:$ACTUAL_USER" "$ACTUAL_HOME/.local/share/glance"
chmod 755 /var/lib/glance
chmod 755 "$ACTUAL_HOME/.local/share/glance"

print_status "Data directories created"

# =============================================================================
# Step 6: Download face recognition models
# =============================================================================
echo ""
print_step "Step 6: Downloading face recognition models..."

MODELS_DIR="/usr/share/glance/models"

if [ ! -f "$MODELS_DIR/shape_predictor_68_face_landmarks.dat" ]; then
    print_info "Downloading shape predictor model (~100MB)..."
    cd /tmp
    
    if [ ! -f "shape_predictor_68_face_landmarks.dat" ]; then
        curl -sLO http://dlib.net/files/shape_predictor_68_face_landmarks.dat.bz2
        bunzip2 -f shape_predictor_68_face_landmarks.dat.bz2
    fi
    cp shape_predictor_68_face_landmarks.dat "$MODELS_DIR/"
    rm -f shape_predictor_68_face_landmarks.dat*
    
    print_status "Shape predictor model downloaded"
else
    print_status "Shape predictor model already exists"
fi

if [ ! -f "$MODELS_DIR/dlib_face_recognition_resnet_model_v1.dat" ]; then
    print_info "Downloading face recognition model (~22MB)..."
    cd /tmp
    
    if [ ! -f "dlib_face_recognition_resnet_model_v1.dat" ]; then
        curl -sLO http://dlib.net/files/dlib_face_recognition_resnet_model_v1.dat.bz2
        bunzip2 -f dlib_face_recognition_resnet_model_v1.dat.bz2
    fi
    cp dlib_face_recognition_resnet_model_v1.dat "$MODELS_DIR/"
    rm -f dlib_face_recognition_resnet_model_v1.dat*
    
    print_status "Face recognition model downloaded"
else
    print_status "Face recognition model already exists"
fi

cd "$PROJECT_ROOT"

# =============================================================================
# Step 7: Build and install Rust components
# =============================================================================
echo ""
print_step "Step 7: Building Rust components..."

# Ensure Rust is installed
CARGO_BIN="$ACTUAL_HOME/.cargo/bin/cargo"
if [ ! -f "$CARGO_BIN" ]; then
    if ! command -v cargo &> /dev/null; then
        print_warning "Rust not found. Installing via rustup..."
        su - "$ACTUAL_USER" -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
    fi
fi

# Build the GUI app
if [ -d "$PROJECT_ROOT/glance" ]; then
    print_info "Building Glance GUI app..."
    cd "$PROJECT_ROOT/glance"
    su - "$ACTUAL_USER" -c "cd '$PROJECT_ROOT/glance' && source ~/.cargo/env 2>/dev/null; cargo build --release 2>&1" | tail -5
    
    if [ -f "target/release/glance" ]; then
        cp target/release/glance /usr/local/bin/glance
        chmod 755 /usr/local/bin/glance
        print_status "Glance GUI installed to /usr/local/bin/glance"
    else
        print_error "Failed to build Glance GUI"
    fi
    cd "$PROJECT_ROOT"
fi

# Build the PAM module
if [ -d "$PROJECT_ROOT/pam-glance" ]; then
    print_info "Building PAM module..."
    cd "$PROJECT_ROOT/pam-glance"
    su - "$ACTUAL_USER" -c "cd '$PROJECT_ROOT/pam-glance' && source ~/.cargo/env 2>/dev/null; cargo build --release 2>&1" | tail -5
    
    if [ -f "target/release/libpam_glance.so" ]; then
        cp target/release/libpam_glance.so /lib/x86_64-linux-gnu/security/pam_glance.so
        chmod 644 /lib/x86_64-linux-gnu/security/pam_glance.so
        print_status "PAM module installed to /lib/x86_64-linux-gnu/security/"
    else
        print_error "Failed to build PAM module"
    fi
    cd "$PROJECT_ROOT"
fi

# =============================================================================
# Step 8: Install desktop entry and icon
# =============================================================================
echo ""
print_step "Step 8: Installing desktop entry..."

# Desktop entry
cat > /usr/share/applications/io.github.glance.Glance.desktop << 'EOF'
[Desktop Entry]
Name=Glance
Comment=Windows Hello-style facial recognition for Linux
Exec=glance
Icon=io.github.glance.Glance
Terminal=false
Type=Application
Categories=System;Security;Settings;
Keywords=face;facial;recognition;authentication;login;security;biometric;
StartupNotify=true
EOF

# Copy icon if it exists
if [ -f "$PROJECT_ROOT/glance/data/icons/io.github.glance.Glance.svg" ]; then
    mkdir -p /usr/share/icons/hicolor/scalable/apps
    cp "$PROJECT_ROOT/glance/data/icons/io.github.glance.Glance.svg" \
       /usr/share/icons/hicolor/scalable/apps/
    gtk-update-icon-cache /usr/share/icons/hicolor 2>/dev/null || true
    print_status "Icon installed"
fi

print_status "Desktop entry installed"

# =============================================================================
# Step 9: Configure PAM
# =============================================================================
echo ""
print_step "Step 9: Configuring PAM authentication..."

configure_pam_service() {
    local service=$1
    local pam_file="/etc/pam.d/$service"
    local pam_line="auth sufficient pam_glance.so"
    
    if [ -f "$pam_file" ]; then
        # Remove any old facerec/glance entries first
        sed -i '/pam_facerec.so/d' "$pam_file"
        sed -i '/pam_glance.so/d' "$pam_file"
        
        # Backup
        cp "$pam_file" "${pam_file}.backup.$(date +%Y%m%d%H%M%S)"
        
        # Add new entry
        if grep -q "@include common-auth" "$pam_file"; then
            sed -i "/^@include common-auth/a $pam_line" "$pam_file"
        elif grep -q "^auth" "$pam_file"; then
            sed -i "0,/^auth/s/^auth/$pam_line\nauth/" "$pam_file"
        else
            sed -i "1a $pam_line" "$pam_file"
        fi
        
        print_status "$service configured"
    else
        print_warning "PAM file not found: $pam_file"
    fi
}

echo ""
echo "Which services should use facial recognition?"
echo ""
echo "  1) sudo only (recommended for testing)"
echo "  2) sudo + lock screen (gdm-password)"
echo "  3) All common services (sudo, login, gdm, screensaver)"
echo "  4) Skip PAM configuration"
echo ""
read -p "Select option [1-4]: " PAM_CHOICE

case "$PAM_CHOICE" in
    1)
        configure_pam_service "sudo"
        ;;
    2)
        configure_pam_service "sudo"
        configure_pam_service "gdm-password"
        ;;
    3)
        configure_pam_service "sudo"
        configure_pam_service "login"
        configure_pam_service "gdm-password"
        configure_pam_service "gnome-screensaver"
        configure_pam_service "polkit-1"
        ;;
    *)
        print_info "Skipping PAM configuration"
        ;;
esac

# =============================================================================
# Done!
# =============================================================================
echo ""
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║                     Installation Complete                        ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""
print_status "Glance has been installed successfully!"
echo ""
print_info "Next steps:"
echo "  1. Run 'glance' to register your face"
echo "  2. Test with 'sudo echo test' (should recognize your face)"
echo ""

if $NEED_RELOGIN; then
    print_warning "Please log out and back in for camera permissions to take effect."
fi

echo ""
print_info "Useful commands:"
echo "  glance                                 - Open GUI to manage face data"
echo "  sudo linux-enable-ir-emitter run       - Test IR emitter"
echo "  sudo linux-enable-ir-emitter configure - Reconfigure IR emitter"
echo ""
