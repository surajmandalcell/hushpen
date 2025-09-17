"""Test model loading and management."""

import pytest
from asrpro.models import MODEL_LOADERS, BaseLoader
from asrpro.model_manager import ModelManager, MODEL_SPECS


class TestModels:
    """Test model loaders."""
    
    def test_model_loaders_exist(self):
        """Test that model loaders are registered."""
        assert len(MODEL_LOADERS) > 0
        assert 'parakeet-0.6b' in MODEL_LOADERS
        assert 'parakeet-1.1b' in MODEL_LOADERS
        assert 'whisper-medium-onnx' in MODEL_LOADERS
    
    def test_base_loader_interface(self):
        """Test BaseLoader interface."""
        loader = BaseLoader()
        
        assert hasattr(loader, 'model_name')
        assert hasattr(loader, 'device')
        assert hasattr(loader, 'load')
        assert hasattr(loader, 'transcribe_file')
    
    def test_model_specs(self):
        """Test model specifications."""
        assert len(MODEL_SPECS) > 0
        assert 'parakeet-tdt-0.6b' in MODEL_SPECS
        assert 'parakeet-tdt-1.1b' in MODEL_SPECS
        assert 'whisper-medium-onnx' in MODEL_SPECS


class TestModelManager:
    """Test model manager."""
    
    def test_model_manager_init(self):
        """Test ModelManager initialization."""
        mm = ModelManager()
        
        assert mm.current_id is None
        assert mm.current_model is None
        assert mm.device in ['cuda', 'mps', 'vulkan', 'cpu']
    
    def test_list_models(self):
        """Test listing available models."""
        mm = ModelManager()
        models = mm.list_models()
        
        assert isinstance(models, list)
        assert len(models) > 0
        
        for model in models:
            assert 'id' in model
            assert 'object' in model
            assert model['object'] == 'model'
    
    def test_device_detection(self):
        """Test device detection."""
        mm = ModelManager()
        
        # Device should be a string
        assert isinstance(mm.device, str)
        
        # Should be one of the known devices
        assert mm.device in ['cuda', 'mps', 'vulkan', 'cpu']
        
        # Platform-specific checks
        import platform
        if platform.system() == 'Darwin':
            # On macOS, should not be CUDA
            assert mm.device != 'cuda'
            # Should prefer MPS if available
            import torch
            if hasattr(torch.backends, 'mps') and torch.backends.mps.is_available():
                assert mm.device == 'mps'
