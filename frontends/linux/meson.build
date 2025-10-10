project('asrpro-gtk4', 'rust',
  version: '1.0.0',
  default_options: ['warning_level=2',
                    'c_std=c99',
                    'rust_std=2021'])

# Dependencies
gtk4_dep = dependency('gtk4')
json_glib_dep = dependency('json-glib-1.0')
libsoup_dep = dependency('libsoup-3.0')
gstreamer_dep = dependency('gstreamer-1.0')
gstreamer_app_dep = dependency('gstreamer-app-1.0')

# Installation directories
bindir = get_option('bindir')
datadir = get_option('datadir')
mandir = get_option('mandir')
localedir = get_option('localedir')

# Rust executable
executable('asrpro-gtk4',
  'src/main.rs',
  dependencies: [gtk4_dep, json_glib_dep, libsoup_dep, gstreamer_dep, gstreamer_app_dep],
  install: true,
  install_dir: bindir
)

# Install desktop file
install_data(
  'data/com.asrpro.ASRPro.desktop',
  install_dir: join_paths(datadir, 'applications')
)

# Install MIME type file
install_data(
  'data/asrpro-mime.xml',
  install_dir: join_paths(datadir, 'mime/packages')
)

# Install icons
icon_sizes = ['16', '32', '48', '64', '128', '256', '512']
foreach size : icon_sizes
  install_data(
    'data/icons/@0@x@0@/com.asrpro.ASRPro.png'.format(size, size),
    install_dir: join_paths(datadir, 'icons/hicolor/@0@x@0@/apps'.format(size, size))
  )
endforeach

# Install scalable icon
install_data(
  'data/icons/scalable/com.asrpro.ASRPro.svg',
  install_dir: join_paths(datadir, 'icons/hicolor/scalable/apps')
)

# Post-install scripts
update_desktop = find_program('update-desktop-database', required: false)
if update_desktop.found()
  meson.add_install_script(
    'sh', '-c',
    'update-desktop-database @0@'.format(join_paths(datadir, 'applications'))
  )
endif

update_mime = find_program('update-mime-database', required: false)
if update_mime.found()
  meson.add_install_script(
    'sh', '-c',
    'update-mime-database @0@'.format(join_paths(datadir, 'mime'))
  )
endif

update_icons = find_program('gtk-update-icon-cache', required: false)
if update_icons.found()
  meson.add_install_script(
    'sh', '-c',
    'gtk-update-icon-cache -f -t @0@'.format(join_paths(datadir, 'icons/hicolor'))
  )
endif
# Testing configuration
if get_option('buildtype').startswith('debug')
  # Enable testing in debug builds
  test('cargo-test', 
    find_program('cargo'),
    args: ['test', '--all', '--no-fail-fast'],
    env: ['RUST_BACKTRACE=1'],
    timeout: 300,  # 5 minutes timeout
    suite: 'unit'
  )

  # Unit tests
  test('cargo-test-unit', 
    find_program('cargo'),
    args: ['test', '--lib', 'tests/unit'],
    env: ['RUST_BACKTRACE=1'],
    timeout: 300,
    suite: 'unit'
  )

  # Integration tests
  test('cargo-test-integration', 
    find_program('cargo'),
    args: ['test', '--test', 'integration'],
    env: ['RUST_BACKTRACE=1'],
    timeout: 600,  # 10 minutes timeout
    suite: 'integration'
  )

  # Performance tests (only in release mode)
  test('cargo-test-performance', 
    find_program('cargo'),
    args: ['test', '--release', '--benches'],
    env: ['RUST_BACKTRACE=1'],
    timeout: 600,
    suite: 'performance'
  )

  # Coverage report generation (requires cargo-tarpaulin)
  tarpaulin = find_program('cargo-tarpaulin', required: false)
  if tarpaulin.found()
    test('cargo-coverage', 
      tarpaulin,
      args: ['--out', 'Html', '--output-dir', 'coverage/'],
      env: ['RUST_BACKTRACE=1'],
      timeout: 600,
      suite: 'coverage'
    )
  endif
endif

# Test data installation
install_subdir(
  'tests',
  install_dir: join_paths(datadir, 'asrpro-gtk4', 'tests'),
  strip_directory: true
)

# Test scripts
install_data(
  'tests/Makefile',
  install_dir: join_paths(datadir, 'asrpro-gtk4', 'tests')
)
