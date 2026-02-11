# pam-glance — Native Rust PAM Module

A native PAM module for facial recognition authentication on Linux. Built in Rust for safety and reliability.

## Features

- **IR Camera Support**: Secure infrared camera authentication (like Windows Hello)
- **RGB Fallback**: Automatically falls back to standard webcam if IR is unavailable
- **Dual Camera**: Tries IR first, then RGB, with separate tolerance thresholds
- **IR Emitter Integration**: Automatic IR LED control via `linux-enable-ir-emitter`
- **Fast Camera Detection**: Uses sysfs for instant camera discovery (no OpenCV probing)
- **3-Second Timeout**: Fails fast with "use your password" so you're never stuck waiting
- **PAM Integration**: Works with sudo, login, GDM, polkit, and any PAM-enabled service

## Requirements

### Build Dependencies

```bash
# Debian/Ubuntu
sudo apt install build-essential cmake clang pkg-config \
    libdlib-dev libopencv-dev libopenblas-dev libpam0g-dev

# Arch Linux
sudo pacman -S base-devel cmake clang opencv dlib pam

# Fedora
sudo dnf install @development-tools cmake clang \
    opencv-devel dlib-devel pam-devel openblas-devel
```

### Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Building

```bash
cargo build --release
```

## Installation

```bash
# Download dlib models (required)
sudo ./download_models.sh

# Install the PAM module
sudo ./install.sh

# Or manually:
sudo cp target/release/libpam_glance.so /usr/lib/x86_64-linux-gnu/security/pam_glance.so
```

## Configuration

### PAM Configuration

Add to `/etc/pam.d/common-auth` (or `/etc/pam.d/sudo`, etc.):

```
auth    sufficient    pam_glance.so
```

Place it **before** other auth lines. The `sufficient` keyword means: if face auth succeeds, no password needed; if it fails, fall through to password.

### PAM Options

| Option | Description | Default |
|--------|-------------|---------|
| `timeout=N` | Authentication timeout in seconds | `3` |
| `prefer_ir` | Prefer IR camera over RGB | enabled |
| `prefer_rgb` | Prefer RGB camera over IR | disabled |
| `data_dir=PATH` | Directory containing face data | `/var/lib/glance` |
| `config=PATH` | Path to config file | `~/.config/glance/config.json` |
| `debug` | Enable debug logging to syslog | disabled |

Example with options:

```
auth    sufficient    pam_glance.so timeout=5 prefer_ir
```

### Authentication Defaults

| Parameter | Value |
|-----------|-------|
| IR tolerance | 0.45 (stricter — IR is more reliable) |
| RGB tolerance | 0.50 |
| Timeout | 3 seconds |
| Max frames per camera | 15 |
| Frame rate | ~30 FPS |

## Face Registration

Use the Glance GTK application to register your face:

```bash
glance
```

Click **"Add Face"** and follow the on-screen instructions.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     PAM Authentication                       │
│                    (sudo, login, gdm)                        │
└─────────────────────────────┬───────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   pam_glance.so (Rust)                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Camera    │  │    Face     │  │   IR Emitter        │  │
│  │   Module    │  │  Recognition│  │   Control           │  │
│  │  (OpenCV)   │  │   (dlib)    │  │                     │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
└─────────┼────────────────┼────────────────────┼─────────────┘
          │                │                    │
          ▼                ▼                    ▼
    ┌──────────┐    ┌────────────┐    ┌─────────────────────┐
    │ /dev/    │    │ Face Data  │    │ linux-enable-ir-    │
    │ video*   │    │   (JSON)   │    │     emitter         │
    └──────────┘    └────────────┘    └─────────────────────┘
```

### Source Structure

```
pam-glance/src/
├── lib.rs          # PAM entry point, argument parsing
├── auth.rs         # Core authentication logic, dual-camera fallback
├── camera.rs       # Camera detection (sysfs) and capture (OpenCV)
├── config.rs       # Config file loading
├── face.rs         # Face detection & encoding (dlib)
├── ir_emitter.rs   # IR LED control via linux-enable-ir-emitter
└── bin/
    └── test_faces.rs   # CLI tool for testing face recognition
```

## Logging

Logs are written to syslog (`LOG_AUTH` facility):

```bash
# View logs
journalctl -t pam_glance

# Or check auth log
grep pam_glance /var/log/auth.log
```

## Troubleshooting

### Module not loading

Check if the module is in the correct location:
```bash
ls -la /usr/lib/x86_64-linux-gnu/security/pam_glance.so
```

### Camera not detected

List available cameras:
```bash
v4l2-ctl --list-devices
ls /sys/class/video4linux/*/name
```

### IR emitter not working

Ensure it's been calibrated:
```bash
# Check for saved config
ls /etc/linux-enable-ir-emitter/

# If empty, run calibration
sudo linux-enable-ir-emitter configure

# Test it
sudo linux-enable-ir-emitter run
```

### Face not recognized

1. Re-register your face with the Glance app
2. Ensure adequate lighting (especially for RGB)
3. Check logs: `journalctl -t pam_glance --since "5 min ago"`

### Locked out

PAM is configured with `sufficient`, so password always works as fallback. To remove face auth entirely:
```bash
sudo sed -i '/pam_glance.so/d' /etc/pam.d/common-auth
```

## Security Considerations

- IR cameras are more secure than RGB (can't be fooled by photos)
- The PAM module runs as root for authentication
- Face encodings are stored as 128-dimensional vectors, not images
- Use `sufficient` in PAM config to always allow password fallback
- **Do not** use as the sole authentication method

## License

GPL-3.0

## Credits

- [dlib](http://dlib.net/) — Face detection and recognition
- [OpenCV](https://opencv.org/) — Camera capture
- [linux-enable-ir-emitter](https://github.com/EmixamPP/linux-enable-ir-emitter) — IR LED control
- Inspired by [Howdy](https://github.com/boltgolt/howdy)
