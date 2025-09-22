"""
Tests for Device detection utilities
"""

import pytest
from unittest.mock import patch, Mock
import platform

from utils.device import DeviceDetector

class TestDeviceDetector:
    """Test cases for DeviceDetector class."""
    
    def test_init(self):
        """Test DeviceDetector initialization."""
        detector = DeviceDetector()
        
        assert detector.system == platform.system()
        assert detector.device_info["system"] == platform.system()
        assert detector.device_info["device"] == "cpu"
        assert detector.device_info["compute_type"] == "float32"
        assert detector.device_info["cuda_available"] is False
        assert detector.device_info["mps_available"] is False
        assert detector.device_info["device_name"] == "CPU"
    
    @pytest.mark.asyncio
    async def test_detect_capabilities_cuda_available(self):
        """Test device detection with CUDA available."""
        detector = DeviceDetector()
        
        mock_torch = Mock()
        mock_torch.cuda.is_available.return_value = True
        mock_torch.cuda.device_count.return_value = 1
        mock_torch.cuda.get_device_name.return_value = "NVIDIA GeForce RTX 3080"
        
        with patch.dict('sys.modules', {'torch': mock_torch}):
            await detector.detect_capabilities()
            
            assert detector.device_info["cuda_available"] is True
            assert detector.device_info["cuda_device_count"] == 1
            assert detector.device_info["cuda_device_name"] == "NVIDIA GeForce RTX 3080"
            assert detector.device_info["device"] == "cuda"
            assert detector.device_info["compute_type"] == "float16"
    
    @pytest.mark.asyncio
    async def test_detect_capabilities_mps_available(self):
        """Test device detection with MPS available."""
        detector = DeviceDetector()
        
        mock_torch = Mock()
        mock_torch.backends.mps.is_available.return_value = True
        
        with patch('platform.system', return_value='Darwin'):
            with patch.dict('sys.modules', {'torch': mock_torch}):
                await detector.detect_capabilities()
                
                assert detector.device_info["mps_available"] is True
                assert detector.device_info["device"] == "mps"
                assert detector.device_info["compute_type"] == "float16"
    
    @pytest.mark.asyncio
    async def test_detect_capabilities_cpu_only(self):
        """Test device detection with CPU only."""
        detector = DeviceDetector()
        
        with patch.dict('sys.modules', {}, clear=True):
            await detector.detect_capabilities()
            
            assert detector.device_info["cuda_available"] is False
            assert detector.device_info["mps_available"] is False
            assert detector.device_info["device"] == "cpu"
            assert detector.device_info["compute_type"] == "float32"
    
    @pytest.mark.asyncio
    async def test_detect_capabilities_priority_cuda_over_mps(self):
        """Test that CUDA is prioritized over MPS."""
        detector = DeviceDetector()
        
        mock_torch = Mock()
        mock_torch.cuda.is_available.return_value = True
        mock_torch.cuda.device_count.return_value = 1
        mock_torch.cuda.get_device_name.return_value = "NVIDIA GPU"
        mock_torch.backends.mps.is_available.return_value = True
        
        with patch('platform.system', return_value='Darwin'):
            with patch.dict('sys.modules', {'torch': mock_torch}):
                await detector.detect_capabilities()
                
                assert detector.device_info["cuda_available"] is True
                assert detector.device_info["mps_available"] is True
                assert detector.device_info["device"] == "cuda"  # CUDA should be prioritized
                assert detector.device_info["compute_type"] == "float16"
    
    def test_get_device_config(self):
        """Test getting device configuration."""
        detector = DeviceDetector()
        detector.device_info["device"] = "cuda"
        detector.device_info["compute_type"] = "float16"
        
        config = detector.get_device_config()
        
        assert config["device"] == "cuda"
        assert config["compute_type"] == "float16"
    
    def test_get_current_device(self):
        """Test getting current device."""
        detector = DeviceDetector()
        detector.device_info["device"] = "mps"
        
        device = detector.get_current_device()
        assert device == "mps"
    
    def test_get_device_info(self):
        """Test getting full device information."""
        detector = DeviceDetector()
        detector.device_info["device"] = "cuda"
        detector.device_info["cuda_available"] = True
        
        info = detector.get_device_info()
        
        assert info["device"] == "cuda"
        assert info["cuda_available"] is True
        assert info is not detector.device_info  # Should be a copy
    
    def test_is_cuda_available(self):
        """Test CUDA availability check."""
        detector = DeviceDetector()
        detector.device_info["cuda_available"] = True
        
        assert detector.is_cuda_available() is True
        
        detector.device_info["cuda_available"] = False
        assert detector.is_cuda_available() is False
    
    def test_is_mps_available(self):
        """Test MPS availability check."""
        detector = DeviceDetector()
        detector.device_info["mps_available"] = True
        
        assert detector.is_mps_available() is True
        
        detector.device_info["mps_available"] = False
        assert detector.is_mps_available() is False
    
    def test_get_device_name(self):
        """Test getting device name."""
        detector = DeviceDetector()
        detector.device_info["device_name"] = "NVIDIA GeForce RTX 3080"
        
        name = detector.get_device_name()
        assert name == "NVIDIA GeForce RTX 3080"
