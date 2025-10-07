"""
End-to-end tests for Docker-based ASR system
"""

import asyncio
import json
import logging
import time
import traceback
from pathlib import Path
from typing import Dict, Any, List, Optional
import pytest
import httpx
import websockets

from integration_helper import IntegrationTestHelper

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class TestE2EDockerASR:
    """End-to-end tests for Docker-based ASR system."""
    
    @pytest.fixture(scope="class")
    def test_helper(self):
        """Create test helper instance."""
        helper = IntegrationTestHelper()
        helper.setup_test_environment()
        yield helper
        helper.cleanup_test_environment()
    
    @pytest.fixture(scope="class", autouse=True)
    def setup_server(self, test_helper):
        """Start server for all tests."""
        if not test_helper.start_server():
            pytest.fail("Failed to start ASR server")
        yield
        test_helper.stop_server()
    
    @pytest.mark.asyncio
    async def test_server_health_check(self, test_helper):
        """Test server health check endpoint."""
        logger.info("Testing server health check")
        
        try:
            async with httpx.AsyncClient() as client:
                response = await client.get(f"{test_helper.server_url}/health", timeout=10)
                
                assert response.status_code == 200
                health_data = response.json()
                
                assert "status" in health_data
                assert health_data["status"] in ["healthy", "ready", "initializing"]
                assert "device" in health_data
                
                logger.info(f"Health check passed: {health_data}")
                return {"status": "passed", "data": health_data}
                
        except Exception as e:
            logger.error(f"Health check failed: {e}")
            return {"status": "failed", "error": str(e)}
    
    @pytest.mark.asyncio
    async def test_list_models(self, test_helper):
        """Test listing available models."""
        logger.info("Testing model listing")
        
        try:
            async with httpx.AsyncClient() as client:
                response = await client.get(f"{test_helper.server_url}/v1/models", timeout=10)
                
                assert response.status_code == 200
                models_data = response.json()
                
                assert "data" in models_data
                assert len(models_data["data"]) > 0
                
                # Check that test models are available
                available_model_ids = [model["id"] for model in models_data["data"]]
                test_models = test_helper.config["test_models"]
                
                for model_id in test_models[:2]:  # Check first 2 models
                    assert model_id in available_model_ids, f"Model {model_id} not found"
                
                logger.info(f"Model listing passed: {len(models_data['data'])} models available")
                return {"status": "passed", "data": models_data}
                
        except Exception as e:
            logger.error(f"Model listing failed: {e}")
            return {"status": "failed", "error": str(e)}
    
    @pytest.mark.asyncio
    async def test_set_model(self, test_helper):
        """Test setting active model."""
        logger.info("Testing model setting")
        
        model_id = test_helper.config["test_models"][0]
        
        try:
            async with httpx.AsyncClient() as client:
                response = await client.post(
                    f"{test_helper.server_url}/v1/settings/model",
                    json={"model_id": model_id},
                    timeout=30
                )
                
                assert response.status_code == 200
                result = response.json()
                
                assert result["status"] == "success"
                assert result["model"] == model_id
                
                # Wait for container to be ready
                container_ready = await test_helper.wait_for_container_ready(model_id)
                assert container_ready, f"Container for model {model_id} not ready"
                
                # Verify container is running
                container_status = test_helper.verify_container_status(model_id)
                assert container_status, f"Container for model {model_id} not running"
                
                logger.info(f"Model setting passed: {model_id}")
                return {"status": "passed", "data": result}
                
        except Exception as e:
            logger.error(f"Model setting failed: {e}")
            return {"status": "failed", "error": str(e)}
    
    @pytest.mark.asyncio
    async def test_audio_transcription(self, test_helper):
        """Test audio transcription."""
        logger.info("Testing audio transcription")
        
        model_id = test_helper.config["test_models"][0]
        audio_file = test_helper.test_data_dir / test_helper.config["test_audio_files"]["short"]["filename"]
        
        try:
            # Set model first
            await test_helper.set_model(model_id)
            
            # Transcribe audio
            result = await test_helper.transcribe_audio(audio_file, model_id)
            
            assert "text" in result
            assert len(result["text"]) > 0
            assert result["model_id"] == model_id
            assert result["backend"] == "docker"
            
            logger.info(f"Audio transcription passed: {len(result['text'])} characters")
            return {"status": "passed", "data": result}
            
        except Exception as e:
            logger.error(f"Audio transcription failed: {e}")
            return {"status": "failed", "error": str(e)}
    
    @pytest.mark.asyncio
    async def test_websocket_progress_updates(self, test_helper):
        """Test WebSocket progress updates."""
        logger.info("Testing WebSocket progress updates")
        
        if not test_helper.config["websocket_tests"]["enabled"]:
            logger.info("WebSocket tests disabled, skipping")
            return {"status": "skipped", "reason": "WebSocket tests disabled"}
        
        model_id = test_helper.config["test_models"][0]
        audio_file = test_helper.test_data_dir / test_helper.config["test_audio_files"]["medium"]["filename"]
        
        try:
            # Connect to WebSocket
            websocket = await test_helper.connect_websocket()
            
            # Start transcription in background
            transcription_task = asyncio.create_task(
                test_helper.transcribe_audio(audio_file, model_id)
            )
            
            # Wait for transcription started message
            started_msg = await test_helper.wait_for_websocket_message(
                websocket, "transcription_started", timeout=10
            )
            assert started_msg is not None, "Did not receive transcription_started message"
            assert started_msg["filename"] == audio_file.name
            
            # Wait for progress updates
            progress_received = False
            for _ in range(10):  # Check for progress updates
                progress_msg = await test_helper.wait_for_websocket_message(
                    websocket, "transcription_progress", timeout=5
                )
                if progress_msg:
                    progress_received = True
                    assert "progress" in progress_msg
                    assert "status" in progress_msg
                    break
            
            assert progress_received, "Did not receive any progress updates"
            
            # Wait for completion
            completed_msg = await test_helper.wait_for_websocket_message(
                websocket, "transcription_completed", timeout=60
            )
            assert completed_msg is not None, "Did not receive transcription_completed message"
            
            # Get transcription result
            result = await transcription_task
            assert "text" in result
            
            # Close WebSocket
            await websocket.close()
            
            logger.info("WebSocket progress updates passed")
            return {"status": "passed", "data": result}
            
        except Exception as e:
            logger.error(f"WebSocket progress updates failed: {e}")
            return {"status": "failed", "error": str(e)}
    
    @pytest.mark.asyncio
    async def test_gpu_allocation(self, test_helper):
        """Test GPU allocation and utilization."""
        logger.info("Testing GPU allocation")
        
        if not test_helper.config["gpu_tests"]["enabled"]:
            logger.info("GPU tests disabled, skipping")
            return {"status": "skipped", "reason": "GPU tests disabled"}
        
        model_id = test_helper.config["test_models"][0]
        model_config = test_helper.config["model_configs"][model_id]
        gpu_memory_mb = model_config["gpu_memory_mb"]
        
        try:
            # Get initial GPU utilization
            initial_utilization = test_helper.get_gpu_utilization()
            assert initial_utilization is not None, "Failed to get initial GPU utilization"
            
            # Set model (should allocate GPU memory)
            await test_helper.set_model(model_id)
            
            # Check GPU utilization after allocation
            post_allocation_utilization = test_helper.get_gpu_utilization()
            assert post_allocation_utilization is not None, "Failed to get post-allocation GPU utilization"
            
            # Transcribe audio (should use GPU)
            audio_file = test_helper.test_data_dir / test_helper.config["test_audio_files"]["short"]["filename"]
            result = await test_helper.transcribe_audio(audio_file, model_id)
            
            # Verify transcription used GPU
            assert "container_info" in result
            assert result["container_info"]["gpu_allocated"] is True
            
            logger.info(f"GPU allocation passed: {gpu_memory_mb}MB allocated")
            return {"status": "passed", "data": result}
            
        except Exception as e:
            logger.error(f"GPU allocation failed: {e}")
            return {"status": "failed", "error": str(e)}
    
    @pytest.mark.asyncio
    async def test_concurrent_requests(self, test_helper):
        """Test concurrent request handling."""
        logger.info("Testing concurrent requests")
        
        if not test_helper.config["concurrent_tests"]["enabled"]:
            logger.info("Concurrent tests disabled, skipping")
            return {"status": "skipped", "reason": "Concurrent tests disabled"}
        
        scenario = test_helper.config["concurrent_tests"]["test_scenarios"][0]
        model_id = scenario["model"]
        
        try:
            # Set model
            await test_helper.set_model(model_id)
            
            # Prepare audio files
            audio_files = [
                test_helper.test_data_dir / filename
                for filename in scenario["files"]
            ]
            
            # Run concurrent transcriptions
            start_time = time.time()
            results = await test_helper.run_concurrent_transcriptions(audio_files, model_id)
            end_time = time.time()
            
            # Verify results
            successful_results = [r for r in results if not isinstance(r, Exception)]
            assert len(successful_results) == len(audio_files), "Some concurrent requests failed"
            
            for result in successful_results:
                assert "text" in result
                assert len(result["text"]) > 0
            
            total_time = end_time - start_time
            logger.info(f"Concurrent requests passed: {len(successful_results)} requests in {total_time:.2f}s")
            
            return {
                "status": "passed",
                "data": {
                    "total_requests": len(audio_files),
                    "successful_requests": len(successful_results),
                    "total_time": total_time
                }
            }
            
        except Exception as e:
            logger.error(f"Concurrent requests failed: {e}")
            return {"status": "failed", "error": str(e)}
    
    @pytest.mark.asyncio
    async def test_error_handling(self, test_helper):
        """Test error handling for various failure scenarios."""
        logger.info("Testing error handling")
        
        test_results = {}
        
        # Test invalid model
        try:
            error_scenario = test_helper.config["error_scenarios"]["invalid_model"]
            async with httpx.AsyncClient() as client:
                response = await client.post(
                    f"{test_helper.server_url}/v1/settings/model",
                    json={"model_id": error_scenario["model_id"]},
                    timeout=10
                )
                
                assert response.status_code == 404
                test_results["invalid_model"] = {"status": "passed"}
        except Exception as e:
            test_results["invalid_model"] = {"status": "failed", "error": str(e)}
        
        # Test invalid audio format
        try:
            error_scenario = test_helper.config["error_scenarios"]["invalid_audio_format"]
            invalid_file = test_helper.test_data_dir / error_scenario["filename"]
            
            # Create invalid file
            invalid_file.write_text("This is not an audio file")
            
            with open(invalid_file, 'rb') as f:
                files = {'file': (invalid_file.name, f, 'text/plain')}
                data = {'model': test_helper.config["test_models"][0]}
                
                async with httpx.AsyncClient() as client:
                    response = await client.post(
                        f"{test_helper.server_url}/v1/audio/transcriptions",
                        files=files,
                        data=data,
                        timeout=10
                    )
                    
                    assert response.status_code == 400
                    test_results["invalid_audio_format"] = {"status": "passed"}
        except Exception as e:
            test_results["invalid_audio_format"] = {"status": "failed", "error": str(e)}
        finally:
            # Clean up invalid file
            if invalid_file.exists():
                invalid_file.unlink()
        
        # Verify all error tests passed
        failed_tests = [name for name, result in test_results.items() if result["status"] == "failed"]
        assert len(failed_tests) == 0, f"Error handling tests failed: {failed_tests}"
        
        logger.info("Error handling passed")
        return {"status": "passed", "data": test_results}
    
    @pytest.mark.asyncio
    async def test_container_lifecycle(self, test_helper):
        """Test container lifecycle management."""
        logger.info("Testing container lifecycle")
        
        if not test_helper.config["container_lifecycle_tests"]["enabled"]:
            logger.info("Container lifecycle tests disabled, skipping")
            return {"status": "skipped", "reason": "Container lifecycle tests disabled"}
        
        model_id = test_helper.config["test_models"][0]
        
        try:
            # Test container startup
            await test_helper.set_model(model_id)
            container_running = test_helper.verify_container_status(model_id)
            assert container_running, "Container failed to start"
            
            # Test container logs
            logs = test_helper.get_container_logs(model_id)
            assert logs is not None, "Failed to get container logs"
            assert len(logs) > 0, "Container logs are empty"
            
            # Test container restart (by setting model again)
            await test_helper.set_model(model_id)
            container_still_running = test_helper.verify_container_status(model_id)
            assert container_still_running, "Container failed to restart"
            
            # Test container cleanup
            # This would normally happen after inactivity timeout
            # For testing, we'll manually stop it
            test_helper.stop_server()
            await asyncio.sleep(2)
            await test_helper.start_server()
            
            logger.info("Container lifecycle passed")
            return {"status": "passed", "data": {"logs_length": len(logs)}}
            
        except Exception as e:
            logger.error(f"Container lifecycle failed: {e}")
            return {"status": "failed", "error": str(e)}
    
    @pytest.mark.asyncio
    async def test_model_switching(self, test_helper):
        """Test switching between different models."""
        logger.info("Testing model switching")
        
        test_models = test_helper.config["test_models"][:2]  # Test first 2 models
        
        try:
            for model_id in test_models:
                # Set model
                async with httpx.AsyncClient() as client:
                    response = await client.post(
                        f"{test_helper.server_url}/v1/settings/model",
                        json={"model_id": model_id},
                        timeout=30
                    )
                    
                    assert response.status_code == 200
                    result = response.json()
                    assert result["status"] == "success"
                
                # Wait for container to be ready
                container_ready = await test_helper.wait_for_container_ready(model_id)
                assert container_ready, f"Container for model {model_id} not ready"
                
                # Test transcription with new model
                audio_file = test_helper.test_data_dir / test_helper.config["test_audio_files"]["short"]["filename"]
                transcription_result = await test_helper.transcribe_audio(audio_file, model_id)
                
                assert "text" in transcription_result
                assert transcription_result["model_id"] == model_id
            
            logger.info("Model switching passed")
            return {"status": "passed", "data": {"tested_models": test_models}}
            
        except Exception as e:
            logger.error(f"Model switching failed: {e}")
            return {"status": "failed", "error": str(e)}
    
    @pytest.mark.asyncio
    async def test_response_formats(self, test_helper):
        """Test different response formats."""
        logger.info("Testing response formats")
        
        model_id = test_helper.config["test_models"][0]
        audio_file = test_helper.test_data_dir / test_helper.config["test_audio_files"]["short"]["filename"]
        
        try:
            # Set model
            await test_helper.set_model(model_id)
            
            # Test JSON format
            json_result = await test_helper.transcribe_audio(audio_file, model_id, "json")
            assert isinstance(json_result, dict)
            assert "text" in json_result
            
            # Test text format
            text_result = await test_helper.transcribe_audio(audio_file, model_id, "text")
            assert isinstance(text_result, dict)
            assert "text" in text_result
            assert isinstance(text_result["text"], str)
            
            # Test SRT format
            srt_result = await test_helper.transcribe_audio(audio_file, model_id, "srt")
            assert isinstance(srt_result, dict)
            assert "text" in srt_result
            assert isinstance(srt_result["text"], str)
            
            logger.info("Response formats passed")
            return {"status": "passed", "data": {"formats_tested": ["json", "text", "srt"]}}
            
        except Exception as e:
            logger.error(f"Response formats failed: {e}")
            return {"status": "failed", "error": str(e)}


