#!/usr/bin/env python3
"""
End-to-end test runner for Docker-based ASR system
"""

import argparse
import asyncio
import json
import logging
import os
import sys
import time
from pathlib import Path
from typing import Dict, Any, Optional

# Add sidecar directory to path
sys.path.insert(0, str(Path(__file__).parent.parent / "sidecar"))

from tests.integration_helper import IntegrationTestHelper
from tests.test_e2e_docker_asr import run_all_tests

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[
        logging.FileHandler("test_e2e.log"),
        logging.StreamHandler()
    ]
)
logger = logging.getLogger(__name__)


class E2ETestRunner:
    """End-to-end test runner for Docker-based ASR system."""
    
    def __init__(self, config_file: Optional[str] = None):
        """
        Initialize test runner.
        
        Args:
            config_file: Path to test configuration file
        """
        self.config_file = config_file
        self.test_helper = None
        self.test_results = None
        self.start_time = None
        self.end_time = None
    
    async def setup(self):
        """Set up test environment."""
        logger.info("Setting up E2E test runner")
        
        # Create test helper
        self.test_helper = IntegrationTestHelper(self.config_file)
        
        # Set up test environment
        self.test_helper.setup_test_environment()
        
        # Start server
        if not self.test_helper.start_server():
            logger.error("Failed to start ASR server")
            return False
        
        logger.info("E2E test runner setup complete")
        return True
    
    async def run_tests(self) -> Dict[str, Any]:
        """Run all tests."""
        logger.info("Running E2E tests")
        self.start_time = time.time()
        
        try:
            # Run all tests
            self.test_results = await run_all_tests(self.test_helper)
            
            # Add timing information
            self.end_time = time.time()
            self.test_results["timing"] = {
                "start_time": self.start_time,
                "end_time": self.end_time,
                "duration": self.end_time - self.start_time
            }
            
            # Generate report
            report = self.test_helper.create_test_report(self.test_results["results"])
            self.test_results["report"] = json.loads(report)
            
            logger.info("E2E tests completed")
            return self.test_results
            
        except Exception as e:
            logger.error(f"Error running tests: {e}")
            return {
                "error": str(e),
                "timing": {
                    "start_time": self.start_time,
                    "end_time": time.time(),
                    "duration": time.time() - self.start_time if self.start_time else 0
                }
            }
    
    async def cleanup(self):
        """Clean up test environment."""
        logger.info("Cleaning up E2E test runner")
        
        if self.test_helper:
            self.test_helper.cleanup_test_environment()
        
        logger.info("E2E test runner cleanup complete")
    
    def save_results(self, output_file: str = "e2e_test_results.json"):
        """Save test results to file."""
        if not self.test_results:
            logger.warning("No test results to save")
            return
        
        try:
            with open(output_file, 'w') as f:
                json.dump(self.test_results, f, indent=2)
            logger.info(f"Test results saved to {output_file}")
        except Exception as e:
            logger.error(f"Failed to save test results: {e}")
    
    def print_summary(self):
        """Print test summary."""
        if not self.test_results:
            logger.warning("No test results to summarize")
            return
        
        summary = self.test_results.get("summary", {})
        timing = self.test_results.get("timing", {})
        
        print("\n" + "="*60)
        print("E2E TEST SUMMARY")
        print("="*60)
        print(f"Total Tests: {summary.get('total_tests', 0)}")
        print(f"Passed: {summary.get('passed', 0)}")
        print(f"Failed: {summary.get('failed', 0)}")
        print(f"Skipped: {summary.get('skipped', 0)}")
        print(f"Success Rate: {summary.get('success_rate', 0):.1f}%")
        print(f"Duration: {timing.get('duration', 0):.2f} seconds")
        print("="*60)
        
        # Print failed tests
        results = self.test_results.get("results", {})
        failed_tests = [name for name, result in results.items() if result.get("status") == "failed"]
        
        if failed_tests:
            print("\nFAILED TESTS:")
            print("-"*40)
            for test_name in failed_tests:
                error = results[test_name].get("error", "Unknown error")
                print(f"  {test_name}: {error}")
        
        # Print skipped tests
        skipped_tests = [name for name, result in results.items() if result.get("status") == "skipped"]
        
        if skipped_tests:
            print("\nSKIPPED TESTS:")
            print("-"*40)
            for test_name in skipped_tests:
                reason = results[test_name].get("reason", "Unknown reason")
                print(f"  {test_name}: {reason}")
        
        print("\n" + "="*60)
    
    async def run(self, output_file: str = "e2e_test_results.json"):
        """Run the complete test suite."""
        try:
            # Setup
            if not await self.setup():
                return False
            
            # Run tests
            results = await self.run_tests()
            
            # Save results
            self.save_results(output_file)
            
            # Print summary
            self.print_summary()
            
            # Cleanup
            await self.cleanup()
            
            # Return success based on test results
            summary = results.get("summary", {})
            return summary.get("failed", 0) == 0
            
        except Exception as e:
            logger.error(f"Error in test runner: {e}")
            await self.cleanup()
            return False


async def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(description="Run E2E tests for Docker-based ASR system")
    parser.add_argument(
        "--config",
        type=str,
        help="Path to test configuration file",
        default=None
    )
    parser.add_argument(
        "--output",
        type=str,
        help="Output file for test results",
        default="e2e_test_results.json"
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable verbose logging"
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress console output"
    )
    
    args = parser.parse_args()
    
    # Configure logging level
    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    elif args.quiet:
        logging.getLogger().setLevel(logging.WARNING)
    
    # Create and run test runner
    runner = E2ETestRunner(args.config)
    success = await runner.run(args.output)
    
    # Exit with appropriate code
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    asyncio.run(main())