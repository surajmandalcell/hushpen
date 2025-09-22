"""
Tests for Model registry
"""

import pytest

from models import ModelRegistry


class TestModelRegistry:
    """Test cases for ModelRegistry class."""

    def test_init(self):
        """Test ModelRegistry initialization."""
        registry = ModelRegistry()
        assert registry._models is not None
        assert len(registry._models) > 0

    def test_list_models(self):
        """Test listing all available models."""
        registry = ModelRegistry()
        models = registry.list_models()

        assert isinstance(models, list)
        assert len(models) > 0
        assert "whisper-tiny" in models
        assert "whisper-base" in models
        assert "whisper-large" in models
        assert "whisper-tiny-local" in models
        assert "whisper-base-local" in models
        assert "parakeet-tdt-0.6b-v2" in models

    def test_get_model_info_existing(self):
        """Test getting model info for existing model."""
        registry = ModelRegistry()
        info = registry.get_model_info("whisper-base")

        assert info is not None
        assert info["id"] == "whisper-base"
        assert info["name"] == "Whisper Base (ONNX)"
        assert info["type"] == "onnx"
        assert info["size"] == "base"
        assert info["family"] == "whisper"
        assert "en" in info["languages"]
        assert info["sample_rate"] == 16000

    def test_get_model_info_nonexistent(self):
        """Test getting model info for non-existent model."""
        registry = ModelRegistry()
        info = registry.get_model_info("nonexistent-model")

        assert info is None

    def test_get_models_by_type_whisper(self):
        """Test getting models by type (Whisper)."""
        registry = ModelRegistry()
        whisper_models = registry.get_models_by_type("whisper")

        assert len(whisper_models) == 5  # 5 Whisper models (including local variants)
        for model_id in whisper_models:
            model_info = registry.get_model_info(model_id)
            assert model_info["family"] == "whisper"
            assert model_info["loader"] == "config"

    def test_get_models_by_type_parakeet(self):
        """Test getting models by type (Parakeet)."""
        registry = ModelRegistry()
        parakeet_models = registry.get_models_by_type("parakeet")

        assert len(parakeet_models) == 1  # 1 Parakeet model
        for model_id in parakeet_models:
            model_info = registry.get_model_info(model_id)
            assert model_info["family"] == "parakeet"
            assert model_info["loader"] == "config"

    def test_get_models_by_language_english(self):
        """Test getting models by language (English)."""
        registry = ModelRegistry()
        en_models = registry.get_models_by_language("en")

        assert len(en_models) == 6  # All 6 models support English
        for model_id in en_models:
            model_info = registry.get_model_info(model_id)
            assert "en" in model_info["languages"]

    def test_get_models_by_language_spanish(self):
        """Test getting models by language (Spanish)."""
        registry = ModelRegistry()
        es_models = registry.get_models_by_language("es")

        assert len(es_models) == 0  # No models support Spanish in current registry
        for model in es_models:
            assert "es" in model["languages"]
            assert model["type"] == "whisper"

    def test_get_models_by_language_nonexistent(self):
        """Test getting models by non-existent language."""
        registry = ModelRegistry()
        models = registry.get_models_by_language("xyz")

        assert len(models) == 0

    def test_is_model_available_existing(self):
        """Test checking availability of existing model."""
        registry = ModelRegistry()

        assert registry.is_model_available("whisper-base") is True
        assert registry.is_model_available("parakeet-tdt-0.6b-v2") is True

    def test_is_model_available_nonexistent(self):
        """Test checking availability of non-existent model."""
        registry = ModelRegistry()

        assert registry.is_model_available("nonexistent-model") is False

    def test_get_loader_type_whisper(self):
        """Test getting loader type for Whisper model."""
        registry = ModelRegistry()

        loader_type = registry.get_loader_type("whisper-base")
        assert loader_type == "whisper"

    def test_get_loader_type_parakeet(self):
        """Test getting loader type for Parakeet model."""
        registry = ModelRegistry()

        loader_type = registry.get_loader_type("parakeet-tdt-0.6b-v2")
        assert loader_type == "parakeet"

    def test_get_loader_type_nonexistent(self):
        """Test getting loader type for non-existent model."""
        registry = ModelRegistry()

        loader_type = registry.get_loader_type("nonexistent-model")
        assert loader_type is None

    def test_model_properties_whisper(self):
        """Test properties of Whisper models."""
        registry = ModelRegistry()

        # Test tiny model
        tiny_info = registry.get_model_info("whisper-tiny")
        assert tiny_info["size"] == "tiny"
        assert tiny_info["description"] == "OpenAI Whisper tiny model - fast & lightweight - ONNX"

        # Test large model
        large_info = registry.get_model_info("whisper-large")
        assert large_info["size"] == "large"
        assert large_info["description"] == "OpenAI Whisper large model - ONNX"

    def test_model_properties_parakeet(self):
        """Test properties of Parakeet models."""
        registry = ModelRegistry()

        # Test TDT model
        tdt_info = registry.get_model_info("parakeet-tdt-0.6b-v2")
        assert tdt_info is not None
        assert tdt_info["family"] == "parakeet"
        assert tdt_info["description"] == "NVIDIA Parakeet TDT model (0.6B parameters) - English/Hindi - ONNX"
