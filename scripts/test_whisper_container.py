#!/usr/bin/env python3
"""
Test script for Whisper GPU container integration
"""

import os
import sys
import time
import json
import logging
import subprocess
import requests
import tempfile
import wave
import numpy as np
from pathlib import Path
from typing import Dict, Any, Optional, List

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


class WhisperContainerTester:
    """Test suite for Whisper GPU container."""
    
    def __init__(self, container_name: str = "whisper-gpu-test", port: int = 8001):
        """
        Initialize the container tester.
        
        Args:
            container_name: Name for the test container
            port: Port to expose the container service
        """
        self.container_name = container_name
        self.port = port
        self.container_id = None
        self.base_url = f"http://localhost:{port}"
        self.test_audio_path = None
        
    def create_test_audio(self, duration: float = 5.0, sample_rate: int = 16000) -> str:
        """
        Create a test audio file with sine wave.
        
        Args:
            duration: Duration in seconds
            sample_rate: Sample rate in Hz
            
        Returns:
            Path to the created audio file
        """
        # Generate a sine wave test signal
        t = np.linspace(0, duration, int(sample_rate * duration), False)
        # Generate a 440 Hz sine wave
        tone = np.sin(440 * 2 * np.pi * t) * 0.3
        
        # Add some noise to make it more realistic
        noise = np.random.normal(0, 0.05, tone.shape)
        audio = tone + noise
        
        # Normalize to 16-bit PCM
        audio = (audio * 32767).astype(np.int16)
        
        # Create temporary file
        with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
            with wave.open(f.name, 'wb') as wav_file:
                wav_file.setnchannels(1)  # Mono
                wav_file.setsampwidth(2)  # 16-bit
                wav_file.setframerate(sample_rate)
                wav_file.writeframes(audio.tobytes())
            
            self.test_audio_path = f.name
            logger.info(f"Created test audio file: {f.name}")
            return f.name
    
    def check_prerequisites(self) -> bool:
        """
        Check if all prerequisites are met.
        
        Returns:
            True if all prerequisites are met
        """
        logger.info("Checking prerequisites...")
        
        # Check Docker
        try:
            result = subprocess.run(
                ["docker", "--version"],
                capture_output=True,
                text=True,
                timeout=10
            )
            if result.returncode != 0:
                logger.error("Docker not found or not working")
                return False
            logger.info(f"Docker available: {result.stdout.strip()}")
        except (subprocess.TimeoutExpired, subprocess.SubprocessError, FileNotFoundError) as e:
            logger.error(f"Docker check failed: {e}")
            return False
        
        # Check NVIDIA Docker support
        try:
            result = subprocess.run(
                ["docker", "run", "--rm", "--gpus", "all", "nvidia/cuda:12.1.0-base-ubuntu22.04", 
                 "nvidia-smi", "--query-gpu=name", "--format=csv,noheader,nounits"],
                capture_output=True,
                text=True,
                timeout=30
            )
            if result.returncode != 0:
                logger.error("NVIDIA Docker support not available")
                return False
            logger.info(f"NVIDIA Docker support available, GPU: {result.stdout.strip()}")
        except (subprocess.TimeoutExpired, subprocess.SubprocessError) as e:
            logger.error(f"NVIDIA Docker check failed: {e}")
            return False
        
        # Check if we can build the container
        dockerfile_path = Path(__file__).parent.parent / "docker" / "whisper-gpu" / "Dockerfile"
        if not dockerfile_path.exists():
            logger.error(f"Dockerfile not found at {dockerfile_path}")
            return False
        
        logger.info("All prerequisites met")
        return True
    
    def build_container(self) -> bool:
        """
        Build the Whisper GPU container.
        
        Returns:
            True if build was successful
        """
        logger.info("Building Whisper GPU container...")
        
        dockerfile_path = Path(__file__).parent.parent / "docker" / "whisper-gpu"
        
        try:
            # Build the container
            cmd = [
                "docker", "build",
                "-t", "asrpro/whisper-gpu:test",
                str(dockerfile_path)
            ]
            
            logger.info(f"Running: {' '.join(cmd)}")
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=300  # 5 minutes timeout
            )
            
            if result.returncode != 0:
                logger.error(f"Container build failed: {result.stderr}")
                return False
            
            logger.info("Container build successful")
            return True
            
        except subprocess.TimeoutExpired:
            logger.error("Container build timed out")
            return False
        except Exception as e:
            logger.error(f"Container build error: {e}")
            return False
    
    def start_container(self) -> bool:
        """
        Start the Whisper GPU container.
        
        Returns:
            True if container started successfully
        """
        logger.info("Starting Whisper GPU container...")
        
        try:
            # Stop any existing container with the same name
            subprocess.run(
                ["docker", "stop", self.container_name],
                capture_output=True,
                timeout=10
            )
            subprocess.run(
                ["docker", "rm", self.container_name],
                capture_output=True,
                timeout=10
            )
            
            # Start new container
            cmd = [
                "docker", "run", "-d",
                "--name", self.container_name,
                "--gpus", "all",
                "-p", f"{self.port}:8000",
                "--memory", "8g",
                "asrpro/whisper-gpu:test"
            ]
            
            logger.info(f"Running: {' '.join(cmd)}")
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=30
            )
            
            if result.returncode != 0:
                logger.error(f"Container start failed: {result.stderr}")
                return False
            
            self.container_id = result.stdout.strip()
            logger.info(f"Container started with ID: {self.container_id}")
            
            # Wait for container to be ready
            return self.wait_for_container_ready()
            
        except subprocess.TimeoutExpired:
            logger.error("Container start timed out")
            return False
        except Exception as e:
            logger.error(f"Container start error: {e}")
            return False
    
    def wait_for_container_ready(self, timeout: int = 60) -> bool:
        """
        Wait for the container to be ready.
        
        Args:
            timeout: Timeout in seconds
            
        Returns:
            True if container is ready
        """
        logger.info(f"Waiting for container to be ready (timeout: {timeout}s)...")
        
        start_time = time.time()
        while time.time() - start_time < timeout:
            try:
                # Check health endpoint
                response = requests.get(f"{self.base_url}/health", timeout=5)
                if response.status_code == 200:
                    logger.info("Container is ready")
                    return True
            except requests.exceptions.RequestException:
                pass
            
            # Check container status
            try:
                result = subprocess.run(
                    ["docker", "inspect", self.container_name, "--format='{{.State.Status}}'"],
                    capture_output=True,
                    text=True,
                    timeout=5
                )
                if result.returncode == 0 and "exited" in result.stdout:
                    logger.error("Container has exited")
                    return False
            except subprocess.TimeoutExpired:
                pass
            
            time.sleep(2)
        
        logger.error("Container readiness timeout")
        return False
    
    def test_gpu_access(self) -> bool:
        """
        Test GPU access inside the container.
        
        Returns:
            True if GPU access is working
        """
        logger.info("Testing GPU access inside container...")
        
        try:
            # Check GPU info endpoint
            response = requests.get(f"{self.base_url}/gpu/info", timeout=10)
            if response.status_code != 200:
                logger.error(f"GPU info endpoint failed: {response.status_code}")
                return False
            
            gpu_info = response.json()
            logger.info(f"GPU info: {json.dumps(gpu_info, indent=2)}")
            
            # Verify GPU is detected
            if not gpu_info.get("gpu_available", False):
                logger.error("GPU not available in container")
                return False
            
            # Check GPU memory
            if not gpu_info.get("memory_total", 0) > 0:
                logger.error("GPU memory not detected")
                return False
            
            logger.info("GPU access test passed")
            return True
            
        except requests.exceptions.RequestException as e:
            logger.error(f"GPU access test failed: {e}")
            return False
    
    def test_transcription(self) -> bool:
        """
        Test transcription with the container.
        
        Returns:
            True if transcription test passed
        """
        logger.info("Testing transcription...")
        
        if not self.test_audio_path:
            logger.error("No test audio file available")
            return False
        
        try:
            # Upload and transcribe audio
            with open(self.test_audio_path, 'rb') as f:
                files = {'audio': f}
                response = requests.post(
                    f"{self.base_url}/transcribe",
                    files=files,
                    timeout=60
                )
            
            if response.status_code != 200:
                logger.error(f"Transcription failed: {response.status_code}")
                return False
            
            result = response.json()
            logger.info(f"Transcription result: {json.dumps(result, indent=2)}")
            
            # Verify result structure
            if "text" not in result:
                logger.error("Transcription result missing 'text' field")
                return False
            
            # Log the transcribed text
            logger.info(f"Transcribed text: {result['text']}")
            
            logger.info("Transcription test passed")
            return True
            
        except requests.exceptions.RequestException as e:
            logger.error(f"Transcription test failed: {e}")
            return False
    
    def test_gpu_memory_tracking(self) -> bool:
        """
        Test GPU memory tracking during transcription.
        
        Returns:
            True if memory tracking test passed
        """
        logger.info("Testing GPU memory tracking...")
        
        try:
            # Get initial GPU memory
            response = requests.get(f"{self.base_url}/gpu/memory", timeout=10)
            if response.status_code != 200:
                logger.error(f"GPU memory endpoint failed: {response.status_code}")
                return False
            
            initial_memory = response.json()
            logger.info(f"Initial GPU memory: {json.dumps(initial_memory, indent=2)}")
            
            # Perform transcription
            if not self.test_transcription():
                return False
            
            # Get GPU memory after transcription
            response = requests.get(f"{self.base_url}/gpu/memory", timeout=10)
            if response.status_code != 200:
                logger.error(f"GPU memory endpoint failed after transcription: {response.status_code}")
                return False
            
            final_memory = response.json()
            logger.info(f"Final GPU memory: {json.dumps(final_memory, indent=2)}")
            
            # Verify memory tracking is working
            if "allocated" not in initial_memory or "allocated" not in final_memory:
                logger.error("Memory tracking not working properly")
                return False
            
            logger.info("GPU memory tracking test passed")
            return True
            
        except requests.exceptions.RequestException as e:
            logger.error(f"GPU memory tracking test failed: {e}")
            return False
    
    def test_container_logs(self) -> bool:
        """
        Check container logs for errors.
        
        Returns:
            True if no critical errors found
        """
        logger.info("Checking container logs...")
        
        try:
            result = subprocess.run(
                ["docker", "logs", self.container_name],
                capture_output=True,
                text=True,
                timeout=10
            )
            
            if result.returncode != 0:
                logger.error(f"Failed to get container logs: {result.stderr}")
                return False
            
            logs = result.stdout
            logger.info(f"Container logs:\n{logs}")
            
            # Check for critical errors
            critical_errors = [
                "CUDA out of memory",
                "CUDA error",
                "GPU memory allocation failed",
                "RuntimeError",
                "Fatal error"
            ]
            
            for error in critical_errors:
                if error.lower() in logs.lower():
                    logger.error(f"Critical error found in logs: {error}")
                    return False
            
            logger.info("Container logs check passed")
            return True
            
        except subprocess.TimeoutExpired:
            logger.error("Container logs check timed out")
            return False
        except Exception as e:
            logger.error(f"Container logs check error: {e}")
            return False
    
    def stop_container(self) -> bool:
        """
        Stop and remove the test container.
        
        Returns:
            True if container stopped successfully
        """
        logger.info("Stopping container...")
        
        try:
            # Stop container
            result = subprocess.run(
                ["docker", "stop", self.container_name],
                capture_output=True,
                text=True,
                timeout=10
            )
            
            if result.returncode != 0:
                logger.warning(f"Container stop warning: {result.stderr}")
            
            # Remove container
            result = subprocess.run(
                ["docker", "rm", self.container_name],
                capture_output=True,
                text=True,
                timeout=10
            )
            
            if result.returncode != 0:
                logger.warning(f"Container removal warning: {result.stderr}")
            
            logger.info("Container stopped and removed")
            return True
            
        except subprocess.TimeoutExpired:
            logger.error("Container stop timed out")
            return False
        except Exception as e:
            logger.error(f"Container stop error: {e}")
            return False
    
    def cleanup(self):
        """Clean up test resources."""
        logger.info("Cleaning up...")
        
        # Stop container
        if self.container_id:
            self.stop_container()
        
        # Remove test audio file
        if self.test_audio_path and os.path.exists(self.test_audio_path):
            os.unlink(self.test_audio_path)
            logger.info(f"Removed test audio file: {self.test_audio_path}")
    
    def run_all_tests(self) -> Dict[str, bool]:
        """
        Run all tests and return results.
        
        Returns:
            Dictionary with test results
        """
        results = {
            "prerequisites": False,
            "build": False,
            "start": False,
            "gpu_access": False,
            "transcription": False,
            "memory_tracking": False,
            "logs_check": False
        }
        
        try:
            # Check prerequisites
            if not self.check_prerequisites():
                logger.error("Prerequisites check failed")
                return results
            results["prerequisites"] = True
            
            # Create test audio
            self.create_test_audio()
            
            # Build container
            if not self.build_container():
                logger.error("Container build failed")
                return results
            results["build"] = True
            
            # Start container
            if not self.start_container():
                logger.error("Container start failed")
                return results
            results["start"] = True
            
            # Run tests
            results["gpu_access"] = self.test_gpu_access()
            results["transcription"] = self.test_transcription()
            results["memory_tracking"] = self.test_gpu_memory_tracking()
            results["logs_check"] = self.test_container_logs()
            
        except Exception as e:
            logger.error(f"Test execution error: {e}")
        finally:
            self.cleanup()
        
        return results


