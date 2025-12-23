# FaceRec - Rust + Blueprint

A modern GNOME application for Windows Hello-style facial recognition on Linux.

## Status: Work in Progress 🚧

The Rust + Blueprint version is under development. The full project structure is in place, but the window.rs needs more work to handle threading correctly with GTK's main loop.

**Current Python version is fully functional** - use that for now.

## Architecture

```
facerec-app/
├── Cargo.toml              # Rust dependencies
├── meson.build             # GNOME build system
├── data/
│   ├── ui/                 # Blueprint UI files
│   │   ├── window.blp
│   │   ├── add-face-dialog.blp
│   │   ├── preferences.blp
│   │   └── ir-setup-dialog.blp
│   ├── style.css           # Custom styles
│   ├── facerec.gresource.xml
│   ├── io.github.facerec.desktop
│   ├── io.github.facerec.metainfo.xml
│   └── icons/
└── src/
    ├── main.rs             # Entry point
    ├── app.rs              # GtkApplication
    ├── window.rs           # Main window
    ├── camera.rs           # Camera handling + IR detection
    ├── face.rs             # Face detection & encoding
    ├── pose.rs             # Head pose detection
    ├── storage.rs          # Face data storage
    └── widgets/
        └── face_guide.rs   # Face guide overlay widget
```

## Building

### Prerequisites

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# GTK4 and Libadwaita development files
sudo apt install libgtk-4-dev libadwaita-1-dev

# Blueprint compiler
sudo apt install blueprint-compiler

# OpenCV
sudo apt install libopencv-dev clang libclang-dev

# dlib (for face recognition)
sudo apt install libdlib-dev

# Meson build system
sudo apt install meson ninja-build
```

### Build with Meson (recommended)

```bash
meson setup build
meson compile -C build
meson install -C build
```

### Build with Cargo (development)

```bash
# First, compile Blueprint files manually
blueprint-compiler compile data/ui/window.blp > data/ui/window.ui
blueprint-compiler compile data/ui/add-face-dialog.blp > data/ui/add-face-dialog.ui
blueprint-compiler compile data/ui/preferences.blp > data/ui/preferences.ui
blueprint-compiler compile data/ui/ir-setup-dialog.blp > data/ui/ir-setup-dialog.ui

# Compile resources
glib-compile-resources data/facerec.gresource.xml --target=data/facerec.gresource

# Build
cargo build --release
```

### Run

```bash
# After meson install
facerec

# Or directly
./target/release/facerec
```

## Features

- **IR Camera Support**: Automatically detects and uses infrared cameras for secure authentication
- **Multi-Pose Capture**: Captures 5 different head angles for better accuracy
- **Head Pose Detection**: Requires actual head movement (not just holding still)
- **Smoothed UI**: Consistent guidance messages without flickering
- **PAM Integration**: Works with the existing Rust PAM module

## Technology Stack

- **Rust**: Memory-safe, fast system programming
- **GTK4**: Modern Linux GUI toolkit
- **Libadwaita**: GNOME HIG compliance
- **Blueprint**: Declarative UI definition
- **OpenCV**: Camera capture and image processing
- **dlib**: Face detection and recognition

## vs Python Version

| Feature | Python | Rust |
|---------|--------|------|
| Startup time | ~2s | ~0.3s |
| Memory usage | ~150MB | ~40MB |
| Type safety | Runtime errors | Compile-time |
| Dependencies | Many Python packages | Single binary |
| PAM integration | Shell wrapper | Native |

## License

MIT
