project('facerec', 'rust',
  version: '1.0.0',
  license: 'MIT',
  meson_version: '>= 1.0.0',
)

gnome = import('gnome')

# Application ID
app_id = 'io.github.facerec'

# Directories
prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')
pkgdatadir = datadir / meson.project_name()

# Dependencies
dependency('gtk4', version: '>= 4.12')
dependency('libadwaita-1', version: '>= 1.4')

# Compile Blueprint files
blueprints = gnome.compile_blueprints(
  output: '.',
  blueprints: files(
    'data/ui/window.blp',
    'data/ui/add-face-dialog.blp',
    'data/ui/preferences.blp',
    'data/ui/ir-setup-dialog.blp',
  ),
)

# Compile resources
gnome.compile_resources(
  'facerec',
  'data/facerec.gresource.xml',
  gresource_bundle: true,
  install: true,
  install_dir: pkgdatadir,
  dependencies: blueprints,
)

# Build Rust application
cargo = find_program('cargo')
cargo_script = find_program('scripts/cargo-build.sh')

custom_target(
  'facerec-bin',
  output: 'facerec',
  command: [
    cargo_script,
    meson.project_source_root(),
    meson.project_build_root(),
    '@OUTPUT@',
    get_option('buildtype'),
  ],
  install: true,
  install_dir: bindir,
  console: true,
)

# Desktop file
install_data(
  'data/io.github.facerec.desktop',
  install_dir: datadir / 'applications',
)

# Icon
install_data(
  'data/icons/io.github.facerec.svg',
  install_dir: datadir / 'icons' / 'hicolor' / 'scalable' / 'apps',
)

# AppStream metadata
install_data(
  'data/io.github.facerec.metainfo.xml',
  install_dir: datadir / 'metainfo',
)

# PAM module (optional, built separately)
subdir('pam')

summary({
  'prefix': prefix,
  'bindir': bindir,
  'datadir': datadir,
}, section: 'Directories')
