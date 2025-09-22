"""
Shared reporting utilities for tests and performance benchmarks (DRY).
"""

from typing import Dict, Any


def print_test_summary(
    results: Dict[str, Dict[str, Any]], total_duration: float
) -> None:
    print("\n" + "=" * 80)
    print("TEST RESULTS SUMMARY")
    print("=" * 80)

    passed_models = sum(1 for r in results.values() if r.get("success"))
    total_models = len(results)

    overall_rate = (passed_models / total_models) if total_models else 0

    print(f"Total models tested: {total_models}")
    print(f"Models passed: {passed_models}")
    print(f"Models failed: {total_models - passed_models}")
    print(f"Overall success rate: {overall_rate:.1%}")
    print(f"Total duration: {total_duration:.2f}s")
    print()


def print_test_detailed_results(results: Dict[str, Dict[str, Any]]) -> None:
    print("DETAILED RESULTS:")
    print("-" * 80)
    print(
        f"{'Model':<20} {'Status':<10} {'Success Rate':<12} {'Duration':<10} {'Tests'}"
    )
    print("-" * 80)

    for model_id, result in results.items():
        status = "✓ PASSED" if result.get("success") else "✗ FAILED"
        success_rate = f"{result.get('success_rate', 0):.1%}"
        duration = f"{result.get('duration', 0):.2f}s"
        tests = f"{result.get('passed_tests', 0)}/{result.get('total_tests', 0)}"
        print(f"{model_id:<20} {status:<10} {success_rate:<12} {duration:<10} {tests}")


def print_perf_summary(results: Dict[str, Any]) -> None:
    print("\n" + "=" * 80)
    print("PERFORMANCE RESULTS SUMMARY")
    print("=" * 80)
    total_models = results.get("total_models", 0)
    print(f"Total models benchmarked: {total_models}")
    print(f"Total duration: {results.get('total_duration', 0):.2f}s")
    print()
    print("RESULTS (per model/backend):")
    print("-" * 80)
    for key, data in results.get("models", {}).items():
        status = (
            "PASS" if data.get("summary", {}).get("success_rate", 0) > 0 else "FAIL"
        )
        actual_backend = data.get("backend", {}).get("device", "unknown")
        requested_backend = data.get("backend_type", "detected")
        avg_t = data.get("summary", {}).get("avg_transcribe_time", 0)
        fallback = (
            " (fallback)"
            if requested_backend != actual_backend and status != "FAIL"
            else ""
        )
        label = f"{requested_backend}->{actual_backend}{fallback}"
        print(f"{key:<32} {label:<12} avg {avg_t:.2f}s -> {status}")
    print("-" * 80)