# Test runner function
async def run_all_tests(test_helper: IntegrationTestHelper) -> Dict[str, Any]:
    """Run all end-to-end tests."""
    logger.info("Starting end-to-end test suite")
    
    test_instance = TestE2EDockerASR()
    test_results = {}
    
    # List of all test methods
    test_methods = [
        test_instance.test_server_health_check,
        test_instance.test_list_models,
        test_instance.test_set_model,
        test_instance.test_audio_transcription,
        test_instance.test_websocket_progress_updates,
        test_instance.test_gpu_allocation,
        test_instance.test_concurrent_requests,
        test_instance.test_error_handling,
        test_instance.test_container_lifecycle,
        test_instance.test_model_switching,
        test_instance.test_response_formats
    ]
    
    # Run all tests
    for test_method in test_methods:
        test_name = test_method.__name__
        logger.info(f"Running test: {test_name}")
        
        try:
            result = await test_method(test_helper)
            test_results[test_name] = result
            
            if result["status"] == "passed":
                logger.info(f"✓ {test_name} passed")
            elif result["status"] == "skipped":
                logger.info(f"- {test_name} skipped: {result.get('reason', 'Unknown')}")
            else:
                logger.error(f"✗ {test_name} failed: {result.get('error', 'Unknown')}")
                
        except Exception as e:
            logger.error(f"✗ {test_name} failed with exception: {e}")
            test_results[test_name] = {
                "status": "failed",
                "error": str(e),
                "traceback": traceback.format_exc()
            }
    
    # Generate summary
    total_tests = len(test_results)
    passed_tests = sum(1 for r in test_results.values() if r["status"] == "passed")
    failed_tests = sum(1 for r in test_results.values() if r["status"] == "failed")
    skipped_tests = sum(1 for r in test_results.values() if r["status"] == "skipped")
    
    summary = {
        "total_tests": total_tests,
        "passed": passed_tests,
        "failed": failed_tests,
        "skipped": skipped_tests,
        "success_rate": (passed_tests / total_tests) * 100 if total_tests > 0 else 0
    }
    
    logger.info(f"Test suite completed: {passed_tests}/{total_tests} passed ({summary['success_rate']:.1f}%)")
    
    return {
        "summary": summary,
        "results": test_results,
        "timestamp": time.time()
    }


if __name__ == "__main__":
    # Run tests directly
    async def main():
        helper = IntegrationTestHelper()
        helper.setup_test_environment()
        
        try:
            if helper.start_server():
                results = await run_all_tests(helper)
                print(json.dumps(results, indent=2))
            else:
                print("Failed to start server")
        finally:
            helper.cleanup_test_environment()
    
    asyncio.run(main())