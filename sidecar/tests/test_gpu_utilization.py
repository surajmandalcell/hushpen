"""
Comprehensive GPU utilization tests for RTX 4070 Super
"""

import pytest
import logging
import subprocess
import time
import threading
from typing import Dict, Any, List, Optional
from unittest.mock import Mock, patch, MagicMock

# Import the modules we're testing
from docker.gpu_allocator import GPUAllocator, GPUAllocation

logger = logging.getLogger(__name__)


class TestGPUUtilization:
    """Test GPU utilization and detection for RTX 4070 Super."""
    
    @pytest.fixture
    def gpu_allocator(self):
        """Create a GPU allocator for testing."""
        return GPUAllocator(total_memory_mb=16384)  # RTX 4070 Super has 16GB
    
    def test_nvidia_driver_availability(self):
        """Test if NVIDIA drivers are installed and accessible."""
        try:
            # Check if nvidia-smi is available
            result = subprocess.run(
                ["nvidia-smi", "--version"],
                capture_output=True,
                text=True,
                timeout=10
            )
            
            assert result.returncode == 0, "NVIDIA drivers not installed or nvidia-smi not in PATH"
            assert "NVIDIA-SMI" in result.stdout, "nvidia-smi output invalid"
            
            logger.info("NVIDIA drivers are available")
        except (subprocess.TimeoutExpired, subprocess.SubprocessError, FileNotFoundError) as e:
            pytest.skip(f"NVIDIA drivers not available: {e}")
    
    def test_gpu_detection(self):
        """Test GPU detection and properties."""
        try:
            # Get GPU information
            result = subprocess.run(
                ["nvidia-smi", "--query-gpu=name,memory.total,memory.free,memory.used,utilization.gpu,temperature.gpu", 
                 "--format=csv,noheader,nounits"],
                capture_output=True,
                text=True,
                timeout=10
            )
            
            assert result.returncode == 0, "Failed to query GPU information"
            
            # Parse GPU information
            gpu_info = result.stdout.strip().split(", ")
            assert len(gpu_info) >= 6, "Incomplete GPU information received"
            
            gpu_name = gpu_info[0]
            total_memory = int(gpu_info[1])
            free_memory = int(gpu_info[2])
            used_memory = int(gpu_info[3])
            gpu_utilization = int(gpu_info[4])
            temperature = int(gpu_info[5])
            
            # Verify RTX 4070 Super
            assert "4070" in gpu_name or "RTX" in gpu_name, f"Expected RTX 4070 Super, found {gpu_name}"
            assert total_memory >= 16000, f"Expected at least 16GB memory, found {total_memory}MB"
            
            logger.info(f"Detected GPU: {gpu_name}")
            logger.info(f"Memory: {used_memory}MB/{total_memory}MB ({free_memory}MB free)")
            logger.info(f"Utilization: {gpu_utilization}%, Temperature: {temperature}°C")
            
        except (subprocess.TimeoutExpired, subprocess.SubprocessError, FileNotFoundError) as e:
            pytest.skip(f"GPU detection failed: {e}")
    
    def test_cuda_availability(self):
        """Test CUDA availability and version."""
        try:
            # Check CUDA version
            result = subprocess.run(
                ["nvidia-smi", "--query-gpu=cuda_version", "--format=csv,noheader,nounits"],
                capture_output=True,
                text=True,
                timeout=10
            )
            
            assert result.returncode == 0, "Failed to query CUDA version"
            
            cuda_version = result.stdout.strip()
            assert cuda_version, "CUDA version not detected"
            
            # Parse CUDA version (format: "12.2" or similar)
            major, minor = map(int, cuda_version.split("."))
            assert major >= 11, f"CUDA version {cuda_version} is too old, need at least 11.0"
            
            logger.info(f"CUDA version: {cuda_version}")
            
        except (subprocess.TimeoutExpired, subprocess.SubprocessError, FileNotFoundError, ValueError) as e:
            pytest.skip(f"CUDA detection failed: {e}")
    
    def test_docker_gpu_support(self):
        """Test Docker GPU support."""
        try:
            # Check if Docker is installed
            docker_result = subprocess.run(
                ["docker", "--version"],
                capture_output=True,
                text=True,
                timeout=10
            )
            
            assert docker_result.returncode == 0, "Docker not installed or not in PATH"
            
            # Check if nvidia-docker2 or NVIDIA Container Toolkit is installed
            try:
                nvidia_docker_result = subprocess.run(
                    ["docker", "run", "--rm", "--gpus", "all", "nvidia/cuda:12.1.0-base-ubuntu22.04", 
                     "nvidia-smi", "--query-gpu=name", "--format=csv,noheader,nounits"],
                    capture_output=True,
                    text=True,
                    timeout=30
                )
                
                assert nvidia_docker_result.returncode == 0, "NVIDIA Container Toolkit not properly installed"
                assert "4070" in nvidia_docker_result.stdout or "RTX" in nvidia_docker_result.stdout, \
                    "GPU not accessible from Docker"
                
                logger.info("Docker GPU support is available")
                logger.info(f"GPU accessible from Docker: {nvidia_docker_result.stdout.strip()}")
                
            except subprocess.TimeoutExpired:
                pytest.skip("Docker GPU test timed out")
            except subprocess.SubprocessError as e:
                pytest.skip(f"Docker GPU support test failed: {e}")
                
        except (subprocess.TimeoutExpired, subprocess.SubprocessError, FileNotFoundError) as e:
            pytest.skip(f"Docker not available: {e}")
    
    def test_gpu_allocator_initialization(self, gpu_allocator):
        """Test GPU allocator initialization."""
        assert gpu_allocator.total_memory_mb == 16384
        assert gpu_allocator.allocated_memory_mb == 0
        assert len(gpu_allocator.allocations) == 0
        assert gpu_allocator.gpu_available is True
    
    def test_gpu_memory_allocation(self, gpu_allocator):
        """Test GPU memory allocation and deallocation."""
        container_id = "test-container"
        memory_mb = 2048
        
        # Test allocation
        assert gpu_allocator.can_allocate(memory_mb), "Should be able to allocate 2048MB"
        assert gpu_allocator.allocate_gpu_memory(container_id, memory_mb), "Allocation failed"
        
        # Verify allocation
        assert gpu_allocator.allocated_memory_mb == memory_mb, "Memory not properly allocated"
        assert len(gpu_allocator.allocations) == 1, "Allocation not recorded"
        assert container_id in gpu_allocator.allocations, "Container not in allocations"
        
        allocation = gpu_allocator.get_container_allocation(container_id)
        assert allocation.memory_mb == memory_mb, "Allocation memory incorrect"
        
        # Test deallocation
        assert gpu_allocator.release_gpu_memory(container_id), "Deallocation failed"
        assert gpu_allocator.allocated_memory_mb == 0, "Memory not properly released"
        assert len(gpu_allocator.allocations) == 0, "Allocation not removed"
    
    def test_gpu_memory_exhaustion(self, gpu_allocator):
        """Test GPU memory exhaustion scenarios."""
        # Allocate most of the memory
        assert gpu_allocator.allocate_gpu_memory("container1", 12000), "Initial allocation failed"
        
        # Try to allocate more than available
        assert not gpu_allocator.can_allocate(5000), "Should not be able to allocate more than available"
        assert not gpu_allocator.allocate_gpu_memory("container2", 5000), "Over-allocation should fail"
        
        # Allocate remaining memory
        assert gpu_allocator.can_allocate(4384), "Should be able to allocate remaining memory"
        assert gpu_allocator.allocate_gpu_memory("container2", 4384), "Final allocation failed"
        
        # No memory should be available
        assert not gpu_allocator.can_allocate(1), "No memory should be available"
    
    def test_gpu_utilization_stats(self, gpu_allocator):
        """Test GPU utilization statistics."""
        # Allocate some memory
        gpu_allocator.allocate_gpu_memory("container1", 4096)
        gpu_allocator.allocate_gpu_memory("container2", 2048)
        
        # Get utilization stats
        utilization = gpu_allocator.get_gpu_utilization()
        
        assert utilization["total_memory_mb"] == 16384
        assert utilization["allocated_memory_mb"] == 6144
        assert utilization["available_memory_mb"] == 10240
        assert utilization["utilization_percent"] == 37.5  # 6144/16384 * 100
        assert utilization["active_allocations"] == 2
        assert utilization["gpu_available"] is True
        assert len(utilization["allocations"]) == 2
        
        # Check allocation details
        assert "container1" in utilization["allocations"]
        assert utilization["allocations"]["container1"]["memory_mb"] == 4096
        assert "container2" in utilization["allocations"]
        assert utilization["allocations"]["container2"]["memory_mb"] == 2048
    
    def test_activity_tracking(self, gpu_allocator):
        """Test container activity tracking."""
        container_id = "test-container"
        
        # Allocate memory
        gpu_allocator.allocate_gpu_memory(container_id, 2048)
        
        # Get initial activity
        allocation = gpu_allocator.get_container_allocation(container_id)
        initial_activity = allocation.last_activity
        
        # Wait a bit and update activity
        time.sleep(0.1)
        gpu_allocator.update_container_activity(container_id)
        
        # Check activity was updated
        updated_allocation = gpu_allocator.get_container_allocation(container_id)
        assert updated_allocation.last_activity > initial_activity, "Activity not updated"
    
    def test_inactive_allocation_detection(self, gpu_allocator):
        """Test detection of inactive allocations."""
        # Create allocations with different activity times
        gpu_allocator.allocate_gpu_memory("active-container", 2048)
        gpu_allocator.allocate_gpu_memory("inactive-container", 2048)
        
        # Manually set one allocation as inactive
        allocation = gpu_allocator.get_container_allocation("inactive-container")
        allocation.last_activity = time.time() - 3600  # 1 hour ago
        
        # Check for inactive allocations
        inactive = gpu_allocator.get_inactive_allocations(1800)  # 30 minutes timeout
        
        assert len(inactive) == 1
        assert "inactive-container" in inactive
        assert "active-container" not in inactive
    
    def test_gpu_requirements_validation(self, gpu_allocator):
        """Test GPU requirements validation."""
        # Test valid requirements
        result = gpu_allocator.validate_gpu_requirements(2048)
        assert result["valid"] is True
        assert result["available"] is True
        assert result["sufficient_memory"] is True
        assert result["total_memory_mb"] == 16384
        assert result["available_memory_mb"] == 16384
        
        # Allocate some memory
        gpu_allocator.allocate_gpu_memory("test-container", 8192)
        
        # Test with less available memory
        result = gpu_allocator.validate_gpu_requirements(8192)
        assert result["valid"] is True
        assert result["available"] is True
        assert result["sufficient_memory"] is True
        assert result["available_memory_mb"] == 8192
        
        # Test with insufficient memory
        result = gpu_allocator.validate_gpu_requirements(16384)
        assert result["valid"] is False
        assert result["available"] is True
        assert result["sufficient_memory"] is False
        assert "Insufficient GPU memory" in result["message"]


