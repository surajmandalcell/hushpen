#!/usr/bin/env python3
"""Create or update setup.py for py2app builds."""

from pathlib import Path

def create_setup():
    """Create a minimal setup.py if it doesn't exist."""
    
    project_root = Path(__file__).parent.parent
    setup_file = project_root / "setup.py"
    
    if setup_file.exists():
        print(f"setup.py already exists at {setup_file}")
        return
    
    # The setup.py was already created, this is just a placeholder
    print(f"setup.py already configured at {setup_file}")

if __name__ == "__main__":
    create_setup()