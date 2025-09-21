#!/usr/bin/env python3
"""
Test framework for ASR Pro model testing
"""

import asyncio
import tempfile
import os
import json
import time
from pathlib import Path
from typing import Dict, Any, List, Optional
from datetime import datetime
import wave
import numpy as np


class TestResult:
    """Test result container."""

    def __init__(self, model_id: str, test_name: str):
        self.model_id = model_id
        self.test_name = test_name
        self.passed = False
        self.error = None
        self.duration = 0.0
        self.input_info = {}
        self.output_info = {}
        self.timestamp = datetime.now().isoformat()

    def to_dict(self) -> Dict[str, Any]:
        return {
            "model_id": self.model_id,
            "test_name": self.test_name,
            "passed": self.passed,
            "error": self.error,
            "duration": self.duration,
            "input_info": self.input_info,
            "output_info": self.output_info,
            "timestamp": self.timestamp,
        }


class AudioGenerator:
    """Generate test audio files."""

    @staticmethod
    def create_silence(duration: float = 1.0, sample_rate: int = 16000) -> bytes:
        """Create silent audio."""
        samples = int(duration * sample_rate)
        silence = np.zeros(samples, dtype=np.int16)
        return silence.tobytes()

    @staticmethod
    def create_tone(
        frequency: float = 440.0, duration: float = 1.0, sample_rate: int = 16000
    ) -> bytes:
        """Create a sine wave tone."""
        samples = int(duration * sample_rate)
        t = np.linspace(0, duration, samples, False)
        wave = np.sin(frequency * 2 * np.pi * t)
        # Convert to 16-bit PCM
        wave_16bit = (wave * 32767).astype(np.int16)
        return wave_16bit.tobytes()

    @staticmethod
    def create_noise(duration: float = 1.0, sample_rate: int = 16000) -> bytes:
        """Create white noise."""
        samples = int(duration * sample_rate)
        noise = np.random.randint(-32768, 32767, samples, dtype=np.int16)
        return noise.tobytes()

    @staticmethod
    def create_wav_file(
        audio_data: bytes, sample_rate: int = 16000, filename: str = None
    ) -> str:
        """Create a WAV file from audio data."""
        if filename is None:
            fd, filename = tempfile.mkstemp(suffix=".wav")
            os.close(fd)

        with wave.open(filename, "wb") as wav_file:
            wav_file.setnchannels(1)  # Mono
            wav_file.setsampwidth(2)  # 16-bit
            wav_file.setframerate(sample_rate)
            wav_file.writeframes(audio_data)

        return filename


class ModelTester:
    """Base class for model testing."""

    def __init__(self, output_dir: str = "outputs"):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(exist_ok=True)
        self.results: List[TestResult] = []
        self.audio_gen = AudioGenerator()
        self.tests_dir = Path(__file__).parent

    async def test_model_loading(self, model_id: str, manager) -> TestResult:
        """Test model loading."""
        result = TestResult(model_id, "model_loading")
        start_time = time.time()

        try:
            success = await manager.set_model(model_id)
            result.passed = success
            result.input_info = {"model_id": model_id}
            result.output_info = {
                "loaded": success,
                "current_model": manager.get_current_model(),
                "model_ready": manager.is_model_ready(model_id),
            }
        except Exception as e:
            result.error = str(e)
            result.passed = False

        result.duration = time.time() - start_time
        self.results.append(result)
        return result

    async def test_transcription(
        self, model_id: str, manager, audio_file: str, test_name: str
    ) -> TestResult:
        """Test transcription."""
        result = TestResult(model_id, test_name)
        start_time = time.time()

        try:
            with open(audio_file, "rb") as f:
                transcription = await manager.transcribe_file(f, model_id)

            result.passed = True
            result.input_info = {
                "audio_file": audio_file,
                "file_size": os.path.getsize(audio_file),
            }
            result.output_info = transcription

            # Save detailed output
            output_file = self.output_dir / f"{model_id}_{test_name}_output.json"
            with open(output_file, "w") as f:
                json.dump(
                    {
                        "input": result.input_info,
                        "output": result.output_info,
                        "test_info": {
                            "model_id": model_id,
                            "test_name": test_name,
                            "timestamp": result.timestamp,
                        },
                    },
                    f,
                    indent=2,
                )

        except Exception as e:
            result.error = str(e)
            result.passed = False

        result.duration = time.time() - start_time
        self.results.append(result)
        return result

    async def test_real_audio(self, model_id: str, manager) -> TestResult:
        """Test with real audio file."""
        audio_file = self.tests_dir / "audio_for_test.mp3"
        if not audio_file.exists():
            # Fallback to generated audio if real file doesn't exist
            audio_file = self.audio_gen.create_wav_file(
                self.audio_gen.create_silence(2.0),
                filename=str(self.output_dir / f"{model_id}_real_audio.wav"),
            )
        else:
            # Copy to outputs for reference
            import shutil

            output_audio = self.output_dir / f"{model_id}_real_audio.mp3"
            shutil.copy2(audio_file, output_audio)
            audio_file = str(output_audio)

        return await self.test_transcription(
            model_id, manager, audio_file, "real_audio"
        )

    async def test_edge_cases(self, model_id: str, manager) -> List[TestResult]:
        """Test basic edge cases - simplified for English/Hindi focus."""
        results = []

        # Test 1: Short audio (1 second)
        short_audio = self.audio_gen.create_wav_file(
            self.audio_gen.create_silence(1.0),  # 1 second
            filename=str(self.output_dir / f"{model_id}_short_audio.wav"),
        )
        result = await self.test_transcription(
            model_id, manager, short_audio, "edge_case_short"
        )
        results.append(result)

        # Test 2: Long audio (10 seconds)
        long_audio = self.audio_gen.create_wav_file(
            self.audio_gen.create_silence(10.0),  # 10 seconds
            filename=str(self.output_dir / f"{model_id}_long_audio.wav"),
        )
        result = await self.test_transcription(
            model_id, manager, long_audio, "edge_case_long"
        )
        results.append(result)

        return results

    def save_report(self, model_id: str):
        """Save test report for a model."""
        model_results = [r for r in self.results if r.model_id == model_id]

        report = {
            "model_id": model_id,
            "total_tests": len(model_results),
            "passed_tests": sum(1 for r in model_results if r.passed),
            "failed_tests": sum(1 for r in model_results if not r.passed),
            "total_duration": sum(r.duration for r in model_results),
            "tests": [r.to_dict() for r in model_results],
            "summary": {
                "success_rate": (
                    sum(1 for r in model_results if r.passed) / len(model_results)
                    if model_results
                    else 0
                ),
                "average_duration": (
                    sum(r.duration for r in model_results) / len(model_results)
                    if model_results
                    else 0
                ),
            },
        }

        report_file = self.output_dir / f"{model_id}_report.json"
        with open(report_file, "w") as f:
            json.dump(report, f, indent=2)

        return report
