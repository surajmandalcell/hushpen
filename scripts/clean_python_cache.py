#!/usr/bin/env python3
"""
Clean Python cache files script for ASR Pro project.
This script removes all __pycache__ directories and .pyc files.
"""

import os
import shutil
import sys
from pathlib import Path


def clean_python_cache(root_dir: str = ".") -> None:
    """
    Remove all Python cache files and directories.
    
    Args:
        root_dir: Root directory to clean (default: current directory)
    """
    root_path = Path(root_dir).resolve()
    removed_dirs = 0
    removed_files = 0
    
    print(f"Cleaning Python cache files in: {root_path}")
    
    # Remove __pycache__ directories
    for pycache_dir in root_path.rglob("__pycache__"):
        if pycache_dir.is_dir():
            try:
                shutil.rmtree(pycache_dir)
                print(f"Removed directory: {pycache_dir.relative_to(root_path)}")
                removed_dirs += 1
            except Exception as e:
                print(f"Error removing {pycache_dir}: {e}", file=sys.stderr)
    
    # Remove .pyc files
    for pyc_file in root_path.rglob("*.pyc"):
        if pyc_file.is_file():
            try:
                pyc_file.unlink()
                print(f"Removed file: {pyc_file.relative_to(root_path)}")
                removed_files += 1
            except Exception as e:
                print(f"Error removing {pyc_file}: {e}", file=sys.stderr)
    
    # Remove .pyo files (Python optimization files)
    for pyo_file in root_path.rglob("*.pyo"):
        if pyo_file.is_file():
            try:
                pyo_file.unlink()
                print(f"Removed file: {pyo_file.relative_to(root_path)}")
                removed_files += 1
            except Exception as e:
                print(f"Error removing {pyo_file}: {e}", file=sys.stderr)
    
    print(f"\nCleanup complete:")
    print(f"  - Removed {removed_dirs} __pycache__ directories")
    print(f"  - Removed {removed_files} cache files")


if __name__ == "__main__":
    # Allow specifying a different root directory
    root_dir = sys.argv[1] if len(sys.argv) > 1 else "."
    clean_python_cache(root_dir)
