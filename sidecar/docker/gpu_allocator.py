"""
GPU resource allocation manager for Docker containers
"""

import logging
import threading
import time
from typing import Dict, Any, Optional, List
from dataclasses import dataclass
from enum import Enum

logger = logging.getLogger(__name__)

class GPUAllocationStatus(Enum):
    """Status of GPU allocation."""
    AVAILABLE = "available"
    ALLOCATED = "allocated"
    EXHAUSTED = "exhausted"
    ERROR = "error"

@dataclass
class GPUAllocation:
    """Represents a GPU memory allocation."""
    container_id: str
    memory_mb: int
    allocated_at: float
    last_activity: float

class GPUAllocator:
    """Manages GPU resource allocation for containers, optimized for RTX 4070 Super."""
    
    def __init__(self, total_memory_mb: int = 16384):
        """
        Initialize GPU allocator.
        
        Args:
            total_memory_mb: Total GPU memory in MB (default: 16384 for RTX 4070 Super)
        """
        self.total_memory_mb = total_memory_mb
        self.allocated_memory_mb = 0
        self.allocations: Dict[str, GPUAllocation] = {}
        self.lock = threading.RLock()
        self._initialize_gpu_detection()
    
    def _initialize_gpu_detection(self):
        """Initialize GPU detection and verify availability."""
        try:
            self._detect_gpu_memory()
            logger.info(f"GPU Allocator initialized with {self.total_memory_mb}MB total memory")
        except Exception as e:
            logger.error(f"Failed to initialize GPU detection: {e}")
            self.gpu_available = False
        else:
            self.gpu_available = True
    
    def _detect_gpu_memory(self) -> int:
        """
        Detect total GPU memory on NVIDIA RTX 4070 Super.
        
        Returns:
            Total GPU memory in MB
        """
        try:
            import subprocess
            import re
            
            # Try to get GPU info using nvidia-smi
            result = subprocess.run(
                ["nvidia-smi", "--query-gpu=memory.total", "--format=csv,noheader,nounits"],
                capture_output=True,
                text=True,
                timeout=10
            )
            
            if result.returncode == 0:
                # Parse memory in MB
                memory_mb = int(result.stdout.strip())
                logger.info(f"Detected GPU memory: {memory_mb}MB")
                self.total_memory_mb = memory_mb
                return memory_mb
            else:
                logger.warning("nvidia-smi failed, using default RTX 4070 Super memory (16GB)")
                return 16384
                
        except (subprocess.TimeoutExpired, subprocess.SubprocessError, FileNotFoundError) as e:
            logger.warning(f"Could not detect GPU memory: {e}, using default RTX 4070 Super memory (16GB)")
            return 16384
    
    def can_allocate(self, memory_mb: int) -> bool:
        """
        Check if GPU memory can be allocated.
        
        Args:
            memory_mb: Memory required in MB
            
        Returns:
            True if allocation is possible
        """
        with self.lock:
            if not self.gpu_available:
                logger.warning("GPU not available for allocation")
                return False
            
            available_memory = self.total_memory_mb - self.allocated_memory_mb
            return available_memory >= memory_mb
    
    def allocate_gpu_memory(self, container_id: str, memory_mb: int) -> bool:
        """
        Allocate GPU memory for a container.
        
        Args:
            container_id: Unique container identifier
            memory_mb: Memory to allocate in MB
            
        Returns:
            True if allocation was successful
        """
        with self.lock:
            if not self.gpu_available:
                logger.error("GPU not available for allocation")
                return False
            
            # Check if container already has allocation
            if container_id in self.allocations:
                logger.warning(f"Container {container_id} already has GPU allocation")
                return False
            
            # Check if enough memory is available
            if not self.can_allocate(memory_mb):
                available = self.total_memory_mb - self.allocated_memory_mb
                logger.error(f"Insufficient GPU memory: requested {memory_mb}MB, available {available}MB")
                return False
            
            # Create allocation
            allocation = GPUAllocation(
                container_id=container_id,
                memory_mb=memory_mb,
                allocated_at=time.time(),
                last_activity=time.time()
            )
            
            self.allocations[container_id] = allocation
            self.allocated_memory_mb += memory_mb
            
            logger.info(f"Allocated {memory_mb}MB GPU memory for container {container_id}")
            return True
    
    def release_gpu_memory(self, container_id: str) -> bool:
        """
        Release GPU memory for a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            True if release was successful
        """
        with self.lock:
            if container_id not in self.allocations:
                logger.warning(f"No GPU allocation found for container {container_id}")
                return False
            
            allocation = self.allocations.pop(container_id)
            self.allocated_memory_mb -= allocation.memory_mb
            
            logger.info(f"Released {allocation.memory_mb}MB GPU memory for container {container_id}")
            return True
    
    def update_container_activity(self, container_id: str) -> bool:
        """
        Update the last activity timestamp for a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            True if update was successful
        """
        with self.lock:
            if container_id not in self.allocations:
                return False
            
            self.allocations[container_id].last_activity = time.time()
            return True
    
    def get_container_allocation(self, container_id: str) -> Optional[GPUAllocation]:
        """
        Get allocation information for a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            GPU allocation or None if not found
        """
        with self.lock:
            return self.allocations.get(container_id)
    
    def get_gpu_utilization(self) -> Dict[str, Any]:
        """
        Get current GPU utilization statistics.
        
        Returns:
            Dictionary with GPU utilization information
        """
        with self.lock:
            utilization = {
                "total_memory_mb": self.total_memory_mb,
                "allocated_memory_mb": self.allocated_memory_mb,
                "available_memory_mb": self.total_memory_mb - self.allocated_memory_mb,
                "utilization_percent": (self.allocated_memory_mb / self.total_memory_mb) * 100 if self.total_memory_mb > 0 else 0,
                "active_allocations": len(self.allocations),
                "gpu_available": self.gpu_available,
                "allocations": {}
            }
            
            # Add detailed allocation information
            for container_id, allocation in self.allocations.items():
                utilization["allocations"][container_id] = {
                    "memory_mb": allocation.memory_mb,
                    "allocated_at": allocation.allocated_at,
                    "last_activity": allocation.last_activity,
                    "uptime_seconds": time.time() - allocation.allocated_at,
                    "inactive_seconds": time.time() - allocation.last_activity
                }
            
            return utilization
    
    def get_inactive_allocations(self, timeout_seconds: int) -> List[str]:
        """
        Get list of containers with allocations inactive for specified timeout.
        
        Args:
            timeout_seconds: Inactivity timeout in seconds
            
        Returns:
            List of container IDs with inactive allocations
        """
        with self.lock:
            current_time = time.time()
            inactive_containers = []
            
            for container_id, allocation in self.allocations.items():
                inactive_time = current_time - allocation.last_activity
                if inactive_time > timeout_seconds:
                    inactive_containers.append(container_id)
            
            return inactive_containers
    
    def force_cleanup_allocation(self, container_id: str) -> bool:
        """
        Force cleanup of allocation for a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            True if cleanup was successful
        """
        logger.warning(f"Force cleanup GPU allocation for container {container_id}")
        return self.release_gpu_memory(container_id)
    
    def cleanup_all_allocations(self) -> int:
        """
        Clean up all GPU allocations.
        
        Returns:
            Number of allocations cleaned up
        """
        with self.lock:
            count = len(self.allocations)
            self.allocations.clear()
            self.allocated_memory_mb = 0
            
            logger.info(f"Cleaned up {count} GPU allocations")
            return count
    
    def get_allocation_history(self, limit: int = 100) -> List[Dict[str, Any]]:
        """
        Get allocation history (placeholder for future implementation).
        
        Args:
            limit: Maximum number of history entries to return
            
        Returns:
            List of allocation history entries
        """
        # This would require persistent storage for full implementation
        return []
    
    def validate_gpu_requirements(self, memory_mb: int) -> Dict[str, Any]:
        """
        Validate GPU requirements for a container.
        
        Args:
            memory_mb: Memory required in MB
            
        Returns:
            Validation result with details
        """
        result = {
            "valid": False,
            "available": False,
            "sufficient_memory": False,
            "total_memory_mb": self.total_memory_mb,
            "allocated_memory_mb": self.allocated_memory_mb,
            "available_memory_mb": 0,
            "requested_memory_mb": memory_mb,
            "message": ""
        }
        
        if not self.gpu_available:
            result["message"] = "GPU not available"
            return result
        
        result["available"] = True
        result["available_memory_mb"] = self.total_memory_mb - self.allocated_memory_mb
        
        if self.can_allocate(memory_mb):
            result["sufficient_memory"] = True
            result["valid"] = True
            result["message"] = "GPU requirements satisfied"
        else:
            result["message"] = f"Insufficient GPU memory: requested {memory_mb}MB, available {result['available_memory_mb']}MB"
        
        return result