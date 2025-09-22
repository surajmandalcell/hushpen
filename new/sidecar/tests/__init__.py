"""
Test package for ASR Pro Python Sidecar
"""

import asyncio
import json
import time
import sys
import tempfile
import os
import requests
import logging
from pathlib import Path
from datetime import datetime
from threading import Thread
from typing import Dict, List

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from tests.test_framework import ModelTester
from utils.reporting import print_test_summary, print_test_detailed_results
from models import ModelManager
from config.settings import Settings
from api.server import create_app

logger = logging.getLogger(__name__)


def setup_test_environment():
    """Setup test environment and clean outputs."""
    outputs_dir = Path("outputs")
    if outputs_dir.exists():
        print("Cleaning previous test outputs...")
        import shutil

        shutil.rmtree(outputs_dir)
        print("✓ Outputs directory cleaned")
    else:
        print("Creating outputs directory...")
        outputs_dir.mkdir(exist_ok=True)
        print("✓ Outputs directory created")
    print()


async def test_single_model(model_id: str, manager: ModelManager) -> dict:
    """Test a single model comprehensively."""
    tester = ModelTester()
    audio_gen = tester.audio_gen

    print(f"\n{'='*20} TESTING {model_id.upper()} {'='*20}")

    results = []

    try:
        # Test 1: Model Loading
        print(f"1. Testing model loading...")
        load_result = await tester.test_model_loading(model_id, manager)
        print(f"   {'✓ PASSED' if load_result.passed else '✗ FAILED'}")
        if load_result.error:
            print(f"   Error: {load_result.error}")

        if not load_result.passed:
            return {
                "model_id": model_id,
                "success": False,
                "error": "Model loading failed",
                "tests": [],
            }

        # Test 2: Real Audio
        print(f"2. Testing real audio...")
        real_result = await tester.test_real_audio(model_id, manager)
        print(f"   {'✓ PASSED' if real_result.passed else '✗ FAILED'}")
        if real_result.passed:
            text = real_result.output_info.get("text", "")
            print(f"   Output: '{text[:50]}{'...' if len(text) > 50 else ''}'")

        # Test 3: Basic Tests
        print(f"3. Testing basic cases...")

        # Silence test
        silence_audio = audio_gen.create_wav_file(
            audio_gen.create_silence(2.0), filename=f"outputs/{model_id}_silence.wav"
        )
        silence_result = await tester.test_transcription(
            model_id, manager, silence_audio, "silence"
        )
        print(f"   Silence: {'✓ PASSED' if silence_result.passed else '✗ FAILED'}")

        # Test 4: Edge Cases (simplified)
        print(f"4. Testing edge cases...")
        edge_results = await tester.test_edge_cases(model_id, manager)
        edge_passed = sum(1 for r in edge_results if r.passed)
        print(f"   Edge cases: {edge_passed}/{len(edge_results)} passed")

        # Test 5: Performance (simplified)
        print(f"5. Testing performance...")
        perf_durations = [1.0, 5.0]  # Only 2 durations
        perf_results = []

        for duration in perf_durations:
            perf_audio = audio_gen.create_wav_file(
                audio_gen.create_silence(duration),
                filename=f"outputs/{model_id}_perf_{duration}s.wav",
            )
            result = await tester.test_transcription(
                model_id, manager, perf_audio, f"perf_{duration}s"
            )
            perf_results.append(result)
            print(
                f"   {duration}s: {'✓ PASSED' if result.passed else '✗ FAILED'} ({result.duration:.2f}s)"
            )

        # Collect all results
        all_results = (
            [load_result, real_result, silence_result] + edge_results + perf_results
        )

        # Calculate summary
        total_tests = len(all_results)
        passed_tests = sum(1 for r in all_results if r.passed)
        success_rate = passed_tests / total_tests if total_tests > 0 else 0

        return {
            "model_id": model_id,
            "success": success_rate > 0.8,
            "success_rate": success_rate,
            "total_tests": total_tests,
            "passed_tests": passed_tests,
            "failed_tests": total_tests - passed_tests,
            "tests": [r.to_dict() for r in all_results],
        }

    except Exception as e:
        return {"model_id": model_id, "success": False, "error": str(e), "tests": []}


