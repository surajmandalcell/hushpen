#!/usr/bin/env python3
"""
Nuitka build script for asrpro standalone executable.
Builds a single executable with all dependencies bundled.
"""

import subprocess
import sys
import os
import platform
from pathlib import Path


def build_asrpro():
    """Build asrpro with Nuitka for standalone deployment."""

    # Ensure we're in the project root
    project_root = Path(__file__).parent.parent  # scripts/ -> project root
    os.chdir(project_root)

    # Check if assets directory exists
    assets_dir = project_root / "assets"
    assets_include = (
        f"--include-data-files={assets_dir}/*={assets_dir.name}/*"
        if assets_dir.exists()
        else ""
    )

    # Platform-specific output filename
    is_windows = platform.system() == 'Windows'
    is_macos = platform.system() == 'Darwin'
    
    output_name = "asrpro.exe" if is_windows else "asrpro"
    
    # Create entry point if it doesn't exist
    entry_point = project_root / "scripts" / "asrpro_run.py"
    if not entry_point.exists():
        entry_point.write_text("""
#!/usr/bin/env python3
"""Entry point for Nuitka build."""
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from asrpro.__main__ import main

if __name__ == "__main__":
    main()
""")
    
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
        f"--output-filename={output_name}",
    ]
    
    # Windows-specific options
    if is_windows:
        cmd.extend([
            "--windows-console-mode=disable",
            "--windows-icon-from-ico=assets/icon.ico" if (assets_dir / "icon.ico").exists() else "",
        ])
    
    # macOS-specific options
    if is_macos:
        if (assets_dir / "icon.icns").exists():
            cmd.append(f"--macos-app-icon={assets_dir / 'icon.icns'}")
    
    cmd.extend([
        assets_include,
        "--follow-imports",
        "--prefer-source-code",
        str(entry_point),
    ])

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
