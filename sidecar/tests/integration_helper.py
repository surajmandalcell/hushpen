"""
Integration test helper utilities for Docker-based ASR system
"""

import asyncio
import json
import logging
import os
import tempfile
import time
import wave
from pathlib import Path
from typing import Dict, Any, Optional, List, Tuple
import httpx
import websockets
import docker
from docker.models.containers import Container

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class IntegrationTestHelper:
    """Helper class for integration testing of Docker-based ASR system."""
    
    def __init__(self, config_file: Optional[str] = None):
        """
        Initialize integration test helper.
        
        Args:
            config_file: Path to test configuration file
        """
        self.config_file = config_file or os.path.join(
            os.path.dirname(__file__), "test_config.json"
        )
        self.config = self._load_config()
        self.docker_client = None
        self.test_data_dir = Path(__file__).parent / "test_data"
        self.temp_dir = None
        self.server_process = None
        self.server_url = f"http://{self.config.get('server_host', '127.0.0.1')}:{self.config.get('server_port', 8000)}"
        self.websocket_url = f"ws://{self.config.get('server_host', '127.0.0.1')}:{self.config.get('server_port', 8000)}/ws"
        
        # Ensure test data directory exists
        self.test_data_dir.mkdir(exist_ok=True)
        
        # Initialize Docker client
        self._initialize_docker_client()
    
    def _load_config(self) -> Dict[str, Any]:
        """Load test configuration."""
        try:
            with open(self.config_file, 'r') as f:
                return json.load(f)
        except Exception as e:
            logger.warning(f"Failed to load config from {self.config_file}: {e}")
            return self._get_default_config()
    
    def _get_default_config(self) -> Dict[str, Any]:
        """Get default test configuration."""
        return {
            "server_host": "127.0.0.1",
            "server_port": 8000,
            "test_models": ["whisper-tiny", "whisper-base"],
            "test_audio_files": {
                "short": "sample1.mp3",
                "medium": "sample2.mp3",
                "long": "sample3.mp3"
            },
            "gpu_tests": {
                "enabled": True,
                "min_memory_mb": 2048,
                "max_memory_mb": 8192
            },
            "concurrent_tests": {
                "enabled": True,
                "max_concurrent": 3
            },
            "timeouts": {
                "container_startup": 60,
                "transcription": 120,
                "websocket_connect": 10
            }
        }
    
    def _initialize_docker_client(self):
        """Initialize Docker client."""
        try:
            self.docker_client = docker.from_env()
            self.docker_client.ping()
            logger.info("Docker client initialized successfully")
        except Exception as e:
            logger.error(f"Failed to initialize Docker client: {e}")
            raise
    
    def setup_test_environment(self):
        """Set up test environment."""
        logger.info("Setting up test environment")
        
        # Create temporary directory
        self.temp_dir = tempfile.mkdtemp(prefix="asr_e2e_test_")
        logger.info(f"Created temporary directory: {self.temp_dir}")
        
        # Generate test audio files if they don't exist
        self._generate_test_audio_files()
        
        # Clean up any existing test containers
        self._cleanup_test_containers()
        
        logger.info("Test environment setup complete")
    
    def cleanup_test_environment(self):
        """Clean up test environment."""
        logger.info("Cleaning up test environment")
        
        # Stop server if running
        if self.server_process:
            self.stop_server()
        
        # Clean up test containers
        self._cleanup_test_containers()
        
        # Remove temporary directory
        if self.temp_dir and os.path.exists(self.temp_dir):
            import shutil
            shutil.rmtree(self.temp_dir)
            logger.info(f"Removed temporary directory: {self.temp_dir}")
        
        logger.info("Test environment cleanup complete")
    
    def _generate_test_audio_files(self):
        """Generate test audio files if they don't exist."""
        logger.info("Generating test audio files")
        
        # Define test audio files
        test_files = {
            "sample1.mp3": {"duration": 2, "frequency": 440},  # 2 seconds, 440Hz
            "sample2.mp3": {"duration": 5, "frequency": 880},  # 5 seconds, 880Hz
            "sample3.mp3": {"duration": 10, "frequency": 220}, # 10 seconds, 220Hz
        }
        
        for filename, params in test_files.items():
            file_path = self.test_data_dir / filename
            if not file_path.exists():
                self._generate_audio_file(file_path, params["duration"], params["frequency"])
                logger.info(f"Generated test audio file: {file_path}")
    
    def _generate_audio_file(self, file_path: Path, duration: float, frequency: int):
        """Generate a simple audio file for testing."""
        import numpy as np
        
        # Generate audio data
        sample_rate = 16000  # 16kHz
        samples = int(duration * sample_rate)
        t = np.linspace(0, duration, samples, False)
        audio_data = np.sin(frequency * 2 * np.pi * t) * 0.3  # Sine wave
        
        # Convert to 16-bit PCM
        audio_data = (audio_data * 32767).astype(np.int16)
        
        # Create WAV file
        with wave.open(str(file_path), 'w') as wav_file:
            wav_file.setnchannels(1)  # Mono
            wav_file.setsampwidth(2)  # 16-bit
            wav_file.setframerate(sample_rate)
            wav_file.writeframes(audio_data.tobytes())
    
    def _cleanup_test_containers(self):
        """Clean up any test containers."""
        try:
            containers = self.docker_client.containers.list(
                all=True,
                filters={"label": "asrpro_test=true"}
            )
            
            for container in containers:
                try:
                    container.remove(force=True)
                    logger.info(f"Removed test container: {container.id}")
                except Exception as e:
                    logger.warning(f"Failed to remove container {container.id}: {e}")
        except Exception as e:
            logger.warning(f"Failed to list test containers: {e}")
    
    def start_server(self) -> bool:
        """Start the ASR server for testing."""
        logger.info("Starting ASR server")
        
        try:
            import subprocess
            import sys
            
            # Get path to main.py
            main_path = Path(__file__).parent.parent / "main.py"
            
            # Start server in subprocess
            self.server_process = subprocess.Popen(
                [sys.executable, str(main_path)],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env={**os.environ, "PYTHONPATH": str(Path(__file__).parent.parent)}
            )
            
            # Wait for server to be ready
            max_wait = 30
            for i in range(max_wait):
                if self._is_server_ready():
                    logger.info("ASR server started successfully")
                    return True
                time.sleep(1)
            
            logger.error("Server failed to start within timeout")
            return False
            
        except Exception as e:
            logger.error(f"Failed to start server: {e}")
            return False
    
    def stop_server(self):
        """Stop the ASR server."""
        if self.server_process:
            logger.info("Stopping ASR server")
            self.server_process.terminate()
            try:
                self.server_process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.server_process.kill()
            self.server_process = None
            logger.info("ASR server stopped")
    
    def _is_server_ready(self) -> bool:
        """Check if the server is ready."""
        try:
            response = httpx.get(f"{self.server_url}/health", timeout=5)
            return response.status_code == 200
        except Exception:
            return False
    
    async def wait_for_container_ready(self, model_id: str, timeout: int = 60) -> bool:
        """
        Wait for a container to be ready.
        
        Args:
            model_id: Model ID
            timeout: Timeout in seconds
            
        Returns:
            True if container is ready
        """
        start_time = time.time()
        
        while time.time() - start_time < timeout:
            try:
                # Check model status
                response = httpx.get(f"{self.server_url}/v1/models", timeout=5)
                if response.status_code == 200:
                    models = response.json()
                    for model in models.get("data", []):
                        if model.get("id") == model_id and model.get("ready"):
                            return True
                
                await asyncio.sleep(2)
            except Exception as e:
                logger.debug(f"Error checking container readiness: {e}")
                await asyncio.sleep(2)
        
        return False
    
    def get_gpu_utilization(self) -> Optional[Dict[str, Any]]:
        """Get current GPU utilization."""
        try:
            response = httpx.get(f"{self.server_url}/health", timeout=5)
            if response.status_code == 200:
                # This would need to be extended to include GPU info in health endpoint
                # For now, we'll use a basic check
                return {"available": True}
        except Exception as e:
            logger.error(f"Failed to get GPU utilization: {e}")
        
        return None
    
    async def transcribe_audio(
        self, 
        audio_file: Path, 
        model: str = "whisper-base",
        expected_format: str = "json"
    ) -> Dict[str, Any]:
        """
        Transcribe audio file.
        
        Args:
            audio_file: Path to audio file
            model: Model to use
            expected_format: Expected response format
            
        Returns:
            Transcription result
        """
        with open(audio_file, 'rb') as f:
            files = {'file': (audio_file.name, f, 'audio/mpeg')}
            data = {'model': model, 'response_format': expected_format}
            
            async with httpx.AsyncClient() as client:
                response = await client.post(
                    f"{self.server_url}/v1/audio/transcriptions",
                    files=files,
                    data=data,
                    timeout=self.config.get("timeouts", {}).get("transcription", 120)
                )
                
                if response.status_code == 200:
                    if expected_format == "json":
                        return response.json()
                    else:
                        return {"text": response.text}
                else:
                    raise Exception(f"Transcription failed: {response.status_code} - {response.text}")
    
    async def connect_websocket(self) -> websockets.WebSocketServerProtocol:
        """Connect to WebSocket for real-time updates."""
        return await websockets.connect(self.websocket_url)
    
    async def wait_for_websocket_message(
        self, 
        websocket: websockets.WebSocketServerProtocol,
        message_type: str,
        timeout: int = 30
    ) -> Optional[Dict[str, Any]]:
        """
        Wait for specific WebSocket message type.
        
        Args:
            websocket: WebSocket connection
            message_type: Expected message type
            timeout: Timeout in seconds
            
        Returns:
            Message data or None if timeout
        """
        start_time = time.time()
        
        while time.time() - start_time < timeout:
            try:
                message = await asyncio.wait_for(websocket.recv(), timeout=1)
                data = json.loads(message)
                
                if data.get("type") == message_type:
                    return data.get("data")
                    
            except asyncio.TimeoutError:
                continue
            except Exception as e:
                logger.debug(f"Error receiving WebSocket message: {e}")
        
        return None
    
    def verify_container_status(self, model_id: str) -> bool:
        """
        Verify container status for a model.
        
        Args:
            model_id: Model ID
            
        Returns:
            True if container is running
        """
        try:
            # Check if container exists and is running
            containers = self.docker_client.containers.list(
                filters={"label": f"asrpro_model={model_id}"}
            )
            
            return len(containers) > 0 and containers[0].status == "running"
        except Exception as e:
            logger.error(f"Failed to verify container status: {e}")
            return False
    
    def get_container_logs(self, model_id: str, lines: int = 100) -> Optional[str]:
        """
        Get logs from a container.
        
        Args:
            model_id: Model ID
            lines: Number of lines to retrieve
            
        Returns:
            Container logs or None
        """
        try:
            containers = self.docker_client.containers.list(
                all=True,
                filters={"label": f"asrpro_model={model_id}"}
            )
            
            if containers:
                return containers[0].logs(tail=lines).decode('utf-8')
        except Exception as e:
            logger.error(f"Failed to get container logs: {e}")
        
        return None
    
    async def run_concurrent_transcriptions(
        self, 
        audio_files: List[Path], 
        model: str = "whisper-base"
    ) -> List[Dict[str, Any]]:
        """
        Run multiple transcriptions concurrently.
        
        Args:
            audio_files: List of audio files to transcribe
            model: Model to use
            
        Returns:
            List of transcription results
        """
        tasks = []
        for audio_file in audio_files:
            task = self.transcribe_audio(audio_file, model)
            tasks.append(task)
        
        return await asyncio.gather(*tasks, return_exceptions=True)
    
    def create_test_report(self, test_results: Dict[str, Any]) -> str:
        """
        Create a test report.
        
        Args:
            test_results: Test results dictionary
            
        Returns:
            Test report as JSON string
        """
        report = {
            "timestamp": time.time(),
            "summary": {
                "total_tests": len(test_results),
                "passed": sum(1 for r in test_results.values() if r.get("status") == "passed"),
                "failed": sum(1 for r in test_results.values() if r.get("status") == "failed"),
                "skipped": sum(1 for r in test_results.values() if r.get("status") == "skipped")
            },
            "environment": {
                "server_url": self.server_url,
                "docker_available": self.docker_client is not None,
                "temp_dir": self.temp_dir
            },
            "results": test_results
        }
        
        return json.dumps(report, indent=2)