#!/usr/bin/env python3
"""Setup script for ASR Pro - backward compatibility and py2app support."""

from setuptools import setup, find_packages
from pathlib import Path

# Read README for long description
this_directory = Path(__file__).parent
long_description = (this_directory / "README.md").read_text()

# Basic setup configuration (most config is in pyproject.toml)
setup(
    name="asrpro",
    version="0.1.0",
    packages=find_packages(),
    include_package_data=True,
    long_description=long_description,
    long_description_content_type="text/markdown",
    entry_points={
        "console_scripts": [
            "asrpro=asrpro.__main__:main",
        ],
    },
)

# py2app configuration for macOS
try:
    import py2app
    from setuptools import setup
    
    APP = ['scripts/asrpro_run.py']
    DATA_FILES = [
        ('assets', ['assets/icon.png', 'assets/icon.svg']),
        ('assets/icons', Path('assets/icons').glob('*.svg')),
        ('assets/fonts', Path('assets/fonts').glob('*.ttf')),
    ]
    OPTIONS = {
        'argv_emulation': True,
        'iconfile': 'assets/icon.icns' if Path('assets/icon.icns').exists() else None,
        'plist': {
            'CFBundleName': 'ASR Pro',
            'CFBundleDisplayName': 'ASR Pro',
            'CFBundleIdentifier': 'com.surajmandal.asrpro',
            'CFBundleVersion': '0.1.0',
            'CFBundleShortVersionString': '0.1.0',
            'NSHumanReadableCopyright': 'Copyright © 2024 Suraj Mandal',
            'NSHighResolutionCapable': True,
            'NSMicrophoneUsageDescription': 'ASR Pro needs access to your microphone for speech recognition.',
            'NSAppleEventsUsageDescription': 'ASR Pro needs to send Apple Events for accessibility features.',
        },
        'packages': ['PySide6', 'torch', 'torchaudio', 'transformers', 'faster_whisper'],
        'includes': ['asrpro'],
        'excludes': ['tkinter', 'matplotlib'],
    }
    
    setup(
        app=APP,
        data_files=DATA_FILES,
        options={'py2app': OPTIONS},
        setup_requires=['py2app'],
    )
except ImportError:
    # py2app not available, use regular setup
    pass