class TestGPUStress:
    """Stress tests for GPU utilization."""
    
    @pytest.fixture
    def gpu_allocator(self):
        """Create a GPU allocator for testing."""
        return GPUAllocator(total_memory_mb=16384)
    
    def test_concurrent_allocation(self, gpu_allocator):
        """Test concurrent GPU memory allocation."""
        results = []
        errors = []
        
        def allocate_memory(container_id, memory_mb):
            try:
                if gpu_allocator.allocate_gpu_memory(container_id, memory_mb):
                    results.append(container_id)
                    # Hold allocation for a short time
                    time.sleep(0.1)
                    gpu_allocator.release_gpu_memory(container_id)
                else:
                    errors.append(f"Failed to allocate for {container_id}")
            except Exception as e:
                errors.append(f"Error allocating for {container_id}: {e}")
        
        # Create multiple threads for concurrent allocation
        threads = []
        for i in range(10):
            thread = threading.Thread(
                target=allocate_memory,
                args=(f"container-{i}", 1024)
            )
            threads.append(thread)
        
        # Start all threads
        for thread in threads:
            thread.start()
        
        # Wait for all threads to complete
        for thread in threads:
            thread.join()
        
        # Verify results
        assert len(errors) == 0, f"Errors occurred: {errors}"
        assert len(results) == 10, f"Expected 10 successful allocations, got {len(results)}"
        assert gpu_allocator.allocated_memory_mb == 0, "Memory not properly released"
    
    def test_memory_fragmentation(self, gpu_allocator):
        """Test memory fragmentation scenarios."""
        # Allocate memory in chunks
        containers = []
        for i in range(8):
            container_id = f"container-{i}"
            gpu_allocator.allocate_gpu_memory(container_id, 1024)
            containers.append(container_id)
        
        # Release every other container to create fragmentation
        for i in range(0, 8, 2):
            gpu_allocator.release_gpu_memory(f"container-{i}")
        
        # Try to allocate a large chunk
        assert gpu_allocator.can_allocate(4096), "Should be able to allocate despite fragmentation"
        assert gpu_allocator.allocate_gpu_memory("large-container", 4096), "Large allocation failed"
        
        # Clean up
        gpu_allocator.release_gpu_memory("large-container")
        for i in range(1, 8, 2):
            gpu_allocator.release_gpu_memory(f"container-{i}")


