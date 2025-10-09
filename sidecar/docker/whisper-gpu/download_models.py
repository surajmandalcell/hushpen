#!/usr/bin/env python3
"""
Download Whisper models for GPU container
"""

import torch
import whisper
import os
import sys

def main():
    """Download Whisper models."""
    # Check GPU availability
    if not torch.cuda.is_available():
        print('CUDA not available, skipping model downloads')
        sys.exit(1)
    
    print('CUDA available:', torch.cuda.get_device_name(0))
    print('CUDA version:', torch.version.cuda)
    print('PyTorch version:', torch.__version__)
    
    # Create models directory
    models_dir = '/app/models'
    os.makedirs(models_dir, exist_ok=True)
    
    # Download small model (good balance of speed and accuracy)
    print('Downloading Whisper small model...')
    try:
        whisper.load_model('small', download_root=models_dir)
        print('Whisper small model downloaded successfully')
    except Exception as e:
        print(f'Failed to download small model: {e}')
        sys.exit(1)
    
    # Download medium model (better accuracy)
    print('Downloading Whisper medium model...')
    try:
        whisper.load_model('medium', download_root=models_dir)
        print('Whisper medium model downloaded successfully')
    except Exception as e:
        print(f'Failed to download medium model: {e}')
        sys.exit(1)
    
    print('All models downloaded successfully')

if __name__ == '__main__':
    main()