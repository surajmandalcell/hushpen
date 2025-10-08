"""
FastAPI server for ASR Pro Python Sidecar
"""

from fastapi import (
    FastAPI,
    HTTPException,
    UploadFile,
    File,
    WebSocket,
    WebSocketDisconnect,
    Request,
    Body,
)
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import HTMLResponse, RedirectResponse
from scalar_fastapi import get_scalar_api_reference
from typing import Optional
import logging
import asyncio
import json
import io
import email.parser
from email.message import EmailMessage

from config.settings import Settings
from .models import (
    ModelResponse,
    ModelListResponse,
    ModelSettingRequest,
    ModelSettingResponse,
    HealthResponse,
    TranscriptionResponse,
)
try:
    from ..utils.errors import (
        SidecarError,
        ModelError,
        DeviceError,
        ConfigurationError,
        APIError,
    )
except ImportError:
    # Fallback for when running as a module
    from utils.errors import (
        SidecarError,
        ModelError,
        DeviceError,
        ConfigurationError,
        APIError,
    )

logger = logging.getLogger(__name__)

# Global model manager instance
model_manager = None


class ConnectionManager:
    """WebSocket connection manager."""

    def __init__(self):
        self.active_connections: list[WebSocket] = []

    async def connect(self, websocket: WebSocket):
        await websocket.accept()
        self.active_connections.append(websocket)

    def disconnect(self, websocket: WebSocket):
        self.active_connections.remove(websocket)

    async def send_personal_message(self, message: dict, websocket: WebSocket):
        try:
            await websocket.send_text(json.dumps(message))
        except:
            self.disconnect(websocket)

    async def broadcast(self, message: dict):
        disconnected_connections = []
        for connection in self.active_connections:
            try:
                await connection.send_text(json.dumps(message))
            except:
                disconnected_connections.append(connection)

        # Remove disconnected connections
        for connection in disconnected_connections:
            self.disconnect(connection)


connection_manager = ConnectionManager()


