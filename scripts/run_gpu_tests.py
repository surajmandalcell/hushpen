#!/usr/bin/env python3
"""
GPU Test Runner for ASRPro
Runs all GPU-related tests and generates a comprehensive report
"""

import os
import sys
import time
import json
import logging
import subprocess
import argparse
from pathlib import Path
from typing import Dict, Any, List, Optional, Tuple
from datetime import datetime

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


class TestRunner:
    """Main test runner for GPU tests."""
    
    def __init__(self, output_dir: str = "test-reports"):
        """
        Initialize the test runner.
        
        Args:
            output_dir: Directory to save test reports
        """
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(exist_ok=True)
        self.test_results = {}
        self.start_time = None
        self.end_time = None
        
    def check_prerequisites(self) -> Dict[str, bool]:
        """
        Check if all prerequisites are met.
        
        Returns:
            Dictionary with prerequisite check results
        """
        logger.info("Checking prerequisites...")
        
        results = {
            "python": False,
            "docker": False,
            "nvidia_drivers": False,
            "nvidia_docker": False,
            "cuda": False
        }
        
        # Check Python
        try:
            import sys
            python_version = sys.version_info
            if python_version.major >= 3 and python_version.minor >= 8:
                results["python"] = True
                logger.info(f"Python version: {python_version.major}.{python_version.minor}.{python_version.micro}")
            else:
                logger.error(f"Python version too old: {python_version.major}.{python_version.minor}.{python_version.micro}")
        except Exception as e:
            logger.error(f"Python check failed: {e}")
        
        # Check Docker
        try:
            result = subprocess.run(
                ["docker", "--version"],
                capture_output=True,
                text=True,
                timeout=10
            )
            if result.returncode == 0:
                results["docker"] = True
                logger.info(f"Docker available: {result.stdout.strip()}")
            else:
                logger.error("Docker not available")
        except Exception as e:
            logger.error(f"Docker check failed: {e}")
        
        # Check NVIDIA drivers
        try:
            result = subprocess.run(
                ["nvidia-smi", "--version"],
                capture_output=True,
                text=True,
                timeout=10
            )
            if result.returncode == 0:
                results["nvidia_drivers"] = True
                logger.info(f"NVIDIA drivers available: {result.stdout.strip()}")
            else:
                logger.error("NVIDIA drivers not available")
        except Exception as e:
            logger.error(f"NVIDIA drivers check failed: {e}")
        
        # Check NVIDIA Docker support
        if results["docker"] and results["nvidia_drivers"]:
            try:
                result = subprocess.run(
                    ["docker", "run", "--rm", "--gpus", "all", "nvidia/cuda:12.1.0-base-ubuntu22.04", 
                     "nvidia-smi", "--query-gpu=name", "--format=csv,noheader,nounits"],
                    capture_output=True,
                    text=True,
                    timeout=30
                )
                if result.returncode == 0:
                    results["nvidia_docker"] = True
                    logger.info(f"NVIDIA Docker support available, GPU: {result.stdout.strip()}")
                else:
                    logger.error("NVIDIA Docker support not available")
            except Exception as e:
                logger.error(f"NVIDIA Docker check failed: {e}")
        
        # Check CUDA
        if results["nvidia_drivers"]:
            try:
                result = subprocess.run(
                    ["nvidia-smi", "--query-gpu=cuda_version", "--format=csv,noheader,nounits"],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                if result.returncode == 0:
                    cuda_version = result.stdout.strip()
                    major, minor = map(int, cuda_version.split("."))
                    if major >= 11:
                        results["cuda"] = True
                        logger.info(f"CUDA version: {cuda_version}")
                    else:
                        logger.error(f"CUDA version too old: {cuda_version}")
                else:
                    logger.error("CUDA version check failed")
            except Exception as e:
                logger.error(f"CUDA check failed: {e}")
        
        return results
    
    def run_gpu_utilization_tests(self) -> Dict[str, Any]:
        """
        Run GPU utilization tests.
        
        Returns:
            Dictionary with test results
        """
        logger.info("Running GPU utilization tests...")
        
        # Path to test file
        test_file = Path(__file__).parent.parent / "sidecar" / "tests" / "test_gpu_utilization.py"
        
        if not test_file.exists():
            logger.error(f"GPU utilization test file not found: {test_file}")
            return {"status": "error", "message": "Test file not found"}
        
        try:
            # Run pytest
            cmd = [
                sys.executable, "-m", "pytest",
                str(test_file),
                "-v",
                "--json-report",
                f"--json-report-file={self.output_dir / 'gpu_utilization_results.json'}"
            ]
            
            logger.info(f"Running: {' '.join(cmd)}")
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=300
            )
            
            # Parse results
            success = result.returncode == 0
            output = result.stdout
            errors = result.stderr
            
            # Try to load JSON report
            json_report = None
            json_file = self.output_dir / 'gpu_utilization_results.json'
            if json_file.exists():
                try:
                    with open(json_file, 'r') as f:
                        json_report = json.load(f)
                except Exception as e:
                    logger.warning(f"Failed to parse JSON report: {e}")
            
            return {
                "status": "passed" if success else "failed",
                "output": output,
                "errors": errors,
                "json_report": json_report
            }
            
        except subprocess.TimeoutExpired:
            logger.error("GPU utilization tests timed out")
            return {"status": "error", "message": "Tests timed out"}
        except Exception as e:
            logger.error(f"GPU utilization tests failed: {e}")
            return {"status": "error", "message": str(e)}
    
    def run_whisper_container_tests(self) -> Dict[str, Any]:
        """
        Run Whisper container tests.
        
        Returns:
            Dictionary with test results
        """
        logger.info("Running Whisper container tests...")
        
        # Path to test file
        test_file = Path(__file__).parent / "test_whisper_container.py"
        
        if not test_file.exists():
            logger.error(f"Whisper container test file not found: {test_file}")
            return {"status": "error", "message": "Test file not found"}
        
        try:
            # Run test script
            cmd = [sys.executable, str(test_file)]
            
            logger.info(f"Running: {' '.join(cmd)}")
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=600  # 10 minutes timeout
            )
            
            # Parse results
            success = result.returncode == 0
            output = result.stdout
            errors = result.stderr
            
            return {
                "status": "passed" if success else "failed",
                "output": output,
                "errors": errors,
                "return_code": result.returncode
            }
            
        except subprocess.TimeoutExpired:
            logger.error("Whisper container tests timed out")
            return {"status": "error", "message": "Tests timed out"}
        except Exception as e:
            logger.error(f"Whisper container tests failed: {e}")
            return {"status": "error", "message": str(e)}
    
    def run_docker_compose_tests(self) -> Dict[str, Any]:
        """
        Run Docker Compose tests.
        
        Returns:
            Dictionary with test results
        """
        logger.info("Running Docker Compose tests...")
        
        # Path to docker-compose file
        compose_file = Path(__file__).parent.parent / "docker-compose.yml"
        
        if not compose_file.exists():
            logger.error(f"Docker Compose file not found: {compose_file}")
            return {"status": "error", "message": "Docker Compose file not found"}
        
        try:
            # Change to project directory
            project_dir = Path(__file__).parent.parent
            
            # Build containers
            logger.info("Building containers with Docker Compose...")
            build_result = subprocess.run(
                ["docker-compose", "build"],
                cwd=project_dir,
                capture_output=True,
                text=True,
                timeout=600
            )
            
            if build_result.returncode != 0:
                return {
                    "status": "failed",
                    "message": "Docker Compose build failed",
                    "output": build_result.stdout,
                    "errors": build_result.stderr
                }
            
            # Start containers
            logger.info("Starting containers with Docker Compose...")
            up_result = subprocess.run(
                ["docker-compose", "up", "-d"],
                cwd=project_dir,
                capture_output=True,
                text=True,
                timeout=60
            )
            
            if up_result.returncode != 0:
                return {
                    "status": "failed",
                    "message": "Docker Compose up failed",
                    "output": up_result.stdout,
                    "errors": up_result.stderr
                }
            
            # Wait for containers to be ready
            logger.info("Waiting for containers to be ready...")
            time.sleep(30)
            
            # Check container status
            logger.info("Checking container status...")
            ps_result = subprocess.run(
                ["docker-compose", "ps"],
                cwd=project_dir,
                capture_output=True,
                text=True,
                timeout=30
            )
            
            # Stop containers
            logger.info("Stopping containers...")
            down_result = subprocess.run(
                ["docker-compose", "down"],
                cwd=project_dir,
                capture_output=True,
                text=True,
                timeout=60
            )
            
            return {
                "status": "passed",
                "build_output": build_result.stdout,
                "up_output": up_result.stdout,
                "ps_output": ps_result.stdout,
                "down_output": down_result.stdout,
                "build_errors": build_result.stderr,
                "up_errors": up_result.stderr,
                "ps_errors": ps_result.stderr,
                "down_errors": down_result.stderr
            }
            
        except subprocess.TimeoutExpired:
            logger.error("Docker Compose tests timed out")
            return {"status": "error", "message": "Tests timed out"}
        except Exception as e:
            logger.error(f"Docker Compose tests failed: {e}")
            return {"status": "error", "message": str(e)}
    
    def generate_report(self) -> str:
        """
        Generate a comprehensive test report.
        
        Returns:
            Path to the generated report
        """
        logger.info("Generating test report...")
        
        # Calculate duration
        duration = None
        if self.start_time and self.end_time:
            duration = self.end_time - self.start_time
        
        # Create report
        report = {
            "test_run": {
                "timestamp": datetime.now().isoformat(),
                "duration_seconds": duration.total_seconds() if duration else None,
                "start_time": self.start_time.isoformat() if self.start_time else None,
                "end_time": self.end_time.isoformat() if self.end_time else None
            },
            "prerequisites": self.test_results.get("prerequisites", {}),
            "tests": {
                "gpu_utilization": self.test_results.get("gpu_utilization", {}),
                "whisper_container": self.test_results.get("whisper_container", {}),
                "docker_compose": self.test_results.get("docker_compose", {})
            },
            "summary": self.generate_summary()
        }
        
        # Save report
        report_file = self.output_dir / f"gpu_test_report_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        with open(report_file, 'w') as f:
            json.dump(report, f, indent=2)
        
        # Generate HTML report
        html_file = self.output_dir / f"gpu_test_report_{datetime.now().strftime('%Y%m%d_%H%M%S')}.html"
        self.generate_html_report(report, html_file)
        
        logger.info(f"Test report saved to: {report_file}")
        logger.info(f"HTML report saved to: {html_file}")
        
        return str(report_file)
    
    def generate_summary(self) -> Dict[str, Any]:
        """
        Generate a summary of test results.
        
        Returns:
            Dictionary with test summary
        """
        summary = {
            "overall_status": "unknown",
            "prerequisites_met": False,
            "tests_passed": 0,
            "tests_failed": 0,
            "tests_error": 0,
            "total_tests": 0,
            "details": {}
        }
        
        # Check prerequisites
        prereqs = self.test_results.get("prerequisites", {})
        summary["prerequisites_met"] = all(prereqs.values())
        summary["details"]["prerequisites"] = prereqs
        
        # Check test results
        tests = {
            "gpu_utilization": self.test_results.get("gpu_utilization", {}),
            "whisper_container": self.test_results.get("whisper_container", {}),
            "docker_compose": self.test_results.get("docker_compose", {})
        }
        
        for test_name, result in tests.items():
            status = result.get("status", "unknown")
            summary["details"][test_name] = status
            
            if status == "passed":
                summary["tests_passed"] += 1
            elif status == "failed":
                summary["tests_failed"] += 1
            elif status == "error":
                summary["tests_error"] += 1
            
            summary["total_tests"] += 1
        
        # Determine overall status
        if summary["prerequisites_met"] and summary["tests_failed"] == 0 and summary["tests_error"] == 0:
            summary["overall_status"] = "passed"
        elif summary["tests_error"] > 0:
            summary["overall_status"] = "error"
        else:
            summary["overall_status"] = "failed"
        
        return summary
    
    def generate_html_report(self, report: Dict[str, Any], output_file: Path):
        """
        Generate an HTML test report.
        
        Args:
            report: Test report data
            output_file: Path to save the HTML report
        """
        html = f"""
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>GPU Test Report - {report['test_run']['timestamp']}</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background-color: #f5f5f5;
        }}
        .container {{
            background-color: white;
            padding: 30px;
            border-radius: 10px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }}
        h1 {{
            color: #333;
            text-align: center;
        }}
        h2 {{
            color: #555;
            border-bottom: 2px solid #eee;
            padding-bottom: 10px;
        }}
        .status {{
            padding: 10px;
            border-radius: 5px;
            margin-bottom: 20px;
            font-weight: bold;
        }}
        .status.passed {{
            background-color: #d4edda;
            color: #155724;
            border: 1px solid #c3e6cb;
        }}
        .status.failed {{
            background-color: #f8d7da;
            color: #721c24;
            border: 1px solid #f5c6cb;
        }}
        .status.error {{
            background-color: #fff3cd;
            color: #856404;
            border: 1px solid #ffeaa7;
        }}
        .status.unknown {{
            background-color: #d1ecf1;
            color: #0c5460;
            border: 1px solid #bee5eb;
        }}
        .section {{
            margin-bottom: 30px;
            padding: 20px;
            border: 1px solid #ddd;
            border-radius: 5px;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
            margin-bottom: 20px;
        }}
        th, td {{
            padding: 10px;
            text-align: left;
            border-bottom: 1px solid #ddd;
        }}
        th {{
            background-color: #f8f9fa;
            font-weight: bold;
        }}
        .pre {{
            background-color: #f8f9fa;
            padding: 15px;
            border-radius: 5px;
            overflow-x: auto;
            font-family: monospace;
            font-size: 12px;
        }}
        .summary {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }}
        .summary-item {{
            background-color: #f8f9fa;
            padding: 20px;
            border-radius: 5px;
            text-align: center;
        }}
        .summary-item h3 {{
            margin: 0 0 10px 0;
            color: #555;
        }}
        .summary-item .value {{
            font-size: 24px;
            font-weight: bold;
            color: #333;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>GPU Test Report</h1>
        
        <div class="status {report['summary']['overall_status']}">
            Overall Status: {report['summary']['overall_status'].upper()}
        </div>
        
        <div class="section">
            <h2>Test Run Information</h2>
            <table>
                <tr><th>Timestamp</th><td>{report['test_run']['timestamp']}</td></tr>
                <tr><th>Duration</th><td>{report['test_run']['duration_seconds'] or 'N/A'} seconds</td></tr>
                <tr><th>Start Time</th><td>{report['test_run']['start_time'] or 'N/A'}</td></tr>
                <tr><th>End Time</th><td>{report['test_run']['end_time'] or 'N/A'}</td></tr>
            </table>
        </div>
        
        <div class="section">
            <h2>Summary</h2>
            <div class="summary">
                <div class="summary-item">
                    <h3>Total Tests</h3>
                    <div class="value">{report['summary']['total_tests']}</div>
                </div>
                <div class="summary-item">
                    <h3>Passed</h3>
                    <div class="value">{report['summary']['tests_passed']}</div>
                </div>
                <div class="summary-item">
                    <h3>Failed</h3>
                    <div class="value">{report['summary']['tests_failed']}</div>
                </div>
                <div class="summary-item">
                    <h3>Errors</h3>
                    <div class="value">{report['summary']['tests_error']}</div>
                </div>
            </div>
        </div>
        
        <div class="section">
            <h2>Prerequisites</h2>
            <div class="status {'passed' if report['summary']['prerequisites_met'] else 'failed'}">
                Prerequisites Met: {report['summary']['prerequisites_met']}
            </div>
            <table>
                <tr><th>Component</th><th>Status</th></tr>
"""
        
        for component, status in report['summary']['details']['prerequisites'].items():
            status_class = "passed" if status else "failed"
            status_text = "PASS" if status else "FAIL"
            html += f"                <tr><td>{component}</td><td class=\"status {status_class}\">{status_text}</td></tr>\n"
        
        html += """
            </table>
        </div>
        
        <div class="section">
            <h2>Test Results</h2>
            <table>
                <tr><th>Test</th><th>Status</th></tr>
"""
        
        for test_name, status in report['summary']['details'].items():
            if test_name == "prerequisites":
                continue
            status_class = status
            html += f"                <tr><td>{test_name}</td><td class=\"status {status_class}\">{status.upper()}</td></tr>\n"
        
        html += """
            </table>
        </div>
        
        <div class="section">
            <h2>Detailed Results</h2>
"""
        
        for test_name, result in report['tests'].items():
            html += f"            <h3>{test_name.replace('_', ' ').title()}</h3>\n"
            html += f"            <div class=\"status {result.get('status', 'unknown')}\">Status: {result.get('status', 'unknown').upper()}</div>\n"
            
            if 'output' in result and result['output']:
                html += "            <h4>Output:</h4>\n"
                html += f"            <div class=\"pre\">{result['output']}</div>\n"
            
            if 'errors' in result and result['errors']:
                html += "            <h4>Errors:</h4>\n"
                html += f"            <div class=\"pre\">{result['errors']}</div>\n"
        
        html += """
        </div>
    </div>
</body>
</html>
"""
        
        with open(output_file, 'w') as f:
            f.write(html)
    
    def run_all_tests(self) -> str:
        """
        Run all tests and generate a report.
        
        Returns:
            Path to the generated report
        """
        logger.info("Starting GPU test suite...")
        self.start_time = datetime.now()
        
        try:
            # Check prerequisites
            logger.info("Step 1: Checking prerequisites...")
            self.test_results["prerequisites"] = self.check_prerequisites()
            
            # Run GPU utilization tests
            logger.info("Step 2: Running GPU utilization tests...")
            self.test_results["gpu_utilization"] = self.run_gpu_utilization_tests()
            
            # Run Whisper container tests
            logger.info("Step 3: Running Whisper container tests...")
            self.test_results["whisper_container"] = self.run_whisper_container_tests()
            
            # Run Docker Compose tests
            logger.info("Step 4: Running Docker Compose tests...")
            self.test_results["docker_compose"] = self.run_docker_compose_tests()
            
        except Exception as e:
            logger.error(f"Test suite failed: {e}")
        finally:
            self.end_time = datetime.now()
            
            # Generate report
            report_file = self.generate_report()
            
            # Print summary
            self.print_summary()
            
            return report_file
    
    def print_summary(self):
        """Print a summary of test results."""
        summary = self.generate_summary()
        
        logger.info("\n" + "="*50)
        logger.info("GPU TEST SUITE SUMMARY")
        logger.info("="*50)
        logger.info(f"Overall Status: {summary['overall_status'].upper()}")
        logger.info(f"Prerequisites Met: {summary['prerequisites_met']}")
        logger.info(f"Total Tests: {summary['total_tests']}")
        logger.info(f"Passed: {summary['tests_passed']}")
        logger.info(f"Failed: {summary['tests_failed']}")
        logger.info(f"Errors: {summary['tests_error']}")
        logger.info("="*50)
        
        if summary['overall_status'] == 'passed':
            logger.info("All tests passed! ✓")
        else:
            logger.error("Some tests failed! ✗")


def main():
    """Main function to run the test suite."""
    parser = argparse.ArgumentParser(description="Run GPU tests for ASRPro")
    parser.add_argument("--output-dir", default="test-reports", 
                       help="Directory to save test reports")
    parser.add_argument("--verbose", "-v", action="store_true", 
                       help="Enable verbose logging")
    parser.add_argument("--prerequisites-only", action="store_true",
                       help="Only check prerequisites")
    
    args = parser.parse_args()
    
    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    
    # Create test runner
    runner = TestRunner(args.output_dir)
    
    if args.prerequisites_only:
        # Only check prerequisites
        results = runner.check_prerequisites()
        print("\nPrerequisites Check Results:")
        for component, status in results.items():
            status_text = "PASS" if status else "FAIL"
            print(f"  {component}: {status_text}")
        
        return 0 if all(results.values()) else 1
    else:
        # Run all tests
        report_file = runner.run_all_tests()
        
        # Return appropriate exit code
        summary = runner.generate_summary()
        if summary['overall_status'] == 'passed':
            return 0
        else:
            return 1


if __name__ == "__main__":
    sys.exit(main())