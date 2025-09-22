"""
Tests for Model manager
"""

import pytest
from unittest.mock import Mock, AsyncMock, patch

from models import ModelManager


class TestModelManager:
    """Test cases for ModelManager class."""

    @pytest.mark.asyncio
    async def test_init(self, settings):
        """Test ModelManager initialization."""
        manager = ModelManager(settings)

        assert manager.settings == settings
        assert manager.registry is not None
        assert manager.device_detector is not None
        assert manager.current_model is None
        assert manager.current_loader is None
        assert manager.loaders == {}

    @pytest.mark.asyncio
    async def test_initialize(self, settings):
        """Test model manager initialization."""
        manager = ModelManager(settings)

        with patch.object(
            manager.device_detector, "detect_capabilities", new_callable=AsyncMock
        ) as mock_detect:
            await manager.initialize()

            mock_detect.assert_called_once()
            assert hasattr(manager, "loader_configs")
            assert "whisper" in manager.loader_configs
            assert "parakeet" in manager.loader_configs

    @pytest.mark.asyncio
    async def test_list_available_models(self, settings):
        """Test listing available models."""
        manager = ModelManager(settings)

        with patch.object(
            manager.registry,
            "list_models",
            return_value=["whisper-base", "parakeet-ctc"],
        ):
            models = await manager.list_available_models()

            assert models == ["whisper-base", "parakeet-ctc"]

    def test_get_current_model(self, settings):
        """Test getting current model."""
        manager = ModelManager(settings)

        # No model initially
        assert manager.get_current_model() is None

        # Set current model
        manager.current_model = "whisper-base"
        assert manager.get_current_model() == "whisper-base"

    def test_get_current_device(self, settings):
        """Test getting current device."""
        manager = ModelManager(settings)

        with patch.object(
            manager.device_detector, "get_current_device", return_value="cuda"
        ):
            device = manager.get_current_device()
            assert device == "cuda"

    def test_is_model_ready_current_model(self, settings):
        """Test checking if current model is ready."""
        manager = ModelManager(settings)
        manager.current_model = "whisper-base"
        manager.current_loader = Mock()
        manager.current_loader.is_ready.return_value = True

        assert manager.is_model_ready("whisper-base") is True
        assert manager.is_model_ready("whisper-small") is False

    def test_is_model_ready_no_current_model(self, settings):
        """Test checking model readiness when no current model."""
        manager = ModelManager(settings)

        assert manager.is_model_ready("whisper-base") is False

    @pytest.mark.asyncio
    async def test_set_model_success(self, settings):
        """Test successful model setting."""
        manager = ModelManager(settings)

        # Mock registry
        with patch.object(manager.registry, "is_model_available", return_value=True):
            with patch.object(
                manager.registry, "get_loader_type", return_value="whisper"
            ):
                with patch.object(
                    manager.registry,
                    "get_model_info",
                    return_value={"id": "whisper-base"},
                ):
                    with patch.object(
                        manager, "_get_loader", return_value=Mock()
                    ) as mock_get_loader:
                        mock_loader = Mock()
                        mock_loader.load = AsyncMock(return_value=True)
                        mock_get_loader.return_value = mock_loader

                        result = await manager.set_model("whisper-base")

                        assert result is True
                        assert manager.current_model == "whisper-base"
                        assert manager.current_loader == mock_loader
                        mock_loader.load.assert_called_once()

    @pytest.mark.asyncio
    async def test_set_model_not_available(self, settings):
        """Test setting model that's not available."""
        manager = ModelManager(settings)

        with patch.object(manager.registry, "is_model_available", return_value=False):
            result = await manager.set_model("nonexistent-model")

            assert result is False

    @pytest.mark.asyncio
    async def test_set_model_already_loaded(self, settings):
        """Test setting model that's already loaded."""
        manager = ModelManager(settings)
        manager.current_model = "whisper-base"
        manager.current_loader = Mock()
        manager.current_loader.is_ready.return_value = True

        with patch.object(manager.registry, "is_model_available", return_value=True):
            result = await manager.set_model("whisper-base")

            assert result is True

    @pytest.mark.asyncio
    async def test_set_model_load_failure(self, settings):
        """Test setting model when loading fails."""
        manager = ModelManager(settings)

        with patch.object(manager.registry, "is_model_available", return_value=True):
            with patch.object(
                manager.registry, "get_loader_type", return_value="whisper"
            ):
                with patch.object(
                    manager.registry,
                    "get_model_info",
                    return_value={"id": "whisper-base"},
                ):
                    with patch.object(
                        manager, "_get_loader", return_value=Mock()
                    ) as mock_get_loader:
                        mock_loader = Mock()
                        mock_loader.load = AsyncMock(return_value=False)
                        mock_get_loader.return_value = mock_loader

                        result = await manager.set_model("whisper-base")

                        assert result is False

    @pytest.mark.asyncio
    async def test_get_loader_existing(self, settings):
        """Test getting existing loader."""
        manager = ModelManager(settings)
        mock_loader = Mock()
        manager.loaders["whisper-base"] = mock_loader

        loader = await manager._get_loader("whisper-base")

        assert loader == mock_loader

    @pytest.mark.asyncio
    async def test_get_loader_new_whisper(self, settings):
        """Test creating new Whisper loader."""
        manager = ModelManager(settings)
        manager.loader_configs = {
            "whisper": {"device": "cpu", "compute_type": "float32"}
        }

        with patch.object(manager.registry, "get_loader_type", return_value="whisper"):
            with patch.object(
                manager.registry, "get_model_info", return_value={"id": "whisper-base"}
            ):
                with patch("models.manager.ConfigDrivenLoader") as mock_whisper_class:
                    mock_loader = Mock()
                    mock_whisper_class.return_value = mock_loader

                    loader = await manager._get_loader("whisper-base")

                    assert loader == mock_loader
                    assert "whisper-base" in manager.loaders
                    mock_whisper_class.assert_called_once()

    @pytest.mark.asyncio
    async def test_get_loader_new_parakeet(self, settings):
        """Test creating new Parakeet loader."""
        manager = ModelManager(settings)
        manager.loader_configs = {"parakeet": {"device": "cpu"}}

        with patch.object(manager.registry, "get_loader_type", return_value="parakeet"):
            with patch.object(
                manager.registry, "get_model_info", return_value={"id": "parakeet-ctc"}
            ):
                with patch("models.manager.ConfigDrivenLoader") as mock_parakeet_class:
                    mock_loader = Mock()
                    mock_parakeet_class.return_value = mock_loader

                    loader = await manager._get_loader("parakeet-ctc")

                    assert loader == mock_loader
                    assert "parakeet-ctc" in manager.loaders
                    mock_parakeet_class.assert_called_once()

    @pytest.mark.asyncio
    async def test_get_loader_unknown_type(self, settings):
        """Test getting loader for unknown type."""
        manager = ModelManager(settings)

        with patch.object(manager.registry, "get_loader_type", return_value="unknown"):
            loader = await manager._get_loader("unknown-model")

            assert loader is None

    @pytest.mark.asyncio
    async def test_transcribe_file_success(self, settings, sample_audio_file):
        """Test successful file transcription."""
        manager = ModelManager(settings)
        manager.current_model = "whisper-base"
        manager.current_loader = Mock()
        manager.current_loader.transcribe = AsyncMock(
            return_value={"text": "Hello world"}
        )

        with open(sample_audio_file, "rb") as f:
            result = await manager.transcribe_file(f)

        assert result["text"] == "Hello world"
        manager.current_loader.transcribe.assert_called_once()

    @pytest.mark.asyncio
    async def test_transcribe_file_no_model(self, settings, sample_audio_file):
        """Test transcription with no model loaded."""
        manager = ModelManager(settings)

        with open(sample_audio_file, "rb") as f:
            with pytest.raises(Exception, match="No model specified or loaded"):
                await manager.transcribe_file(f)

    @pytest.mark.asyncio
    async def test_transcribe_file_different_model(self, settings, sample_audio_file):
        """Test transcription with different model than current."""
        manager = ModelManager(settings)
        manager.current_model = "whisper-base"

        mock_loader = Mock()
        mock_loader.transcribe = AsyncMock(return_value={"text": "test transcription"})
        
        with patch.object(manager, "set_model", return_value=True) as mock_set_model:
            with patch.object(manager, "_get_loader", return_value=mock_loader):
                manager.current_loader = mock_loader  # Ensure loader is set
                with open(sample_audio_file, "rb") as f:
                    result = await manager.transcribe_file(f, "whisper-small")

            mock_set_model.assert_called_once_with("whisper-small")

    @pytest.mark.asyncio
    async def test_unload_model_success(self, settings):
        """Test successful model unloading."""
        manager = ModelManager(settings)
        mock_loader = Mock()
        mock_loader.unload = AsyncMock(return_value=True)
        manager.loaders["whisper-base"] = mock_loader
        manager.current_model = "whisper-base"
        manager.current_loader = mock_loader

        result = await manager.unload_model("whisper-base")

        assert result is True
        assert "whisper-base" not in manager.loaders
        assert manager.current_model is None
        assert manager.current_loader is None
        mock_loader.unload.assert_called_once()

    @pytest.mark.asyncio
    async def test_unload_model_not_loaded(self, settings):
        """Test unloading model that's not loaded."""
        manager = ModelManager(settings)

        result = await manager.unload_model("whisper-base")

        assert result is False

    @pytest.mark.asyncio
    async def test_unload_all_models(self, settings):
        """Test unloading all models."""
        manager = ModelManager(settings)
        mock_loader1 = Mock()
        mock_loader1.unload = AsyncMock(return_value=True)
        mock_loader2 = Mock()
        mock_loader2.unload = AsyncMock(return_value=True)

        manager.loaders = {"whisper-base": mock_loader1, "parakeet-ctc": mock_loader2}
        manager.current_model = "whisper-base"
        manager.current_loader = mock_loader1

        result = await manager.unload_all_models()

        assert result is True
        assert len(manager.loaders) == 0
        assert manager.current_model is None
        assert manager.current_loader is None
        mock_loader1.unload.assert_called_once()
        mock_loader2.unload.assert_called_once()

    @pytest.mark.asyncio
    async def test_get_model_info_loaded(self, settings):
        """Test getting info for loaded model."""
        manager = ModelManager(settings)
        mock_loader = Mock()
        mock_loader.get_model_info.return_value = {"id": "whisper-base", "loaded": True}
        manager.loaders["whisper-base"] = mock_loader

        info = await manager.get_model_info("whisper-base")

        assert info["id"] == "whisper-base"
        assert info["loaded"] is True
        mock_loader.get_model_info.assert_called_once()

    @pytest.mark.asyncio
    async def test_get_model_info_not_loaded(self, settings):
        """Test getting info for model not loaded."""
        manager = ModelManager(settings)

        with patch.object(
            manager.registry,
            "get_model_info",
            return_value={"id": "whisper-base", "loaded": False},
        ):
            info = await manager.get_model_info("whisper-base")

            assert info["id"] == "whisper-base"
            assert info["loaded"] is False

    @pytest.mark.asyncio
    async def test_cleanup(self, settings):
        """Test cleanup."""
        manager = ModelManager(settings)

        with patch.object(
            manager, "unload_all_models", return_value=True
        ) as mock_unload:
            await manager.cleanup()

            mock_unload.assert_called_once()