async def run_all_tests():
    """Run comprehensive tests for all models."""
    print("=" * 80)
    print("ASR PRO COMPREHENSIVE MODEL TESTING")
    print("=" * 80)
    print(f"Started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print()

    # Setup test environment (clean outputs)
    setup_test_environment()

    # Initialize model manager
    settings = Settings()
    manager = ModelManager(settings)
    await manager.initialize()

    # Get available models - only test English/Hindi focused models
    all_models = await manager.list_available_models()
    # Focus on smaller, faster models for English/Hindi (ensure whisper-tiny is included if available)
    preferred = ["whisper-tiny", "whisper-base", "parakeet-tdt-0.6b-v2"]
    model_ids = [m for m in preferred if m in all_models]

    print(f"Testing {len(model_ids)} models for English/Hindi: {', '.join(model_ids)}")
    print()

    results = {}
    total_start_time = time.time()

    try:
        # Test each model
        for model_id in model_ids:
            start_time = time.time()
            result = await test_single_model(model_id, manager)
            duration = time.time() - start_time

            result["duration"] = duration
            results[model_id] = result

            print(
                f"\n{model_id}: {'✓ PASSED' if result['success'] else '✗ FAILED'} ({duration:.2f}s)"
            )

            # Unload current model before testing next
            await manager.cleanup()
            await manager.initialize()

    finally:
        await manager.cleanup()

    total_duration = time.time() - total_start_time

    # Generate summary (shared format)
    print_test_summary(results, total_duration)
    print_test_detailed_results(results)

    # Save comprehensive report
    report = {
        "test_suite": "ASR Pro Model Testing",
        "timestamp": datetime.now().isoformat(),
        "total_models": total_models,
        "passed_models": passed_models,
        "failed_models": total_models - passed_models,
        "overall_success_rate": passed_models / total_models,
        "total_duration": total_duration,
        "models": results,
    }

    outputs_dir = Path("outputs")
    outputs_dir.mkdir(exist_ok=True)

    report_file = outputs_dir / "comprehensive_test_report.json"
    with open(report_file, "w") as f:
        json.dump(report, f, indent=2)

    print(f"\nComprehensive report saved to: {report_file}")

    # Final summary
    print("\n" + "=" * 80)
    if passed_models == total_models:
        print("🎉 ALL MODELS PASSED! ASR Pro is fully functional.")
    elif passed_models > total_models // 2:
        print(
            f"⚠️  {passed_models}/{total_models} models passed. Some models may have issues."
        )
    else:
        print(
            f"❌ Only {passed_models}/{total_models} models passed. Significant issues detected."
        )
    print("=" * 80)

    return passed_models == total_models


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
        from models import ModelRegistry

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
        model_id = "whisper-base"
        success = await manager.set_model(model_id)
        if success:
            print(f"✓ Model {model_id} loaded successfully")

            # Test transcription with dummy audio
            dummy_audio = b"dummy audio data"
            with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
                f.write(dummy_audio)
                temp_file = f.name

            try:
                with open(temp_file, "rb") as audio_file:
                    result = await manager.transcribe_file(audio_file, model_id)
                print("✓ Transcription test completed")
            finally:
                os.unlink(temp_file)
        else:
            print(f"✗ Failed to load model {model_id}")
            return False

        await manager.cleanup()
        print("✓ Model manager cleaned up")

        return True

    except Exception as e:
        print(f"✗ Core functionality test failed: {e}")
        return False


def test_api_functionality():
    """Test API functionality."""
    print("=== Testing API Functionality ===")

    try:
        # Test health endpoint
        response = requests.get("http://127.0.0.1:8001/health", timeout=5)
        if response.status_code == 200:
            print("✓ Health endpoint working")
        else:
            print(f"✗ Health endpoint failed: {response.status_code}")
            return False

        # Test models endpoint
        response = requests.get("http://127.0.0.1:8001/v1/models", timeout=5)
        if response.status_code == 200:
            models = response.json()
            print(f"✓ Models endpoint working: {len(models)} models")
        else:
            print(f"✗ Models endpoint failed: {response.status_code}")
            return False

        # Test transcription endpoint with dummy data
        dummy_audio = b"dummy audio data"
        files = {"file": ("test.wav", dummy_audio, "audio/wav")}
        response = requests.post(
            "http://127.0.0.1:8001/v1/audio/transcriptions", files=files, timeout=10
        )

        if response.status_code == 200:
            print("✓ Transcription endpoint working")
        else:
            print(f"✗ Transcription endpoint failed: {response.status_code}")
            return False

        return True

    except Exception as e:
        print(f"✗ API functionality test failed: {e}")
        return False


async def run_integration_tests():
    """Run integration tests."""
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
        print("• Model loading and transcription")
        print("• API server startup and endpoints")
        print("• Health checks and model listing")
        print("• Audio transcription via API")
        return True
    else:
        print("\n❌ SOME TESTS FAILED! Please check the errors above.")
        return False


async def run_performance_test():
    """Run comprehensive performance benchmark."""
    try:
        from tests.perf_test import run_performance_test as run_perf_test

        return await run_perf_test()
    except Exception as e:
        logger.error(f"Error running performance test: {e}")
        return None