def create_app(settings: Settings) -> FastAPI:
    """Create FastAPI application with all routes."""
    app = FastAPI(
        title="ASR Pro API",
        description="🎙️ Python sidecar API for ASR Pro - Advanced Speech Recognition and Audio Processing",
        version="1.0.0",
        contact={
            "name": "ASR Pro",
            "url": "https://github.com/asrpro/asrpro",
        },
        license_info={
            "name": "MIT",
        },
        docs_url=None,  # Disable default Swagger docs
        redoc_url=None,  # Disable ReDoc
    )

    # Add CORS middleware
    app.add_middleware(
        CORSMiddleware,
        allow_origins=[
            "http://localhost:3000",
            "http://localhost:1420",
            "app://localhost",
            "tauri://localhost",
            "file://",
            "*"
        ],  # React dev server, Tauri dev server, and native apps
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    # Model manager will be replaced with Docker integration
    # For now, we'll use placeholder responses

    # Helper function to send WebSocket messages
    async def send_websocket_message(message_type: str, data: any):
        """Send message to all connected WebSocket clients."""
        message = {"type": message_type, "data": data}
        await connection_manager.broadcast(message)
        logger.debug(f"Broadcasted WebSocket message: {message_type} to {len(connection_manager.active_connections)} clients")

    async def send_system_status_to_client(websocket: WebSocket):
        """Send system status to a specific WebSocket client."""
        try:
            global model_manager
            if model_manager:
                system_status = await model_manager.get_system_status()
                await websocket.send_text(json.dumps({
                    "type": "system_status",
                    "data": system_status
                }))
        except Exception as e:
            logger.error(f"Failed to send system status to WebSocket client: {e}")

    @app.get("/", include_in_schema=False)
    async def root():
        """Redirect to API documentation."""
        return RedirectResponse(url="/docs")

    @app.get("/health", response_model=HealthResponse, tags=["System"])
    async def health_check():
        """
        System health check endpoint.

        Returns the current system status including:
        - Overall health status
        - Currently active AI model
        - Processing device (CUDA/CPU)
        - Model readiness status
        - Docker integration status
        """
        try:
            global model_manager
            
            if model_manager is None:
                logger.warning("Model manager not initialized in health check")
                return HealthResponse(
                    status="initializing",
                    current_model=None,
                    device="Docker container"
                )
            
            # Get system status from Docker model manager
            system_status = await model_manager.get_system_status()
            
            # Determine overall health status
            status = "healthy"
            if not system_status.get("docker_available", False):
                status = "degraded"  # Docker not available
            elif not system_status.get("current_model"):
                status = "ready"  # Docker available but no model loaded
            
            device = "Docker container (GPU)" if system_status.get("gpu_available") else "Docker container (CPU)"
            
            logger.debug(f"Health check result: status={status}, model={system_status.get('current_model')}, device={device}")
            
            return HealthResponse(
                status=status,
                current_model=system_status.get("current_model"),
                device=device
            )
        except Exception as e:
            logger.error(f"Health check failed: {e}", exc_info=True)
            return HealthResponse(status="unhealthy")

    @app.get("/docs", include_in_schema=False)
    async def scalar_html():
        """Scalar API documentation interface."""
        return get_scalar_api_reference(
            openapi_url=app.openapi_url,
            title=app.title,
        )

    @app.get("/v1/models", response_model=ModelListResponse, tags=["Models"])
    async def list_models():
        """
        List all available AI models.

        Returns a list of available speech recognition models including:
        - Whisper variants (tiny, base, small, medium, large)
        - Parakeet models
        - Custom ONNX models

        Each model includes its ID and readiness status.
        """
        try:
            global model_manager
            
            if model_manager is None:
                logger.warning("Model manager not initialized when listing models")
                raise HTTPException(status_code=503, detail="Model manager not initialized")
            
            # Get available models from Docker model manager
            available_models = await model_manager.list_available_models()
            logger.info(f"Found {len(available_models)} available models")
            
            # Get model info for each available model
            model_responses = []
            for model_id in available_models:
                try:
                    model_info = await model_manager.get_model_info(model_id)
                    if model_info:
                        model_responses.append(
                            ModelResponse(
                                id=model_id,
                                ready=model_info.get("is_running", False)
                            )
                        )
                        logger.debug(f"Model {model_id}: ready={model_info.get('is_running', False)}")
                    else:
                        logger.warning(f"Could not get info for model {model_id}")
                except Exception as e:
                    logger.error(f"Error getting info for model {model_id}: {e}")
                    # Continue with other models even if one fails
            
            logger.info(f"Returning {len(model_responses)} models in response")
            return ModelListResponse(data=model_responses)
        except HTTPException:
            raise
        except Exception as e:
            logger.error(f"Failed to list models: {e}", exc_info=True)
            raise HTTPException(status_code=500, detail="Failed to list models")

    @app.post("/v1/settings/model", response_model=ModelSettingResponse, tags=["Models"])
    async def set_model(request: ModelSettingRequest):
        """
        Set the active AI model for transcription.

        Changes the currently active speech recognition model.
        The model will be loaded and initialized for use in subsequent transcriptions.

        Supported models:
        - `whisper-tiny`: Fastest, lowest accuracy
        - `whisper-base`: Good balance of speed and accuracy
        - `whisper-small`: Better accuracy, slower
        - `whisper-medium`: High accuracy, requires more resources
        - `whisper-large`: Best accuracy, slowest
        """
        try:
            global model_manager
            
            if model_manager is None:
                logger.error("Model manager not initialized when setting model")
                raise HTTPException(status_code=503, detail="Model manager not initialized")
            
            logger.info(f"Setting model to: {request.model_id}")
            
            # Validate model exists
            available_models = await model_manager.list_available_models()
            if request.model_id not in available_models:
                logger.warning(f"Model {request.model_id} not found in available models: {available_models}")
                raise HTTPException(status_code=404, detail=f"Model {request.model_id} not found")
            
            # Set the model using Docker model manager
            success = await model_manager.set_model(request.model_id)
            
            if success:
                logger.info(f"Successfully set model to {request.model_id}")
                return ModelSettingResponse(
                    status="success",
                    model=request.model_id
                )
            else:
                logger.error(f"Failed to set model to {request.model_id}")
                return ModelSettingResponse(
                    status="error",
                    model=request.model_id
                )
        except HTTPException:
            raise
        except Exception as e:
            logger.error(f"Failed to set model: {e}", exc_info=True)
            raise HTTPException(status_code=500, detail="Failed to set model")

    @app.post("/v1/audio/transcriptions", tags=["Transcription"])
    async def transcribe_audio(
        request: Request,
        model: Optional[str] = "whisper-base",
        response_format: str = "json",
    ):
        """
        Transcribe audio file using AI speech recognition.

        Upload an audio file and receive a text transcription using the specified
        or currently active AI model. Supports multiple output formats.

        **Supported Audio/Video Formats:**
        - Audio: WAV, MP3, M4A, FLAC, OGG, AAC, WMA, AIFF, OPUS
        - Video: MP4, AVI, MKV, MOV, FLV, WebM, 3GP, WMV, MPG, MPEG, M4V, TS
        - YouTube/streaming: Any format supported by FFmpeg

        **Response Formats:**
        - `json` (default): Complete transcription with segments and timestamps
        - `text`: Plain text transcription only
        - `srt`: SubRip subtitle format with timestamps

        **Processing:**
        - Real-time progress updates via WebSocket
        - CUDA acceleration when available
        - Automatic model loading if not active
        """
        try:
            # Get raw request data and parse multipart manually
            content_type_header = request.headers.get("content-type", "")
            logger.info(f"Request content-type header: {content_type_header}")

            if "multipart/form-data" not in content_type_header:
                raise HTTPException(
                    status_code=400,
                    detail="Content-Type must be multipart/form-data for file uploads"
                )

            # Extract boundary from content-type
            import re
            boundary_match = re.search(r'boundary=([^;]+)', content_type_header)
            if not boundary_match:
                raise HTTPException(
                    status_code=400,
                    detail="No boundary found in multipart/form-data"
                )

            boundary = boundary_match.group(1).strip()
            logger.info(f"Multipart boundary: {boundary}")

            # Get raw body
            raw_body = await request.body()
            logger.info(f"Raw request body size: {len(raw_body)} bytes")

            if len(raw_body) < 100:
                raise HTTPException(
                    status_code=400,
                    detail="Request body too small - no file data received"
                )

            # Parse multipart data manually
            boundary_bytes = f'--{boundary}'.encode()
            parts = raw_body.split(boundary_bytes)

            logger.info(f"Found {len(parts)} multipart parts")

            file_content = None
            filename = None
            content_type = None

            for i, part in enumerate(parts):
                logger.info(f"Part {i}: {len(part)} bytes, preview: {part[:200]}")

                if b'Content-Disposition' in part:
                    logger.info(f"Found Content-Disposition in part {i}")

                    # Extract filename - try both quoted and unquoted
                    filename_match = re.search(rb'filename="([^"]*)"', part)
                    if not filename_match:
                        filename_match = re.search(rb'filename=([^;\r\n]+)', part)

                    if filename_match:
                        filename = filename_match.group(1).decode('utf-8', errors='ignore').strip()
                        logger.info(f"Extracted filename: '{filename}'")

                    # Extract content-type
                    content_type_match = re.search(rb'Content-Type:\s*([^\r\n]+)', part, re.IGNORECASE)
                    if content_type_match:
                        content_type = content_type_match.group(1).decode('utf-8', errors='ignore').strip()
                        logger.info(f"Extracted content-type: '{content_type}'")

                    # Find start of file data (after headers)
                    header_end = part.find(b'\r\n\r\n')
                    if header_end == -1:
                        header_end = part.find(b'\n\n')

                    if header_end > -1:
                        file_content = part[header_end + 4:]  # Skip \r\n\r\n or \n\n
                        # Remove trailing \r\n if present
                        if file_content.endswith(b'\r\n'):
                            file_content = file_content[:-2]

                        logger.info(f"Extracted file content: {len(file_content)} bytes")
                        if len(file_content) > 0:
                            break

            if not file_content:
                raise HTTPException(
                    status_code=400,
                    detail="No file data found in multipart upload"
                )

            logger.info(f"Parsed multipart: filename='{filename}', content_type='{content_type}', size={len(file_content)} bytes")

            # Log first 100 bytes for debugging
            if len(file_content) > 0:
                preview = file_content[:100]
                logger.info(f"File content preview (first 100 bytes): {preview}")

            # Set default filename if not provided
            if not filename or filename in ['', 'filename']:
                filename = 'uploaded_file'

            # Validate file type - check both filename and content type
            if not filename:
                logger.error("No filename provided in upload")
                raise HTTPException(
                    status_code=400,
                    detail="No filename provided.",
                )

            # Handle cases where API docs interface sends generic filename
            filename_lower = filename.lower()
            # All supported audio and video extensions
            valid_audio_extensions = (".wav", ".mp3", ".m4a", ".flac", ".ogg", ".aac", ".wma", ".aiff", ".opus")
            valid_video_extensions = (".mp4", ".avi", ".mkv", ".mov", ".flv", ".webm", ".3gp", ".wmv", ".mpg", ".mpeg", ".m4v", ".ts", ".f4v")
            valid_extensions = valid_audio_extensions + valid_video_extensions

            valid_content_types = [
                # Audio types
                "audio/wav", "audio/wave", "audio/x-wav",
                "audio/mpeg", "audio/mp3",
                "audio/mp4", "audio/x-m4a",
                "audio/flac", "audio/x-flac",
                "audio/ogg", "audio/opus",
                "audio/aac", "audio/x-aac",
                "audio/wma", "audio/x-ms-wma",
                "audio/aiff", "audio/x-aiff",
                # Video types
                "video/mp4", "video/x-msvideo", "video/avi",
                "video/quicktime", "video/x-matroska",
                "video/x-flv", "video/webm",
                "video/3gpp", "video/x-ms-wmv",
                "video/mpeg", "video/mp2t",
                # Generic types that might contain media
                "application/octet-stream"
            ]

            # Check filename extension or content type
            is_valid_extension = filename_lower.endswith(valid_extensions)
            is_valid_content_type = content_type and any(ct in content_type.lower() for ct in valid_content_types)

            # Special case: if filename is generic "filename", rely on content type or allow through
            if filename in ["filename", "uploaded_file"] and content_type == "application/octet-stream":
                # Allow it through - this is likely from API docs interface
                logger.info("Generic filename detected, allowing upload from API docs interface")
            elif not is_valid_extension and not is_valid_content_type:
                logger.error(f"Unsupported file type: {filename} (content-type: {content_type})")
                raise HTTPException(
                    status_code=400,
                    detail="Unsupported file type. Please upload audio files (WAV, MP3, M4A, FLAC, OGG, etc.) or video files (MP4, AVI, MKV, MOV, WebM, etc.) supported by FFmpeg.",
                )

            # Transcribe using Docker model manager
            logger.info(f"Transcription requested for file: {filename} with model: {model}")
            
            global model_manager
            
            if model_manager is None:
                logger.error("Model manager not initialized during transcription request")
                raise HTTPException(status_code=503, detail="Model manager not initialized")
            
            # Check if model is available
            available_models = await model_manager.list_available_models()
            if model not in available_models:
                logger.error(f"Model {model} not available. Available models: {available_models}")
                raise HTTPException(status_code=404, detail=f"Model {model} not available")
            
            # Send transcription start notification
            await send_websocket_message(
                "transcription_started", {"filename": filename, "model": model}
            )

            # Progress callback for WebSocket updates
            async def progress_callback(progress: float, status: str):
                await send_websocket_message(
                    "transcription_progress",
                    {"filename": filename, "progress": int(progress * 100), "status": status},
                )

            try:
                # Transcribe the audio file
                logger.info(f"Starting transcription of {filename} with model {model}")
                result = await model_manager.transcribe_data(
                    audio_data=file_content,
                    model_id=model,
                    progress_callback=progress_callback,
                    response_format=response_format
                )

                # Ensure result has Docker-specific fields
                if "model_id" not in result:
                    result["model_id"] = model
                if "backend" not in result:
                    result["backend"] = "docker"
                
                # Get container information if available
                try:
                    container_info = await model_manager.get_model_info(model)
                    if container_info:
                        result["container_info"] = {
                            "status": container_info.get("status"),
                            "gpu_allocated": container_info.get("gpu_allocated"),
                            "image": container_info.get("image"),
                        }
                except Exception as e:
                    logger.warning(f"Failed to get container info for {model}: {e}")

                # Send transcription complete notification
                await send_websocket_message(
                    "transcription_completed",
                    {
                        "filename": filename,
                        "result_length": len(result.get("text", "")),
                        "model": model,
                        "processing_time": result.get("processing_time", 0),
                    },
                )

                logger.info(f"Transcription completed for {filename} with {len(result.get('text', ''))} characters")
                
                # Return the transcription result
                return result
                
            except Exception as e:
                # Send transcription error notification
                error_msg = str(e)
                logger.error(f"Transcription failed for {filename}: {error_msg}", exc_info=True)
                
                await send_websocket_message(
                    "transcription_error",
                    {
                        "filename": filename,
                        "error": error_msg,
                        "model": model,
                    },
                )
                raise

        except HTTPException:
            raise
        except ValueError as ve:
            # Handle file validation errors with simple messages
            error_msg = str(ve)
            logger.error(f"File validation error: {error_msg}")
            raise HTTPException(status_code=400, detail=error_msg)
        except RuntimeError as re:
            # Handle Docker-specific runtime errors
            error_msg = str(re)
            logger.error(f"Docker runtime error during transcription: {error_msg}")
            
            # Check for specific Docker errors
            if "not initialized" in error_msg:
                raise HTTPException(status_code=503, detail="Docker model manager not initialized")
            elif "No model specified" in error_msg:
                raise HTTPException(status_code=400, detail="No model specified for transcription")
            elif "Failed to set model" in error_msg:
                raise HTTPException(status_code=503, detail="Failed to initialize model container")
            else:
                raise HTTPException(status_code=500, detail="Docker runtime error")
        except Exception as e:
            logger.error(f"Failed to transcribe audio: {e}", exc_info=True)
            raise HTTPException(status_code=500, detail="Failed to transcribe audio")

    @app.get("/v1/options", tags=["Configuration"])
    async def get_options():
        """
        Get all available endpoints and their configuration options.

        Returns detailed information about all endpoints including:
        - Available endpoints and their methods
        - Supported file formats for each endpoint
        - Available models and their parameters
        - Response format options
        - Input validation rules

        This endpoint is designed to help frontends dynamically configure
        their interface based on backend capabilities.
        """
        global model_manager
        try:
            # Get available models from Docker model manager
            if model_manager:
                available_models = await model_manager.list_available_models()
            else:
                available_models = []

            # Define supported formats
            supported_formats = {
                "audio": {
                    "extensions": [".wav", ".mp3", ".m4a", ".flac", ".ogg", ".aac", ".wma", ".aiff", ".opus"],
                    "mime_types": [
                        "audio/wav", "audio/wave", "audio/x-wav",
                        "audio/mpeg", "audio/mp3",
                        "audio/mp4", "audio/x-m4a",
                        "audio/flac", "audio/x-flac",
                        "audio/ogg", "audio/opus",
                        "audio/aac", "audio/x-aac",
                        "audio/wma", "audio/x-ms-wma",
                        "audio/aiff", "audio/x-aiff"
                    ]
                },
                "video": {
                    "extensions": [".mp4", ".avi", ".mkv", ".mov", ".flv", ".webm", ".3gp", ".wmv", ".mpg", ".mpeg", ".m4v", ".ts", ".f4v"],
                    "mime_types": [
                        "video/mp4", "video/x-msvideo", "video/avi",
                        "video/quicktime", "video/x-matroska",
                        "video/x-flv", "video/webm",
                        "video/3gpp", "video/x-ms-wmv",
                        "video/mpeg", "video/mp2t"
                    ]
                }
            }

            # Define endpoints with their options
            endpoints = {
                "/v1/audio/transcriptions": {
                    "method": "POST",
                    "description": "Transcribe audio or video files using AI models",
                    "parameters": {
                        "file": {
                            "type": "file",
                            "required": True,
                            "description": "Audio or video file to transcribe",
                            "supported_formats": supported_formats,
                            "max_size_mb": 100,
                            "validation": {
                                "min_size_bytes": 100,
                                "supported_extensions": supported_formats["audio"]["extensions"] + supported_formats["video"]["extensions"]
                            }
                        },
                        "model": {
                            "type": "string",
                            "required": False,
                            "default": "whisper-base",
                            "description": "AI model to use for transcription",
                            "options": available_models,
                            "examples": ["whisper-base", "whisper-small", "whisper-medium"]
                        },
                        "response_format": {
                            "type": "string",
                            "required": False,
                            "default": "json",
                            "description": "Output format for transcription result",
                            "options": ["json", "text", "srt"],
                            "examples": ["json", "text", "srt"]
                        }
                    },
                    "responses": {
                        "json": {
                            "description": "Complete transcription with segments and timestamps",
                            "schema": {
                                "text": "string",
                                "segments": "array",
                                "language": "string",
                                "language_probability": "number",
                                "duration": "number",
                                "backend": "string"
                            }
                        },
                        "text": {
                            "description": "Plain text transcription only",
                            "schema": {"text": "string"}
                        },
                        "srt": {
                            "description": "SubRip subtitle format with timestamps",
                            "schema": "string"
                        }
                    }
                },
                "/v1/models": {
                    "method": "GET",
                    "description": "List all available AI models",
                    "parameters": {},
                    "responses": {
                        "default": {
                            "description": "List of available models with readiness status",
                            "schema": {
                                "data": "array",
                                "models": available_models
                            }
                        }
                    }
                },
                "/v1/settings/model": {
                    "method": "POST",
                    "description": "Set the active AI model",
                    "parameters": {
                        "model_id": {
                            "type": "string",
                            "required": True,
                            "description": "Model ID to set as active",
                            "options": available_models,
                            "examples": available_models[:3] if available_models else ["whisper-base"]
                        }
                    },
                    "responses": {
                        "default": {
                            "description": "Model setting result",
                            "schema": {
                                "status": "string",
                                "model": "string"
                            }
                        }
                    }
                },
                "/health": {
                    "method": "GET",
                    "description": "System health check",
                    "parameters": {},
                    "responses": {
                        "default": {
                            "description": "System health and status information",
                            "schema": {
                                "status": "string",
                                "current_model": "string",
                                "device": "string"
                            }
                        }
                    }
                }
            }

            # System capabilities
            gpu_acceleration = False
            current_device = "Unknown"
            
            if model_manager:
                try:
                    system_status = await model_manager.get_system_status()
                    gpu_acceleration = system_status.get("gpu_available", False)
                    current_device = "Docker container (GPU)" if gpu_acceleration else "Docker container (CPU)"
                except Exception as e:
                    logger.warning(f"Failed to get system status: {e}")
                    current_device = "Docker container"
            else:
                current_device = "Docker container"
            
            capabilities = {
                "ffmpeg_support": True,
                "video_processing": True,
                "audio_extraction": True,
                "gpu_acceleration": gpu_acceleration,
                "current_device": current_device,
                "websocket_support": True,
                "real_time_progress": True
            }

            return {
                "api_version": "1.0.0",
                "server": "ASR Pro API",
                "endpoints": endpoints,
                "supported_formats": supported_formats,
                "available_models": available_models,
                "capabilities": capabilities,
                "examples": {
                    "curl_transcription": f"curl -X POST 'http://127.0.0.1:8000/v1/audio/transcriptions?model=whisper-base' -F 'file=@your_audio.mp3'",
                    "curl_video": f"curl -X POST 'http://127.0.0.1:8000/v1/audio/transcriptions?model=whisper-base' -F 'file=@your_video.mp4'",
                    "websocket": "ws://127.0.0.1:8000/ws"
                }
            }

        except Exception as e:
            logger.error(f"Failed to get options: {e}")
            raise HTTPException(status_code=500, detail="Failed to retrieve options")

    @app.on_event("startup")
    async def startup_event():
        """Initialize services on startup."""
        global model_manager
        logger.info("Starting ASR Pro API server")
        
        try:
            # Check Docker availability first
            from config.docker_config import DockerConfig
            docker_config = DockerConfig()
            
            if not docker_config.is_docker_available():
                logger.error("Docker is not available on this system")
                return
            
            logger.info("Docker is available, initializing model manager")
            
            # Initialize Docker model manager
            from docker.model_manager import DockerModelManager
            model_manager = DockerModelManager(settings.get_docker_config())
            
            # Initialize the model manager
            success = await model_manager.initialize()
            if success:
                logger.info("Docker model manager initialized successfully")
                
                # Get system status for logging
                try:
                    system_status = await model_manager.get_system_status()
                    gpu_available = system_status.get("gpu_available", False)
                    logger.info(f"GPU acceleration available: {gpu_available}")
                    
                    # List available models
                    available_models = await model_manager.list_available_models()
                    logger.info(f"Available models: {available_models}")
                except Exception as e:
                    logger.warning(f"Failed to get system status after initialization: {e}")
            else:
                logger.error("Failed to initialize Docker model manager")
        except Exception as e:
            logger.error(f"Error initializing Docker model manager: {e}", exc_info=True)

    @app.on_event("shutdown")
    async def shutdown_event():
        """Cleanup on shutdown."""
        global model_manager
        logger.info("Shutting down ASR Pro API server")
        
        try:
            # Cleanup Docker model manager
            if model_manager:
                logger.info("Cleaning up Docker model manager...")
                await model_manager.cleanup()
                logger.info("Docker model manager cleanup completed")
            else:
                logger.warning("No model manager to cleanup")
        except Exception as e:
            logger.error(f"Error during Docker model manager cleanup: {e}", exc_info=True)

    @app.websocket("/ws")
    async def websocket_endpoint(websocket: WebSocket):
        """
        WebSocket endpoint for real-time updates.

        Connect to receive live progress updates during transcription:
        - `transcription_started`: When transcription begins
        - `transcription_progress`: Progress percentage and status
        - `transcription_completed`: When transcription finishes
        - `container_status`: When Docker container status changes
        - `system_status`: System status updates

        **Message Format:**
        ```json
        {
            "type": "transcription_progress",
            "data": {
                "filename": "audio.wav",
                "progress": 75,
                "status": "Processing...",
                "model": "whisper-base"
            }
        }
        ```
        """
        await connection_manager.connect(websocket)
        logger.info(f"WebSocket client connected (total clients: {len(connection_manager.active_connections)})")
        
        try:
            # Send initial system status to new client
            await send_system_status_to_client(websocket)
            
            while True:
                data = await websocket.receive_text()
                # Handle incoming messages if needed
                logger.debug(f"Received WebSocket message: {data}")
                
                # Parse message and handle if needed
                try:
                    message = json.loads(data)
                    message_type = message.get("type")
                    
                    if message_type == "ping":
                        # Respond to ping with pong
                        await websocket.send_text(json.dumps({"type": "pong"}))
                    elif message_type == "get_status":
                        # Send current system status
                        await send_system_status_to_client(websocket)
                except json.JSONDecodeError:
                    logger.warning(f"Invalid JSON received from WebSocket: {data}")
                    
        except WebSocketDisconnect:
            connection_manager.disconnect(websocket)
            logger.info(f"WebSocket client disconnected (total clients: {len(connection_manager.active_connections)})")

    return app
