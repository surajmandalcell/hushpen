"""
Performance testing module for ASR Pro Python Sidecar
"""

import asyncio
import time
import json
import sys
import os
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Any, Tuple
import logging

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from models import ModelManager
from config.settings import Settings
from tests.test_framework import ModelTester
from utils.reporting import print_perf_summary

logger = logging.getLogger(__name__)


class PerformanceTester:
    """Performance testing for ASR models."""

    def __init__(self):
        self.tester = ModelTester()
        self.audio_gen = self.tester.audio_gen
        self.results = {}

    async def benchmark_model(
        self,
        model_id: str,
        manager: ModelManager,
        test_cases: List[Dict[str, Any]],
        backend: str = "auto",
    ) -> Dict[str, Any]:
        """Benchmark a single model with various test cases."""
        print(f"Benchmarking {model_id}...")

        model_results = {"model_id": model_id, "test_cases": {}, "summary": {}}

        try:
            # Load the model
            load_start = time.time()
            success = await manager.set_model(model_id)
            load_time = time.time() - load_start

            if not success:
                model_results["error"] = "Failed to load model"
                return model_results

            model_results["load_time"] = load_time

            # Get backend information
            current_device = manager.get_current_device()
            device_info = manager.device_detector.get_device_info()
            model_results["backend"] = {
                "device": current_device,
                "device_name": device_info.get("device_name", "Unknown"),
                "cuda_available": device_info.get("cuda_available", False),
                "mps_available": device_info.get("mps_available", False),
                "vulkan_available": device_info.get("vulkan_available", False),
                "compute_type": device_info.get("compute_type", "float32"),
            }

            # Run test cases
            for test_case in test_cases:
                case_name = test_case["name"]
                audio_duration = test_case["duration"]
                audio_type = test_case["type"]

                print(f"  Testing {case_name} ({audio_duration:.2f}s {audio_type})...")

                # Handle different audio types
                if audio_type == "real_audio" and "file_path" in test_case:
                    # Use real audio file
                    temp_file = test_case["file_path"]
                    # Benchmark transcription using specific backend
                    transcribe_start = time.time()
                    try:
                        with open(temp_file, "rb") as audio_file:
                            # Get the current loader and use specific backend method
                            loader = manager.get_current_loader()
                            if hasattr(loader, f"transcribe_{backend}"):
                                result = await getattr(loader, f"transcribe_{backend}")(
                                    audio_file
                                )
                            else:
                                result = await manager.transcribe_file(
                                    audio_file, model_id
                                )
                        transcribe_time = time.time() - transcribe_start

                        # Calculate metrics
                        text_length = len(result.get("text", ""))
                        words_per_second = (
                            text_length / transcribe_time if transcribe_time > 0 else 0
                        )
                        real_time_factor = (
                            audio_duration / transcribe_time
                            if transcribe_time > 0
                            else 0
                        )

                        model_results["test_cases"][case_name] = {
                            "duration": audio_duration,
                            "type": audio_type,
                            "transcribe_time": transcribe_time,
                            "text_length": text_length,
                            "words_per_second": words_per_second,
                            "real_time_factor": real_time_factor,
                            "success": True,
                            "result": result,
                        }

                    except Exception as e:
                        transcribe_time = time.time() - transcribe_start
                        model_results["test_cases"][case_name] = {
                            "duration": audio_duration,
                            "type": audio_type,
                            "transcribe_time": transcribe_time,
                            "success": False,
                            "error": str(e),
                        }
                else:
                    # Generate test audio
                    if audio_type == "silence":
                        audio_data = self.audio_gen.create_silence(audio_duration)
                    elif audio_type == "tone":
                        audio_data = self.audio_gen.create_tone(audio_duration, 440)
                    elif audio_type == "noise":
                        audio_data = self.audio_gen.create_noise(audio_duration)
                    else:
                        audio_data = self.audio_gen.create_silence(audio_duration)

                    # Create temporary file
                    with self.audio_gen.create_temp_file(audio_data) as temp_file:
                        # Benchmark transcription using specific backend
                        transcribe_start = time.time()
                        try:
                            with open(temp_file, "rb") as audio_file:
                                # Get the current loader and use specific backend method
                                loader = manager.get_current_loader()
                                if hasattr(loader, f"transcribe_{backend}"):
                                    result = await getattr(
                                        loader, f"transcribe_{backend}"
                                    )(audio_file)
                                else:
                                    result = await manager.transcribe_file(
                                        audio_file, model_id
                                    )
                            transcribe_time = time.time() - transcribe_start

                            # Calculate metrics
                            text_length = len(result.get("text", ""))
                            words_per_second = (
                                text_length / transcribe_time
                                if transcribe_time > 0
                                else 0
                            )
                            real_time_factor = (
                                audio_duration / transcribe_time
                                if transcribe_time > 0
                                else 0
                            )

                            model_results["test_cases"][case_name] = {
                                "duration": audio_duration,
                                "type": audio_type,
                                "transcribe_time": transcribe_time,
                                "text_length": text_length,
                                "words_per_second": words_per_second,
                                "real_time_factor": real_time_factor,
                                "success": True,
                                "result": result,
                            }

                        except Exception as e:
                            transcribe_time = time.time() - transcribe_start
                            model_results["test_cases"][case_name] = {
                                "duration": audio_duration,
                                "type": audio_type,
                                "transcribe_time": transcribe_time,
                                "success": False,
                                "error": str(e),
                            }

            # Calculate summary statistics
            successful_cases = [
                case
                for case in model_results["test_cases"].values()
                if case.get("success", False)
            ]
            if successful_cases:
                transcribe_times = [
                    case["transcribe_time"] for case in successful_cases
                ]
                real_time_factors = [
                    case["real_time_factor"] for case in successful_cases
                ]
                words_per_second = [
                    case["words_per_second"] for case in successful_cases
                ]

                model_results["summary"] = {
                    "total_tests": len(test_cases),
                    "successful_tests": len(successful_cases),
                    "success_rate": len(successful_cases) / len(test_cases),
                    "avg_transcribe_time": sum(transcribe_times)
                    / len(transcribe_times),
                    "min_transcribe_time": min(transcribe_times),
                    "max_transcribe_time": max(transcribe_times),
                    "avg_real_time_factor": sum(real_time_factors)
                    / len(real_time_factors),
                    "min_real_time_factor": min(real_time_factors),
                    "max_real_time_factor": max(real_time_factors),
                    "avg_words_per_second": sum(words_per_second)
                    / len(words_per_second),
                    "min_words_per_second": min(words_per_second),
                    "max_words_per_second": max(words_per_second),
                }

        except Exception as e:
            model_results["error"] = str(e)

        return model_results

    def generate_test_cases(self) -> List[Dict[str, Any]]:
        """Generate comprehensive test cases for benchmarking."""
        test_cases = []

        # Use real audio file for testing
        audio_file = Path(__file__).parent / "audio_for_test.mp3"
        if audio_file.exists():
            # Get audio duration using librosa or similar
            try:
                import librosa

                duration = librosa.get_duration(path=str(audio_file))
                test_cases.append(
                    {
                        "name": "real_audio",
                        "duration": duration,
                        "type": "real_audio",
                        "file_path": str(audio_file),
                    }
                )
            except ImportError:
                # Fallback: estimate duration or use a default
                test_cases.append(
                    {
                        "name": "real_audio",
                        "duration": 10.0,  # Default estimate
                        "type": "real_audio",
                        "file_path": str(audio_file),
                    }
                )
        else:
            # Fallback to generated audio if real file not found
            durations = [1, 5, 10]  # seconds
            audio_types = ["silence", "tone", "noise"]

            for duration in durations:
                for audio_type in audio_types:
                    test_cases.append(
                        {
                            "name": f"{audio_type}_{duration}s",
                            "duration": duration,
                            "type": audio_type,
                        }
                    )

        return test_cases

    async def run_comprehensive_benchmark(self) -> Dict[str, Any]:
        """Run comprehensive performance benchmark."""
        print("=" * 80)
        print("ASR PRO PERFORMANCE BENCHMARK")
        print("=" * 80)
        print(f"Started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        print()

        # Setup
        settings = Settings()
        manager = ModelManager(settings)
        await manager.initialize()

        # Get available models
        models = await manager.list_available_models()
        print(f"Testing {len(models)} models: {', '.join(models)}")
        print()

        # Generate test cases
        test_cases = self.generate_test_cases()
        audio_duration = test_cases[0]["duration"] if test_cases else 0
        print(f"Audio Duration: {audio_duration:.1f}s ({audio_duration/60:.1f}min)")
        print(f"Test Cases: {len(test_cases)} per model")
        print()

        # Run benchmarks for different backends
        benchmark_results = {}
        total_start_time = time.time()

        # Test each model on different backends
        backends_to_test = ["detected", "cpu", "mps", "cuda", "vulkan"]

        for model_id in models:
            print(f"Testing {model_id}...")

            for backend_name in backends_to_test:
                # Reset manager
                await manager.cleanup()
                await manager.initialize()

                if backend_name == "detected":
                    # Use detected backend from DeviceDetector
                    pass
                elif backend_name == "cpu":
                    # Force CPU
                    manager.device_detector.device_info["device"] = "cpu"
                    manager.device_detector.device_info["compute_type"] = "float32"
                    manager._initialize_loader_configs()
                elif backend_name == "mps":
                    # Force MPS
                    manager.device_detector.device_info["device"] = "mps"
                    manager.device_detector.device_info["compute_type"] = "float16"
                    manager.device_detector.device_info["mps_available"] = True
                    manager._initialize_loader_configs()
                elif backend_name == "cuda":
                    # Force CUDA
                    manager.device_detector.device_info["device"] = "cuda"
                    manager.device_detector.device_info["compute_type"] = "float16"
                    manager.device_detector.device_info["cuda_available"] = True
                    manager._initialize_loader_configs()
                elif backend_name == "vulkan":
                    # Force Vulkan (note: ORT may not have a provider; used for accounting)
                    manager.device_detector.device_info["device"] = "vulkan"
                    manager.device_detector.device_info["compute_type"] = "float16"
                    manager.device_detector.device_info["vulkan_available"] = True
                    manager._initialize_loader_configs()

                start_time = time.time()
                result = await self.benchmark_model(model_id, manager, test_cases)
                duration = time.time() - start_time
                result["total_benchmark_time"] = duration
                result["backend_type"] = backend_name

                actual_backend = result.get("backend", {}).get("device", "unknown")
                benchmark_results[f"{model_id}_{backend_name}"] = result
                print(
                    f"  {model_id} ({backend_name}->{actual_backend}): {duration:.2f}s"
                )

        total_duration = time.time() - total_start_time
        await manager.cleanup()

        # Generate summary
        summary = {
            "timestamp": datetime.now().isoformat(),
            "audio_duration": audio_duration,
            "total_models": len(models),
            "total_test_cases": len(test_cases),
            "total_duration": total_duration,
            "models": benchmark_results,
        }

        return summary

    def generate_markdown_report(self, results: Dict[str, Any]) -> str:
        """Generate minimal cramped performance report."""
        timestamp = datetime.fromisoformat(results["timestamp"])
        audio_duration = results.get("audio_duration", 0)

        report = f"""# ASR Pro Performance Report

**Audio:** {audio_duration:.1f}s ({audio_duration/60:.1f}min) | **Generated:** {timestamp.strftime('%H:%M:%S')} | **Duration:** {results['total_duration']:.1f}s

## Results

| Model | Backend | Load | Transcribe | RTF | WPS | Status |
|-------|---------|------|------------|-----|-----|--------|"""

        # Add model rows
        for model_id, model_data in results["models"].items():
            if "error" in model_data:
                report += f"\n| {model_id} | ERROR | - | - | - | - | FAIL |"
                continue

            summary = model_data.get("summary", {})
            backend_info = model_data.get("backend", {})
            backend = backend_info.get("device", "unknown")
            load_time = model_data.get("load_time", 0)
            transcribe_time = summary.get("avg_transcribe_time", 0)
            rtf = summary.get("avg_real_time_factor", 0)
            wps = summary.get("avg_words_per_second", 0)
            success = summary.get("success_rate", 0) > 0
            status = "PASS" if success else "FAIL"

            # Clean up model name (remove backend suffix)
            clean_model_id = (
                model_id.replace("_detected", "")
                .replace("_cpu", "")
                .replace("_cuda", "")
            )
            backend_type = model_id.split("_")[-1] if "_" in model_id else "detected"

            report += f"\n| {clean_model_id} | {backend_type}->{backend} | {load_time:.1f}s | {transcribe_time:.1f}s | {rtf:.1f}x | {wps:.0f} | {status} |"

        # Backend comparison
        report += f"\n\n## Backend Comparison\n"

        # Group by backend
        backend_stats = {}
        for model_id, model_data in results["models"].items():
            if "error" not in model_data and "backend" in model_data:
                backend = model_data["backend"]["device"]
                if backend not in backend_stats:
                    backend_stats[backend] = {
                        "load_times": [],
                        "transcribe_times": [],
                        "rtfs": [],
                    }

                backend_stats[backend]["load_times"].append(
                    model_data.get("load_time", 0)
                )
                summary = model_data.get("summary", {})
                backend_stats[backend]["transcribe_times"].append(
                    summary.get("avg_transcribe_time", 0)
                )
                backend_stats[backend]["rtfs"].append(
                    summary.get("avg_real_time_factor", 0)
                )

        for backend, stats in backend_stats.items():
            avg_load = sum(stats["load_times"]) / len(stats["load_times"])
            avg_transcribe = sum(stats["transcribe_times"]) / len(
                stats["transcribe_times"]
            )
            avg_rtf = sum(stats["rtfs"]) / len(stats["rtfs"])
            report += f"\n**{backend.upper()}:** {avg_load:.1f}s load, {avg_transcribe:.1f}s transcribe, {avg_rtf:.1f}x RTF"

        # Best performers
        successful_models = {
            k: v
            for k, v in results["models"].items()
            if "error" not in v and v.get("summary", {}).get("success_rate", 0) > 0
        }
        if successful_models:
            fastest = min(
                successful_models.items(),
                key=lambda x: x[1]
                .get("summary", {})
                .get("avg_transcribe_time", float("inf")),
            )
            most_efficient = max(
                successful_models.items(),
                key=lambda x: x[1].get("summary", {}).get("avg_real_time_factor", 0),
            )

            report += f"\n\n## Best\n"
            report += f"**Fastest:** {fastest[0]} ({fastest[1].get('summary', {}).get('avg_transcribe_time', 0):.1f}s)\n"
            report += f"**Most Efficient:** {most_efficient[0]} ({most_efficient[1].get('summary', {}).get('avg_real_time_factor', 0):.1f}x RTF)"

        return report


async def run_performance_test():
    """Run the comprehensive performance test."""
    # Setup test environment
    outputs_dir = Path("outputs")
    if outputs_dir.exists():
        import shutil

        shutil.rmtree(outputs_dir)
    outputs_dir.mkdir(exist_ok=True)

    # Run benchmark
    tester = PerformanceTester()
    results = await tester.run_comprehensive_benchmark()

    # Save JSON results
    json_file = outputs_dir / "performance_benchmark.json"
    with open(json_file, "w") as f:
        json.dump(results, f, indent=2)

    # Generate and save markdown report
    markdown_report = tester.generate_markdown_report(results)
    report_file = Path("PERFORMANCE_SUMMARY.md")
    with open(report_file, "w") as f:
        f.write(markdown_report)

    print_perf_summary(results)
    print(f"JSON results saved to: {json_file}")
    print(f"Markdown report saved to: {report_file}")

    return results


if __name__ == "__main__":
    asyncio.run(run_performance_test())
