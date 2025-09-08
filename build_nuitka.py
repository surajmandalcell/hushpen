#!/usr/bin/env python3
"""
Nuitka build script for asrpro standalone executable.
Builds a single executable with all dependencies bundled.
"""

import subprocess
import sys
import os
from pathlib import Path


def build_asrpro():
    """Build asrpro with Nuitka for standalone deployment."""

    # Ensure we're in the project root
    project_root = Path(__file__).parent
    os.chdir(project_root)

    # Check if assets directory exists
    assets_dir = project_root / "assets"
    assets_include = (
        f"--include-data-files={assets_dir}/*={assets_dir.name}/*"
        if assets_dir.exists()
        else ""
    )

    # Nuitka build command
    cmd = [
        sys.executable,
        "-m",
        "nuitka",
        "--standalone",
        "--enable-plugin=pyside6",
        "--onefile",
        "--nofollow-import-to=tkinter",
        "--nofollow-import-to=matplotlib",
        "--nofollow-import-to=scipy.spatial.cKDTree",
        "--output-dir=dist",
        "--output-filename=asrpro.exe",
        "--windows-console-mode=disable",  # No console window on Windows
        (
            "--windows-icon-from-ico=assets/icon.ico"
            if (assets_dir / "icon.ico").exists()
            else ""
        ),
        assets_include,
        "--follow-imports",
        "--prefer-source-code",
        "asrpro_run.py",
    ]

    # Remove empty arguments
    cmd = [arg for arg in cmd if arg]

    print("Building asrpro with Nuitka...")
    print("Command:", " ".join(cmd))
    print()

    try:
        # Run Nuitka build
        result = subprocess.run(cmd, check=True, capture_output=False)
        print("\n✅ Build completed successfully!")
        print(f"Executable created: {project_root / 'dist' / 'asrpro.exe'}")

        # Show file size
        exe_path = project_root / "dist" / "asrpro.exe"
        if exe_path.exists():
            size_mb = exe_path.stat().st_size / (1024 * 1024)
            print(f"File size: {size_mb:.1f} MB")

    except subprocess.CalledProcessError as e:
        print(f"❌ Build failed with exit code {e.returncode}")
        sys.exit(1)
    except FileNotFoundError:
        print("❌ Nuitka not found. Install with: pip install nuitka")
        sys.exit(1)


if __name__ == "__main__":
    print("asrpro Nuitka Build Script")
    print("=" * 30)
    build_asrpro()
