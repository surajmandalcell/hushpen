#!/usr/bin/env python3
"""Generate macOS .icns icon file from the source icon."""

import subprocess
import shutil
from pathlib import Path
import tempfile
import sys


def create_icns_from_png(source_png: Path, output_icns: Path):
    """Create a macOS .icns file from a PNG image."""
    
    # Required icon sizes for macOS .icns
    sizes = [
        (16, 16),
        (32, 32),
        (64, 64),
        (128, 128),
        (256, 256),
        (512, 512),
        (1024, 1024),  # For Retina displays
    ]
    
    # Create temporary iconset directory
    with tempfile.TemporaryDirectory() as temp_dir:
        iconset_path = Path(temp_dir) / "icon.iconset"
        iconset_path.mkdir()
        
    # Open source image
        try:
            from PIL import Image
            source_image = Image.open(source_png)
            
            # Convert to RGBA if needed
            if source_image.mode != 'RGBA':
                source_image = source_image.convert('RGBA')
            
            print(f"Source image size: {source_image.size}")
            
            # Generate all required sizes
            for size in sizes:
                width, height = size
                
                # Standard resolution
                resized = source_image.resize((width, height), Image.Resampling.LANCZOS)
                resized.save(iconset_path / f"icon_{width}x{height}.png")
                
                # Retina resolution (@2x)
                if width <= 512:  # Don't create @2x for 1024
                    resized_2x = source_image.resize((width * 2, height * 2), Image.Resampling.LANCZOS)
                    resized_2x.save(iconset_path / f"icon_{width}x{height}@2x.png")
            
            print(f"Created iconset at: {iconset_path}")
            
            # Convert iconset to icns using macOS iconutil
            result = subprocess.run(
                ["iconutil", "-c", "icns", "-o", str(output_icns), str(iconset_path)],
                capture_output=True,
                text=True
            )
            
            if result.returncode == 0:
                print(f"✅ Successfully created: {output_icns}")
                return True
            else:
                print(f"❌ Failed to create icns: {result.stderr}")
                return False
                
        except Exception as e:
            print(f"❌ Error processing image: {e}")
            return False


def main():
    """Main function to create macOS icon."""
    
    # Get project root
    project_root = Path(__file__).parent.parent
    
    # Source and destination paths
    source_png = project_root / "assets" / "icon.png"
    output_icns = project_root / "assets" / "icon.icns"
    
    # Check if source exists
    if not source_png.exists():
        print(f"❌ Source icon not found: {source_png}")
        print("Please ensure assets/icon.png exists")
        sys.exit(1)
    
    # Check if iconutil is available (macOS only)
    if shutil.which("iconutil") is None:
        print("❌ iconutil not found. This tool is only available on macOS.")
        print("For other platforms, use an online converter or a third-party tool.")
        sys.exit(1)
    
    # Check if Pillow is installed
    try:
        from PIL import Image
    except ImportError:
        print("❌ Pillow not installed. Installing...")
        subprocess.run([sys.executable, "-m", "pip", "install", "Pillow"])
        from PIL import Image
    
    # Create the icon
    if create_icns_from_png(source_png, output_icns):
        print(f"\n✅ Icon created successfully!")
        print(f"Location: {output_icns}")
        print("\nYou can now use this icon for:")
        print("  - py2app builds (automatically detected)")
        print("  - Manual .app bundle creation")
        print("  - Dock icon customization")
    else:
        sys.exit(1)


if __name__ == "__main__":
    main()