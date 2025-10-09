project('asrpro-gtk4', 'rust',
  version: '1.0.0',
  default_options: ['warning_level=2',
                    'c_std=c99',
                    'rust_std=2021'])

# Dependencies
gtk4_dep = dependency('gtk4')
json_glib_dep = dependency('json-glib-1.0')
libsoup_dep = dependency('libsoup-3.0')

# Rust executable
executable('asrpro-gtk4',
  'src/main.rs',
  dependencies: [gtk4_dep, json_glib_dep, libsoup_dep],
  install: true
)