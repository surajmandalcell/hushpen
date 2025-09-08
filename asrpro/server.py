"""FastAPI server exposing OpenAI-compatible endpoints (models + transcription)."""

from __future__ import annotations
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
        self.port = port
        self._server = None

    def run(self):
        app = create_app(self.model_manager)
        config = uvicorn.Config(
            app, host="127.0.0.1", port=self.port, log_level="warning"
        )
        self._server = uvicorn.Server(config)
        self._server.run()

    def shutdown(self):
        if self._server:
            self._server.should_exit = True
