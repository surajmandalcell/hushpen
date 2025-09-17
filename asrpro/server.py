"""FastAPI server exposing OpenAI-compatible endpoints (models + transcription)."""

from __future__ import annotations
import socket
import threading, tempfile
from pathlib import Path
from typing import Optional
from fastapi import FastAPI, UploadFile, File, Form
from fastapi.responses import JSONResponse, PlainTextResponse
import uvicorn
from .model_manager import ModelManager


def create_app(model_manager: ModelManager):  # pragma: no cover
    app = FastAPI(title="asrpro", version="0.1")

    @app.get("/v1/models")
    def list_models():
        return {"object": "list", "data": model_manager.list_models()}

    @app.post("/v1/audio/transcriptions")
    async def transcribe(
        file: UploadFile = File(...),
        model: Optional[str] = Form(None),
        response_format: str = Form("json"),
    ):
        filename = file.filename or "audio.wav"
        suffix = Path(filename).suffix or ".wav"
        with tempfile.NamedTemporaryFile(delete=False, suffix=suffix) as tmp:
            data = await file.read()
            tmp.write(data)
            tmp_path = tmp.name
        try:
            srt = response_format == "srt"
            result = model_manager.transcribe(tmp_path, model_id=model, return_srt=srt)
            if srt:
                return PlainTextResponse(result, media_type="text/plain")
            # Handle both string and dict results
            if isinstance(result, str):
                return JSONResponse({"text": result})
            return JSONResponse(
                {
                    "text": " ".join(
                        seg.get("text", "") for seg in result if isinstance(seg, dict)
                    ),
                    "segments": result,
                }
            )
        finally:
            Path(tmp_path).unlink(missing_ok=True)

    @app.post("/v1/audio/translations")
    async def translate(
        file: UploadFile = File(...),
        model: Optional[str] = Form(None),
        response_format: str = Form("json"),
    ):
        # For now, just transcribe (translation would need special models)
        return await transcribe(file, model, response_format)

    @app.post("/v1/srt")
    async def generate_srt(
        file: UploadFile = File(...),
        model: Optional[str] = Form(None),
    ):
        filename = file.filename or "audio.wav"
        suffix = Path(filename).suffix or ".wav"
        with tempfile.NamedTemporaryFile(delete=False, suffix=suffix) as tmp:
            data = await file.read()
            tmp.write(data)
            tmp_path = tmp.name
        try:
            result = model_manager.transcribe(tmp_path, model_id=model, return_srt=True)
            return PlainTextResponse(result, media_type="text/plain")
        finally:
            Path(tmp_path).unlink(missing_ok=True)

    @app.post("/v1/settings/model")
    def set_model(model_id: str):
        try:
            model_manager.load(model_id)
            return {"status": "success", "model": model_id}
        except Exception as e:
            return JSONResponse({"status": "error", "error": str(e)}, status_code=400)

    @app.get("/health")
    def health_check():
        return {
            "status": "healthy",
            "current_model": model_manager.current_id,
            "device": model_manager.device,
        }

    return app


class ServerThread(threading.Thread):  # pragma: no cover
    def __init__(self, model_manager: ModelManager, port: int = 7341):
        super().__init__(daemon=True)
        self.model_manager = model_manager
        self.port = self._find_available_port(port)
        self._server = None

    def _find_available_port(self, preferred_port: int, max_tries: int = 10) -> int:
        """Find an available port, starting with the preferred port."""
        for offset in range(max_tries):
            test_port = preferred_port + offset
            if self._is_port_available(test_port):
                if offset > 0:
                    print(f"[Server] Port {preferred_port} is busy, using {test_port} instead")
                return test_port
        
        # If all ports are busy, raise an error
        raise RuntimeError(
            f"Could not find an available port in range {preferred_port}-{preferred_port + max_tries - 1}"
        )
    
    def _is_port_available(self, port: int) -> bool:
        """Check if a port is available for binding."""
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                s.bind(('127.0.0.1', port))
                return True
        except (OSError, socket.error):
            return False

    def run(self):
        app = create_app(self.model_manager)
        try:
            config = uvicorn.Config(
                app, host="127.0.0.1", port=self.port, log_level="warning"
            )
            self._server = uvicorn.Server(config)
            print(f"[Server] Starting API server on http://127.0.0.1:{self.port}")
            self._server.run()
        except Exception as e:
            print(f"[Server] Failed to start server: {e}")
            raise

    def shutdown(self):
        if self._server:
            self._server.should_exit = True
            print("[Server] Shutting down API server")
