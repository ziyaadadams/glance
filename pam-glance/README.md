# PAM FaceRec - Native Rust PAM Module

A high-performance native PAM module for facial recognition authentication on Linux. Built in Rust for maximum speed and reliability.

## Features

- **Native Performance**: ~100-300ms authentication (vs ~1-2s with Python-based solutions)
- **IR Camera Support**: Secure infrared camera authentication (like Windows Hello)
- **RGB Fallback**: Works with standard webcams too
- **Multi-pose Matching**: Compare against multiple registered face angles
- **IR Emitter Integration**: Automatic IR LED control via linux-enable-ir-emitter
- **PAM Integration**: Works with sudo, login, GDM, and any PAM-enabled service

## Performance Comparison

| Solution | Auth Time | Language | Notes |
|----------|-----------|----------|-------|
| **PAM FaceRec (Rust)** | 100-300ms | Rust | Native, no interpreter overhead |
| Howdy | 1-2s | Python | Forks Python interpreter |
| Windows Hello | 100-200ms | Native | Hardware accelerated |

## Requirements

### Build Dependencies

```bash
# Debian/Ubuntu
sudo apt install build-essential cmake clang pkg-config
sudo apt install libdlib-dev libopencv-dev libopenblas-dev libpam0g-dev

# Arch Linux
sudo pacman -S base-devel cmake clang opencv dlib pam

# Fedora
sudo dnf install @development-tools cmake clang opencv-devel dlib-devel pam-devel openblas-devel
```

### Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Building

```bash
# Build the PAM module
./build.sh

# Or manually:
cargo build --release
```

## Installation

```bash
# Download dlib models (required)
sudo ./download_models.sh

# Install the PAM module
sudo ./install.sh
```

## Configuration

### PAM Configuration

Add to `/etc/pam.d/sudo` (before `@include common-auth`):

```
auth    sufficient    pam_facerec.so timeout=5 prefer_ir
```

For login manager (GDM), edit `/etc/pam.d/gdm-password`:

```
auth    sufficient    pam_facerec.so timeout=5
```

### PAM Options

| Option | Description | Default |
|--------|-------------|---------|
| `timeout=N` | Authentication timeout in seconds | 5 |
| `prefer_ir` | Prefer IR camera over RGB | enabled |
| `prefer_rgb` | Prefer RGB camera | disabled |
| `data_dir=PATH` | Directory containing face data | /etc/facerec |
| `config=PATH` | Path to config file | ~/.config/facerec/config.json |
| `debug` | Enable debug logging to syslog | disabled |

### Config File

`/etc/facerec/config.json`:

```json
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
        "device": "/dev/video2"
    }
}
```

## Face Registration

Use the Python GUI app to register your face:

```bash
cd ../
python3 scripts/gui_gtk.py
```

Or use the CLI:

```bash
python3 scripts/register_face.py
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     PAM Authentication                       │
│                    (sudo, login, gdm)                        │
└─────────────────────────────┬───────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   pam_facerec.so (Rust)                      │
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

## Logging

Logs are written to syslog (LOG_AUTH facility):

```bash
# View logs
journalctl -t pam_facerec

# Or
grep pam_facerec /var/log/auth.log
```

## Troubleshooting

### Module not loading

Check if the module is in the correct location:
```bash
ls -la /lib/x86_64-linux-gnu/security/pam_facerec.so
```

### Camera not detected

List available cameras:
```bash
v4l2-ctl --list-devices
ls /sys/class/video4linux/*/name
```

### IR camera too dark

Configure the IR emitter:
```bash
sudo linux-enable-ir-emitter --device /dev/video2 configure
```

### Face not recognized

1. Re-register your face with multiple poses
2. Increase tolerance in config (e.g., 0.7)
3. Check lighting conditions

## Security Considerations

- IR cameras are more secure than RGB (can't be fooled by photos)
- The PAM module runs as root for authentication
- Face encodings are stored as mathematical vectors, not images
- Use `sufficient` in PAM config to allow password fallback

## Development

```bash
# Build in debug mode
cargo build

# Run tests
cargo test

# Check for issues
cargo clippy
```

## License

MIT License - see LICENSE file

## Credits

- [dlib](http://dlib.net/) - Face detection and recognition
- [OpenCV](https://opencv.org/) - Camera capture
- [linux-enable-ir-emitter](https://github.com/EmixamPP/linux-enable-ir-emitter) - IR LED control
- Inspired by [Howdy](https://github.com/boltgolt/howdy)