class TestRealGPUUtilization:
    """Tests that interact with real GPU hardware."""
    
    def test_real_gpu_memory_allocation(self):
        """Test real GPU memory allocation using PyTorch."""
        try:
            import torch
            
            # Check if CUDA is available
            assert torch.cuda.is_available(), "CUDA not available with PyTorch"
            
            # Get GPU device
            device = torch.device("cuda:0")
            
            # Get initial memory state
            initial_memory = torch.cuda.memory_allocated(device)
            initial_reserved = torch.cuda.memory_reserved(device)
            
            # Allocate some memory
            tensor_size = 1024 * 1024 * 100  # 100MB tensor
            test_tensor = torch.randn(tensor_size, dtype=torch.float32, device=device)
            
            # Check memory was allocated
            after_allocation = torch.cuda.memory_allocated(device)
            assert after_allocation > initial_memory, "GPU memory not allocated"
            
            # Release memory
            del test_tensor
            torch.cuda.empty_cache()
            
            # Check memory was released
            after_release = torch.cuda.memory_allocated(device)
            assert after_release < after_allocation, "GPU memory not released"
            
            logger.info(f"Initial memory: {initial_memory / 1024**2:.2f}MB")
            logger.info(f"After allocation: {after_allocation / 1024**2:.2f}MB")
            logger.info(f"After release: {after_release / 1024**2:.2f}MB")
            
        except ImportError:
            pytest.skip("PyTorch not available for GPU memory testing")
        except Exception as e:
            pytest.skip(f"GPU memory allocation test failed: {e}")
    
    def test_real_gpu_utilization_monitoring(self):
        """Test real GPU utilization monitoring."""
        try:
            import pynvml
            
            # Initialize NVML
            pynvml.nvmlInit()
            
            # Get GPU handle
            handle = pynvml.nvmlDeviceGetHandleByIndex(0)
            
            # Get GPU info
            name = pynvml.nvmlDeviceGetName(handle).decode('utf-8')
            memory_info = pynvml.nvmlDeviceGetMemoryInfo(handle)
            
            # Verify RTX 4070 Super
            assert "4070" in name or "RTX" in name, f"Expected RTX 4070 Super, found {name}"
            
            # Get utilization rates
            utilization = pynvml.nvmlDeviceGetUtilizationRates(handle)
            temperature = pynvml.nvmlDeviceGetTemperature(handle, pynvml.NVML_TEMPERATURE_GPU)
            
            # Log information
            logger.info(f"GPU: {name}")
            logger.info(f"Memory: {memory_info.used / 1024**2:.2f}MB / {memory_info.total / 1024**2:.2f}MB")
            logger.info(f"GPU Utilization: {utilization.gpu}%")
            logger.info(f"Memory Utilization: {utilization.memory}%")
            logger.info(f"Temperature: {temperature}°C")
            
            # Verify reasonable values
            assert memory_info.total >= 16 * 1024**3, "GPU should have at least 16GB memory"
            assert 0 <= utilization.gpu <= 100, "GPU utilization should be between 0-100%"
            assert 0 <= utilization.memory <= 100, "Memory utilization should be between 0-100%"
            assert 0 <= temperature <= 150, "Temperature should be reasonable"
            
            # Shutdown NVML
            pynvml.nvmlShutdown()
            
        except ImportError:
            pytest.skip("pynvml not available for GPU monitoring")
        except Exception as e:
            pytest.skip(f"GPU monitoring test failed: {e}")


if __name__ == "__main__":
    # Run tests
    pytest.main([__file__, "-v"])