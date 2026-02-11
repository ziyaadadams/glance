# Glance — GTK4 Application

The graphical interface for Glance. A modern GNOME application built with GTK4 and Libadwaita for managing facial recognition enrollment and IR camera setup.

## Features

- **Face Enrollment**: Capture multiple angles of your face for recognition
- **IR Camera Setup**: Built-in wizard for configuring IR emitters via `linux-enable-ir-emitter`
- **Camera Preview**: Live camera feed with face detection overlay
- **Preferences**: Configure tolerances, camera selection, and PAM integration
- **Default Terminal Support**: Opens the user's default terminal for IR calibration tasks via `xdg-terminal-exec`

## Architecture

```
glance/
├── Cargo.toml
├── data/
│   ├── ui/                 # Blueprint UI files
│   │   ├── window.blp
│   │   ├── add-face-dialog.blp
│   │   ├── preferences.blp
│   │   └── ir-setup-dialog.blp
│   ├── style.css
│   ├── glance.gresource.xml
│   ├── io.github.glance.Glance.desktop
│   ├── io.github.glance.Glance.metainfo.xml
│   └── icons/
└── src/
    ├── main.rs             # Entry point
    ├── app.rs              # GtkApplication
    ├── window.rs           # Main window + IR setup logic
    ├── camera.rs           # Camera handling + IR detection
    ├── face.rs             # Face detection & encoding (dlib)
    ├── models.rs           # Data models
    ├── storage.rs          # Face data storage (JSON)
    └── widgets/
        ├── mod.rs
        └── face_guide.rs   # Face guide overlay widget
```

## Building

### Prerequisites

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# GTK4, Libadwaita, and OpenCV development files
sudo apt install libgtk-4-dev libadwaita-1-dev libopencv-dev clang libclang-dev
```

### Build

```bash
cargo build --release
```

### Install

```bash
sudo cp target/release/glance /usr/local/bin/
```

### Run

```bash
glance
```

## Technology Stack

- **Rust**: Memory-safe systems programming
- **GTK4 + Libadwaita**: Modern GNOME UI
- **OpenCV**: Camera capture and image processing
- **dlib**: Face detection and recognition

## License

GPL-3.0
