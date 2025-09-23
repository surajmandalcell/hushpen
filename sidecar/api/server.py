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
)
from fastapi.middleware.cors import CORSMiddleware
from typing import Optional
import logging
import asyncio
import json

from models import ModelManager
from config.settings import Settings
from .models import (
    ModelResponse,
    ModelListResponse,
    ModelSettingRequest,
    ModelSettingResponse,
    HealthResponse,
)

logger = logging.getLogger(__name__)


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
        description="Python sidecar API for ASR Pro",
        version="1.0.0",
    )

    # Add CORS middleware
    app.add_middleware(
        CORSMiddleware,
        allow_origins=[
            "http://localhost:3000",
            "http://localhost:1420",
        ],  # React dev server and Tauri dev server
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    # Initialize model manager
    model_manager = ModelManager(settings)

    @app.get("/health", response_model=HealthResponse)
    async def health_check():
        """Health check endpoint."""
        try:
            current_model = model_manager.get_current_model()
            device = model_manager.get_current_device()
            return HealthResponse(
                status="healthy", current_model=current_model, device=device
            )
        except Exception as e:
            logger.error(f"Health check failed: {e}")
            return HealthResponse(status="unhealthy")

    @app.get("/v1/models", response_model=ModelListResponse)
    async def list_models():
        """List available models."""
        try:
            models = await model_manager.list_available_models()
            model_responses = [
                ModelResponse(id=model_id, ready=model_manager.is_model_ready(model_id))
                for model_id in models
            ]
            return ModelListResponse(data=model_responses)
        except Exception as e:
            logger.error(f"Failed to list models: {e}")
            raise HTTPException(status_code=500, detail="Failed to list models")

    @app.post("/v1/settings/model", response_model=ModelSettingResponse)
    async def set_model(request: ModelSettingRequest):
        """Set the active model."""
        try:
            success = await model_manager.set_model(request.model_id)
            if success:
                return ModelSettingResponse(status="success", model=request.model_id)
            else:
                raise HTTPException(status_code=400, detail="Failed to set model")
        except Exception as e:
            logger.error(f"Failed to set model: {e}")
            raise HTTPException(status_code=500, detail="Failed to set model")

    @app.post("/v1/audio/transcriptions")
    async def transcribe_audio(
        file: UploadFile = File(...),
        model: Optional[str] = None,
        response_format: str = "json",
    ):
        """Transcribe audio file."""
        try:
            # Validate file type
            if not file.filename.endswith((".wav", ".mp3", ".m4a", ".flac")):
                raise HTTPException(
                    status_code=400,
                    detail="Unsupported file type. Please upload WAV, MP3, M4A, or FLAC files.",
                )

            # Use specified model or current model
            model_id = model or model_manager.get_current_model()
            if not model_id:
                raise HTTPException(
                    status_code=400, detail="No model specified or loaded"
                )

            # Define progress callback
            async def progress_callback(progress: int, status: str):
                await send_websocket_message(
                    "transcription_progress",
                    {"filename": file.filename, "progress": progress, "status": status},
                )

            # Send transcription start notification
            await send_websocket_message(
                "transcription_started", {"filename": file.filename, "model": model_id}
            )

            # Transcribe the file with progress updates
            result = await model_manager.transcribe_file(
                file.file, model_id, progress_callback
            )

            # Send transcription complete notification
            await send_websocket_message(
                "transcription_completed",
                {
                    "filename": file.filename,
                    "result_length": len(result.get("text", "")),
                },
            )

            # Format response based on requested format
            if response_format == "text":
                return {"text": result.get("text", "")}
            elif response_format == "srt":
                # Convert segments to SRT format
                srt_content = ""
                for i, segment in enumerate(result.get("segments", []), 1):
                    start_time = segment.get("start", 0)
                    end_time = segment.get("end", 0)
                    text = segment.get("text", "")

                    # Format timestamps as HH:MM:SS,mmm
                    start_formatted = f"{int(start_time//3600):02d}:{int((start_time%3600)//60):02d}:{int(start_time%60):02d},{int((start_time%1)*1000):03d}"
                    end_formatted = f"{int(end_time//3600):02d}:{int((end_time%3600)//60):02d}:{int(end_time%60):02d},{int((end_time%1)*1000):03d}"

                    srt_content += (
                        f"{i}\n{start_formatted} --> {end_formatted}\n{text}\n\n"
                    )

                return srt_content
            else:  # json (default)
                return result

        except HTTPException:
            raise
        except Exception as e:
            logger.error(f"Failed to transcribe audio: {e}")
            raise HTTPException(status_code=500, detail="Failed to transcribe audio")

    @app.on_event("startup")
    async def startup_event():
        """Initialize services on startup."""
        logger.info("Starting ASR Pro API server")
        try:
            await model_manager.initialize()
            logger.info("Model manager initialized successfully")
        except Exception as e:
            logger.error(f"Failed to initialize model manager: {e}")

    @app.on_event("shutdown")
    async def shutdown_event():
        """Cleanup on shutdown."""
        logger.info("Shutting down ASR Pro API server")
        try:
            await model_manager.cleanup()
            logger.info("Model manager cleaned up successfully")
        except Exception as e:
            logger.error(f"Failed to cleanup model manager: {e}")

    @app.websocket("/ws")
    async def websocket_endpoint(websocket: WebSocket):
        """WebSocket endpoint for real-time updates."""
        await connection_manager.connect(websocket)
        try:
            while True:
                data = await websocket.receive_text()
                # Handle incoming messages if needed
                logger.debug(f"Received WebSocket message: {data}")
        except WebSocketDisconnect:
            connection_manager.disconnect(websocket)
            logger.info("WebSocket client disconnected")

    # Helper function to send WebSocket messages
    async def send_websocket_message(message_type: str, data: any):
        """Send message to all connected WebSocket clients."""
        message = {"type": message_type, "data": data}
        await connection_manager.broadcast(message)

    return app