def main():
    """Main function to run the tests."""
    import argparse
    
    parser = argparse.ArgumentParser(description="Test Whisper GPU container")
    parser.add_argument("--container-name", default="whisper-gpu-test", 
                       help="Name for the test container")
    parser.add_argument("--port", type=int, default=8001, 
                       help="Port to expose the container service")
    parser.add_argument("--verbose", "-v", action="store_true", 
                       help="Enable verbose logging")
    
    args = parser.parse_args()
    
    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    
    # Create tester
    tester = WhisperContainerTester(args.container_name, args.port)
    
    # Run tests
    logger.info("Starting Whisper GPU container tests...")
    results = tester.run_all_tests()
    
    # Print results
    logger.info("\n" + "="*50)
    logger.info("TEST RESULTS")
    logger.info("="*50)
    
    passed = 0
    total = len(results)
    
    for test_name, result in results.items():
        status = "PASS" if result else "FAIL"
        logger.info(f"{test_name.replace('_', ' ').title()}: {status}")
        if result:
            passed += 1
    
    logger.info("="*50)
    logger.info(f"Overall: {passed}/{total} tests passed")
    
    if passed == total:
        logger.info("All tests passed! ✓")
        return 0
    else:
        logger.error(f"{total - passed} tests failed! ✗")
        return 1


if __name__ == "__main__":
    sys.exit(main())