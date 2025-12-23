<p align="center">
  <img src="glance/data/icons/hicolor/scalable/apps/io.github.glance.Glance.svg" alt="Glance" width="128">
</p>

<h1 align="center">Glance</h1>

<p align="center">
  <b>🛡️ Windows Hello™ style facial authentication for Linux</b>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-0.2-blue" alt="Version">
  <img src="https://img.shields.io/badge/rust-1.70+-orange" alt="Rust">
  <img src="https://img.shields.io/badge/license-GPL--3.0-green" alt="License">
  <img src="https://img.shields.io/badge/tested-Ubuntu%2025.10-E95420" alt="Ubuntu">
  <img src="https://img.shields.io/badge/GNOME-49-4A86CF" alt="GNOME">
</p>

---

Glance provides Windows Hello™ style authentication for Linux. Use your built-in IR emitters and camera in combination with facial recognition to prove who you are.

Using the central authentication system (PAM), this works everywhere you would otherwise need your password: login, lock screen, sudo, su, etc.

## Installation

Glance is currently available for Ubuntu and other Debian-based distributions. Built with Rust for performance and safety.

> **Note:** The build of dlib can take several minutes. Give it time.

### Ubuntu / Debian

Clone the repository and run the installer:

```bash
git clone https://github.com/ziyaadsmada/glance.git
cd glance
sudo ./install.sh
```

This will guide you through the installation and automatically:
- Install all dependencies
- Download face recognition models (~122 MB)
- Build the GTK4 GUI application
- Build and install the PAM module
- Configure PAM for sudo, login, and lock screen

### Building from source

If you prefer to build manually, the following dependencies are required:

#### Dependencies

- Rust 1.70 or higher
- GTK4 and Libadwaita
- OpenCV
- dlib (face recognition)
- LLVM/Clang (for dlib bindings)

To install them on Debian/Ubuntu:

```bash
sudo apt-get update && sudo apt-get install -y \
    cmake build-essential pkg-config curl wget bzip2 git \
    libopencv-dev libopenblas-dev liblapack-dev libjpeg-dev libpng-dev \
    libcairo2-dev libgirepository1.0-dev libglib2.0-dev \
    gir1.2-gtk-4.0 gir1.2-adw-1 libclang-dev llvm-dev v4l-utils
```

#### Build

```bash
# Build GUI application
cd glance
cargo build --release

# Build PAM module  
cd ../pam-glance
cargo build --release
```

You can install to your system with:

```bash
sudo cp glance/target/release/glance /usr/local/bin/
sudo cp pam-glance/target/release/libpam_glance.so /lib/x86_64-linux-gnu/security/pam_glance.so
```

## Setup

After installation, Glance needs to learn what you look like so it can recognise you later.

Launch the Glance application from your app menu or run:

```bash
glance
```

Click **"Add Face"** and follow the on-screen instructions. The application will capture multiple angles of your face.

If nothing went wrong, we should be able to run sudo by just showing your face. Open a new terminal and run `sudo -i` to see it in action.

## Features

| Feature | Description |
|---------|-------------|
| **IR Camera Support** | Native support for Windows Hello-compatible infrared cameras |
| **RGB Camera Fallback** | Works with standard webcams when IR is unavailable |
| **PAM Integration** | Seamless authentication for sudo, GDM, login, and screen lock |
| **GTK4 Interface** | Modern GNOME-style application using Libadwaita |
| **Fast Authentication** | Native Rust PAM module with ~100-300ms authentication time |
| **Secure Storage** | Face encodings stored as 128-dimensional vectors, not images |

## IR Camera Configuration

For Windows Hello-compatible IR cameras, you may need to configure the IR emitter:

```bash
# Install the IR emitter tool
pip3 install linux-enable-ir-emitter

# Configure the emitter (interactive)
sudo linux-enable-ir-emitter configure

# Enable on boot
sudo linux-enable-ir-emitter boot enable
```

## Troubleshooting

Any errors get logged directly into the system journal. You can view them with:

```bash
journalctl -t pam_glance
```

Or check the auth log:

```bash
sudo tail -f /var/log/auth.log
```

### Common Issues

**Camera not detected:**
```bash
sudo usermod -aG video $USER
# Log out and back in
```

**Face not recognized:**
- Ensure adequate lighting
- Face the camera directly
- Re-register your face with `glance`

**Locked out:**
PAM backups are created during installation. From recovery mode:
```bash
sudo sed -i '/pam_glance.so/d' /etc/pam.d/sudo
```

## A Note on Security

This package is in no way as secure as a password and will never be. Although it's harder to fool than basic face recognition (especially with IR cameras), a person who looks similar to you could potentially authenticate.

Glance is a more quick and convenient way of logging in, not a more secure one.

**IR cameras provide significantly better security** than RGB cameras as they are nearly impossible to spoof with photographs or screens.

⚠️ **DO NOT USE GLANCE AS THE SOLE AUTHENTICATION METHOD FOR YOUR SYSTEM.**

## Data Storage

| Location | Purpose |
|----------|---------|
| `~/.local/share/glance/` | User face encodings |
| `/var/lib/glance/` | System-wide face data |
| `/usr/share/glance/models/` | Face recognition models |

## Uninstallation

```bash
# Remove PAM configuration
sudo sed -i '/pam_glance.so/d' /etc/pam.d/sudo
sudo sed -i '/pam_glance.so/d' /etc/pam.d/gdm-password

# Remove binaries
sudo rm /usr/local/bin/glance
sudo rm /lib/x86_64-linux-gnu/security/pam_glance.so

# Remove data (optional)
rm -rf ~/.local/share/glance
sudo rm -rf /var/lib/glance
sudo rm -rf /usr/share/glance

# Remove desktop entry and icon
sudo rm /usr/share/applications/io.github.glance.Glance.desktop
sudo rm /usr/share/icons/hicolor/scalable/apps/io.github.glance.Glance.svg
```

## Contributing

The easiest ways to contribute to Glance is by starring the repository and opening GitHub issues for features you'd like to see.

Code contributions are also welcome. Please open an issue to discuss proposed changes before submitting a pull request.

## Tested On

- **Ubuntu 25.10** with GNOME 49

## License

GPL-3.0

## Credits

Inspired by [Howdy](https://github.com/boltgolt/howdy) - Windows Hello™ style authentication for Linux.
