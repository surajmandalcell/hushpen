project('asrpro-gtk4', 'rust',
  version: '1.0.0',
  default_options: ['warning_level=2',
                    'c_std=c99',
                    'rust_std=2021'])

# Dependencies
gtk4_dep = dependency('gtk4')
json_glib_dep = dependency('json-glib-1.0')
libsoup_dep = dependency('libsoup-3.0')

# Include directories
inc = include_directories('include')

# Resources
gnome = import('gnome')
resources = gnome.compile_resources(
  'asrpro-resources',
  'data/resources.xml',
  source_dir: 'data',
  c_name: 'asrpro',
  install: true
)

# Rust executable
executable('asrpro-gtk4',
  'src/main.rs',
  resources,
  dependencies: [gtk4_dep, json_glib_dep, libsoup_dep],
  install: true
)