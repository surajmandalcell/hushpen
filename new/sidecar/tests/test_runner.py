#!/usr/bin/env python3
"""
Consolidated test runner for all ASR Pro models
"""

import asyncio
import json
import time
import sys
from pathlib import Path
from datetime import datetime

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from tests.test_framework import ModelTester
from models.manager import ModelManager
from config.settings import Settings


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

    # Clean outputs directory before running tests
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

    # Initialize model manager
    settings = Settings()
    manager = ModelManager(settings)
    await manager.initialize()

    # Get available models - only test English/Hindi focused models
    all_models = await manager.list_available_models()
    # Focus on smaller, faster models for English/Hindi
    model_ids = ["whisper-tiny", "whisper-base", "whisper-small"]

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

    # Generate summary
    print("\n" + "=" * 80)
    print("TEST RESULTS SUMMARY")
    print("=" * 80)

    passed_models = sum(1 for r in results.values() if r["success"])
    total_models = len(results)

    print(f"Total models tested: {total_models}")
    print(f"Models passed: {passed_models}")
    print(f"Models failed: {total_models - passed_models}")
    print(f"Overall success rate: {passed_models/total_models:.1%}")
    print(f"Total duration: {total_duration:.2f}s")
    print()

    # Detailed results
    print("DETAILED RESULTS:")
    print("-" * 80)
    print(
        f"{'Model':<20} {'Status':<10} {'Success Rate':<12} {'Duration':<10} {'Tests'}"
    )
    print("-" * 80)

    for model_id, result in results.items():
        status = "✓ PASSED" if result["success"] else "✗ FAILED"
        success_rate = f"{result.get('success_rate', 0):.1%}"
        duration = f"{result['duration']:.2f}s"
        tests = f"{result.get('passed_tests', 0)}/{result.get('total_tests', 0)}"
        print(f"{model_id:<20} {status:<10} {success_rate:<12} {duration:<10} {tests}")

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


if __name__ == "__main__":
    success = asyncio.run(run_all_tests())
    exit(0 if success else 1)
