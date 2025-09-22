"""
Tests for API server
"""

import pytest
from unittest.mock import Mock, AsyncMock, patch
from fastapi.testclient import TestClient
import tempfile
import os

from api.server import create_app
from config.settings import Settings

class TestAPIServer:
    """Test cases for API server."""
    
    def test_create_app(self):
        """Test app creation."""
        settings = Settings()
        app = create_app(settings)
        
        assert app is not None
        assert app.title == "ASR Pro API"
        assert app.version == "1.0.0"
    
    def test_health_check_healthy(self):
        """Test health check when healthy."""
        settings = Settings()
        
        with patch('api.server.ModelManager') as mock_manager_class:
            mock_manager = Mock()
            mock_manager.get_current_model.return_value = 'whisper-base'
            mock_manager.get_current_device.return_value = 'cpu'
            mock_manager_class.return_value = mock_manager
            
            app = create_app(settings)
            client = TestClient(app)
            response = client.get("/health")

            assert response.status_code == 200
            data = response.json()
            assert data["status"] == "healthy"
            assert data["current_model"] == "whisper-base"
            assert data["device"] == "cpu"
    
    def test_health_check_unhealthy(self):
        """Test health check when unhealthy."""
        settings = Settings()
        
        with patch('api.server.ModelManager') as mock_manager_class:
            mock_manager = Mock()
            mock_manager.get_current_model.side_effect = Exception("Error")
            mock_manager_class.return_value = mock_manager
            
            app = create_app(settings)
            client = TestClient(app)
            response = client.get("/health")

            assert response.status_code == 200
            data = response.json()
            assert data["status"] == "unhealthy"
    
    def test_list_models_success(self):
        """Test successful model listing."""
        settings = Settings()
        
        with patch('api.server.ModelManager') as mock_manager_class:
            mock_manager = Mock()
            mock_manager.list_available_models = AsyncMock(return_value=['whisper-base', 'whisper-small'])
            mock_manager.is_model_ready.return_value = True
            mock_manager_class.return_value = mock_manager
            
            app = create_app(settings)
            client = TestClient(app)
            response = client.get("/v1/models")
            
            assert response.status_code == 200
            data = response.json()
            assert data["object"] == "list"
            assert len(data["data"]) == 6  # Updated to match actual registry count
            model_ids = [model["id"] for model in data["data"]]
            assert "whisper-base" in model_ids
            assert all(model["ready"] is False for model in data["data"])  # No models loaded initially
    
    def test_list_models_failure(self):
        """Test model listing failure."""
        settings = Settings()
        
        with patch('api.server.ModelManager') as mock_manager_class:
            mock_manager = Mock()
            mock_manager.list_available_models = AsyncMock(side_effect=Exception("Error"))
            mock_manager.is_model_ready = Mock(return_value=False)
            mock_manager_class.return_value = mock_manager
            
            app = create_app(settings)
            client = TestClient(app)
            response = client.get("/v1/models")

            assert response.status_code == 500
            data = response.json()
            assert "Failed to list models" in data["detail"]
    
    def test_set_model_success(self):
        """Test successful model setting."""
        settings = Settings()
        
        with patch('api.server.ModelManager') as mock_manager_class:
            mock_manager = Mock()
            mock_manager.set_model = AsyncMock(return_value=True)
            mock_manager_class.return_value = mock_manager
            
            app = create_app(settings)
            client = TestClient(app)
            response = client.post("/v1/settings/model", json={"model_id": "whisper-base"})
            
            assert response.status_code == 200
            data = response.json()
            assert data["status"] == "success"
            assert data["model"] == "whisper-base"
    
    def test_set_model_failure(self):
        """Test model setting failure."""
        settings = Settings()
        
        with patch('api.server.ModelManager') as mock_manager_class:
            mock_manager = Mock()
            mock_manager.set_model = AsyncMock(return_value=False)
            mock_manager_class.return_value = mock_manager
            
            app = create_app(settings)
            client = TestClient(app)
            response = client.post("/v1/settings/model", json={"model_id": "whisper-base"})

            assert response.status_code == 400
            data = response.json()
            assert "Failed to set model" in data["detail"]
    
    def test_set_model_exception(self):
        """Test model setting with exception."""
        settings = Settings()
        
        with patch('api.server.ModelManager') as mock_manager_class:
            mock_manager = Mock()
            mock_manager.set_model = AsyncMock(side_effect=Exception("Error"))
            mock_manager_class.return_value = mock_manager
            
            app = create_app(settings)
            client = TestClient(app)
            response = client.post("/v1/settings/model", json={"model_id": "whisper-base"})

            assert response.status_code == 500
            data = response.json()
            assert "Failed to set model" in data["detail"]
    
    def test_transcribe_audio_json_format(self):
        """Test audio transcription with JSON format."""
        settings = Settings()
        
        # Create a temporary audio file
        with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
            f.write(b'fake audio data')
            f.flush()

            with patch('api.server.ModelManager') as mock_manager_class:
                mock_manager = Mock()
                mock_manager.get_current_model.return_value = 'whisper-base'
                mock_manager.transcribe_file = AsyncMock(return_value={
                    'text': 'Hello world',
                    'language': 'en',
                    'segments': [{'start': 0, 'end': 1, 'text': 'Hello world'}]
                })
                mock_manager_class.return_value = mock_manager
                
                app = create_app(settings)
                client = TestClient(app)
                with open(f.name, 'rb') as audio_file:
                    response = client.post(
                        "/v1/audio/transcriptions",
                        files={"file": ("test.wav", audio_file, "audio/wav")}
                    )

                assert response.status_code == 200
                data = response.json()
                assert data["text"] == "Hello world"
                assert data["language"] == "en"
                assert len(data["segments"]) == 1
    
    def test_transcribe_audio_text_format(self):
        """Test audio transcription with text format."""
        settings = Settings()

        with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
            f.write(b'fake audio data')
            f.flush()

            with patch('api.server.ModelManager') as mock_manager_class:
                mock_manager = Mock()
                mock_manager.get_current_model.return_value = 'whisper-base'
                mock_manager.transcribe_file = AsyncMock(return_value={
                    'text': 'Hello world',
                    'language': 'en'
                })
                mock_manager_class.return_value = mock_manager
                
                app = create_app(settings)
                client = TestClient(app)
                with open(f.name, 'rb') as audio_file:
                    response = client.post(
                        "/v1/audio/transcriptions?response_format=text",
                        files={"file": ("test.wav", audio_file, "audio/wav")}
                    )
                
                assert response.status_code == 200
                data = response.json()
                assert data["text"] == "Hello world"
    
    def test_transcribe_audio_srt_format(self):
        """Test audio transcription with SRT format."""
        settings = Settings()

        with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
            f.write(b'fake audio data')
            f.flush()

            with patch('api.server.ModelManager') as mock_manager_class:
                mock_manager = Mock()
                mock_manager.get_current_model.return_value = 'whisper-base'
                mock_manager.transcribe_file = AsyncMock(return_value={
                    'text': 'Hello world',
                    'segments': [
                        {'start': 0.0, 'end': 1.0, 'text': 'Hello world'},
                        {'start': 1.0, 'end': 2.0, 'text': 'How are you?'}
                    ]
                })
                mock_manager_class.return_value = mock_manager
                
                app = create_app(settings)
                client = TestClient(app)
                with open(f.name, 'rb') as audio_file:
                    response = client.post(
                        "/v1/audio/transcriptions?response_format=srt",
                        files={"file": ("test.wav", audio_file, "audio/wav")}
                    )
                
                assert response.status_code == 200
                content = response.text
                assert "1\n00:00:00,000 --> 00:00:01,000\nHello world" in content
                assert "2\n00:00:01,000 --> 00:00:02,000\nHow are you?" in content
    
    def test_transcribe_audio_unsupported_format(self):
        """Test audio transcription with unsupported file format."""
        settings = Settings()
        app = create_app(settings)
        
        with tempfile.NamedTemporaryFile(suffix='.txt', delete=False) as f:
            f.write(b'not audio data')
            f.flush()
            
            client = TestClient(app)
            with open(f.name, 'rb') as file:
                response = client.post(
                    "/v1/audio/transcriptions",
                    files={"file": ("test.txt", file, "text/plain")}
                )
            
            assert response.status_code == 400
            data = response.json()
            assert "Unsupported file type" in data["detail"]
    
    def test_transcribe_audio_no_model(self):
        """Test audio transcription with no model loaded."""
        settings = Settings()

        with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
            f.write(b'fake audio data')
            f.flush()

            with patch('api.server.ModelManager') as mock_manager_class:
                mock_manager = Mock()
                mock_manager.get_current_model.return_value = None
                mock_manager_class.return_value = mock_manager
                
                app = create_app(settings)
                client = TestClient(app)
                with open(f.name, 'rb') as audio_file:
                    response = client.post(
                        "/v1/audio/transcriptions",
                        files={"file": ("test.wav", audio_file, "audio/wav")}
                    )
                
                assert response.status_code == 400
                data = response.json()
                assert "No model specified or loaded" in data["detail"]
    
    def test_transcribe_audio_specific_model(self):
        """Test audio transcription with specific model."""
        settings = Settings()

        with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
            f.write(b'fake audio data')
            f.flush()

            with patch('api.server.ModelManager') as mock_manager_class:
                mock_manager = Mock()
                mock_manager.transcribe_file = AsyncMock(return_value={'text': 'Hello world'})
                mock_manager_class.return_value = mock_manager
                
                app = create_app(settings)
                client = TestClient(app)
                with open(f.name, 'rb') as audio_file:
                    response = client.post(
                        "/v1/audio/transcriptions?model=whisper-small",
                        files={"file": ("test.wav", audio_file, "audio/wav")}
                    )
                
                assert response.status_code == 200
                # Verify the model parameter was passed correctly
                mock_manager.transcribe_file.assert_called_once()
    
    def test_transcribe_audio_exception(self):
        """Test audio transcription with exception."""
        settings = Settings()

        with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as f:
            f.write(b'fake audio data')
            f.flush()

            with patch('api.server.ModelManager') as mock_manager_class:
                mock_manager = Mock()
                mock_manager.get_current_model.return_value = 'whisper-base'
                mock_manager.transcribe_file = AsyncMock(side_effect=Exception("Transcription failed"))
                mock_manager_class.return_value = mock_manager
                
                app = create_app(settings)
                client = TestClient(app)
                with open(f.name, 'rb') as audio_file:
                    response = client.post(
                        "/v1/audio/transcriptions",
                        files={"file": ("test.wav", audio_file, "audio/wav")}
                    )
                
                assert response.status_code == 500
                data = response.json()
                assert "Failed to transcribe audio" in data["detail"]
    
    def test_cors_headers(self):
        """Test CORS headers are set correctly."""
        settings = Settings()
        app = create_app(settings)
        
        client = TestClient(app)
        response = client.options("/health", headers={"Origin": "http://localhost:3000"})
        
        # CORS headers should be present
        assert "access-control-allow-origin" in response.headers
