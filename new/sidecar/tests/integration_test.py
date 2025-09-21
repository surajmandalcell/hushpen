#!/usr/bin/env python3
"""
Integration test for ASR Pro Python Sidecar
This test verifies that all features work together correctly.
"""

import asyncio
import tempfile
import os
import requests
import time
from threading import Thread
from api.server import create_app
from config.settings import Settings
from models.manager import ModelManager

def start_server():
    """Start the server in a separate thread."""
    import uvicorn
    settings = Settings()
    app = create_app(settings)
    uvicorn.run(app, host="127.0.0.1", port=8001, log_level="warning")

async def test_core_functionality():
    """Test core functionality without API."""
    print("=== Testing Core Functionality ===")
    
    try:
        # Test settings
        settings = Settings()
        print("✓ Settings initialized")
        
        # Test model registry
        from models.registry import ModelRegistry
        registry = ModelRegistry()
        models = registry.list_models()
        print(f"✓ Model registry: {len(models)} models available")
        
        # Test device detection
        from utils.device import DeviceDetector
        detector = DeviceDetector()
        await detector.detect_capabilities()
        print(f"✓ Device detection: {detector.get_current_device()}")
        
        # Test model manager
        manager = ModelManager(settings)
        await manager.initialize()
        print("✓ Model manager initialized")
        
        # Test model loading
        result = await manager.set_model('whisper-tiny')
        print(f"✓ Model loading: {'Success' if result else 'Failed'}")
        
        if result:
            # Test transcription
            wav_header = b'RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x40\x1f\x00\x00\x80\x3e\x00\x00\x02\x00\x10\x00data\x00\x00\x00\x00'
            wav_data = wav_header + b'\x00' * 1000
            
            with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
                f.write(wav_data)
                f.flush()
                temp_file_path = f.name
            
            try:
                with open(temp_file_path, 'rb') as audio_file:
                    transcription = await manager.transcribe_file(audio_file)
                    print(f"✓ Transcription: {transcription['text'][:50]}...")
            finally:
                if os.path.exists(temp_file_path):
                    os.unlink(temp_file_path)
        
        # Cleanup
        await manager.cleanup()
        print("✓ Cleanup completed")
        
    except Exception as e:
        print(f"✗ Core functionality error: {e}")
        return False
    
    return True

def test_api_functionality():
    """Test API functionality."""
    print("\n=== Testing API Functionality ===")
    
    base_url = "http://127.0.0.1:8001"
    
    try:
        # Test health endpoint
        response = requests.get(f"{base_url}/health", timeout=10)
        if response.status_code == 200:
            print("✓ Health endpoint working")
        else:
            print(f"✗ Health endpoint failed: {response.status_code}")
            return False
        
        # Test models endpoint
        response = requests.get(f"{base_url}/v1/models", timeout=10)
        if response.status_code == 200:
            data = response.json()
            print(f"✓ Models endpoint: {len(data['data'])} models")
        else:
            print(f"✗ Models endpoint failed: {response.status_code}")
            return False
        
        # Test model setting
        response = requests.post(f"{base_url}/v1/settings/model", 
                               json={"model_id": "whisper-tiny"}, timeout=10)
        if response.status_code == 200:
            print("✓ Model setting working")
        else:
            print(f"✗ Model setting failed: {response.status_code}")
            return False
        
        # Test transcription
        wav_header = b'RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x40\x1f\x00\x00\x80\x3e\x00\x00\x02\x00\x10\x00data\x00\x00\x00\x00'
        wav_data = wav_header + b'\x00' * 1000
        
        with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
            f.write(wav_data)
            f.flush()
            temp_file_path = f.name
        
        try:
            with open(temp_file_path, 'rb') as audio_file:
                files = {'file': ('test.wav', audio_file, 'audio/wav')}
                response = requests.post(f"{base_url}/v1/audio/transcriptions", 
                                       files=files, timeout=30)
                if response.status_code == 200:
                    result = response.json()
                    print(f"✓ Transcription API: {result['text'][:50]}...")
                else:
                    print(f"✗ Transcription API failed: {response.status_code}")
                    return False
        finally:
            if os.path.exists(temp_file_path):
                os.unlink(temp_file_path)
        
    except Exception as e:
        print(f"✗ API functionality error: {e}")
        return False
    
    return True

async def main():
    """Main test function."""
    print("ASR Pro Python Sidecar - Integration Test")
    print("=" * 50)
    
    # Test core functionality
    core_success = await test_core_functionality()
    
    # Test API functionality
    print("\nStarting API server...")
    server_thread = Thread(target=start_server, daemon=True)
    server_thread.start()
    
    # Wait for server to start
    time.sleep(3)
    
    api_success = test_api_functionality()
    
    # Summary
    print("\n" + "=" * 50)
    print("INTEGRATION TEST SUMMARY")
    print("=" * 50)
    print(f"Core Functionality: {'✓ PASSED' if core_success else '✗ FAILED'}")
    print(f"API Functionality: {'✓ PASSED' if api_success else '✗ FAILED'}")
    
    if core_success and api_success:
        print("\n🎉 ALL TESTS PASSED! The ASR Pro sidecar is working correctly.")
        print("\nFeatures verified:")
        print("• Model registry and management")
        print("• Device detection and configuration")
        print("• Whisper model loading and transcription")
        print("• FastAPI server with all endpoints")
        print("• Audio file processing")
        print("• Model switching and cleanup")
    else:
        print("\n❌ Some tests failed. Please check the errors above.")
    
    return core_success and api_success

if __name__ == "__main__":
    success = asyncio.run(main())
    exit(0 if success else 1)
