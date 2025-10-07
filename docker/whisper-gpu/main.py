#!/usr/bin/env python3
"""
Whisper GPU Server for ASRPro
Provides HTTP API for audio transcription using Whisper with GPU acceleration
"""

import os
import sys
import logging
import tempfile
import traceback
from pathlib import Path
from typing import Dict, Any, Optional, List
import time

import torch
import whisper
import numpy as np
import soundfile as sf
import librosa
import psutil
import pynvml
import GPUtil
from fastapi import FastAPI, File, UploadFile, HTTPException, BackgroundTasks
from fastapi.responses import JSONResponse
from fastapi.middleware.cors import CORSMiddleware
import uvicorn
from pydantic import BaseModel

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Initialize FastAPI app
app = FastAPI(
    title="Whisper GPU Server",
    description="GPU-accelerated audio transcription using Whisper",
    version="1.0.0"
)

# Add CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Global variables
models = {}
current_model = None
gpu_info = {}

class TranscriptionRequest(BaseModel):
    model: str = "small"
    language: Optional[str] = None
    task: str = "transcribe"

class TranscriptionResponse(BaseModel):
    text: str
    language: str
    model: str
    processing_time: float
    gpu_memory_used: Optional[int] = None

class GPUInfo(BaseModel):
    gpu_available: bool
    gpu_name: Optional[str] = None
    cuda_version: Optional[str] = None
    torch_version: Optional[str] = None
    memory_total: Optional[int] = None
    memory_free: Optional[int] = None
    memory_used: Optional[int] = None
    utilization: Optional[float] = None
    temperature: Optional[float] = None

class GPUMemory(BaseModel):
    allocated: int
    reserved: int
    free: int
    total: int

def initialize_gpu():
    """Initialize GPU and get GPU information."""
    global gpu_info
    
    try:
        # Initialize NVML
        pynvml.nvmlInit()
        
        # Get GPU handle
        handle = pynvml.nvmlDeviceGetHandleByIndex(0)
        
        # Get GPU info
        gpu_info = {
            "gpu_available": True,
            "gpu_name": pynvml.nvmlDeviceGetName(handle).decode('utf-8'),
            "cuda_version": torch.version.cuda,
            "torch_version": torch.__version__,
            "memory_total": None,
            "memory_free": None,
            "memory_used": None,
            "utilization": None,
            "temperature": None
        }
        
        # Get memory info
        memory_info = pynvml.nvmlDeviceGetMemoryInfo(handle)
        gpu_info["memory_total"] = memory_info.total
        gpu_info["memory_free"] = memory_info.free
        gpu_info["memory_used"] = memory_info.used
        
        logger.info(f"GPU initialized: {gpu_info['gpu_name']}")
        logger.info(f"GPU memory: {memory_info.used / 1024**3:.2f}GB / {memory_info.total / 1024**3:.2f}GB")
        
        return True
        
    except Exception as e:
        logger.error(f"Failed to initialize GPU: {e}")
        gpu_info = {"gpu_available": False}
        return False

def load_model(model_name: str) -> bool:
    """Load a Whisper model."""
    global current_model, models
    
    try:
        if model_name in models:
            current_model = models[model_name]
            logger.info(f"Using cached model: {model_name}")
            return True
        
        # Load model
        logger.info(f"Loading model: {model_name}")
        model_path = f"/app/models/{model_name}.pt"
        
        if os.path.exists(model_path):
            model = whisper.load_model(model_path)
        else:
            model = whisper.load_model(model_name, download_root="/app/models")
        
        models[model_name] = model
        current_model = model
        
        logger.info(f"Model loaded successfully: {model_name}")
        return True
        
    except Exception as e:
        logger.error(f"Failed to load model {model_name}: {e}")
        return False

def get_gpu_utilization() -> Dict[str, Any]:
    """Get current GPU utilization."""
    try:
        # Get GPU handle
        handle = pynvml.nvmlDeviceGetHandleByIndex(0)
        
        # Get utilization rates
        utilization = pynvml.nvmlDeviceGetUtilizationRates(handle)
        temperature = pynvml.nvmlDeviceGetTemperature(handle, pynvml.NVML_TEMPERATURE_GPU)
        
        # Get memory info
        memory_info = pynvml.nvmlDeviceGetMemoryInfo(handle)
        
        return {
            "gpu_available": True,
            "utilization": utilization.gpu,
            "memory_utilization": utilization.memory,
            "temperature": temperature,
            "memory_used": memory_info.used,
            "memory_free": memory_info.free,
            "memory_total": memory_info.total
        }
        
    except Exception as e:
        logger.error(f"Failed to get GPU utilization: {e}")
        return {"gpu_available": False}

def get_torch_memory_info() -> Dict[str, int]:
    """Get PyTorch GPU memory information."""
    if not torch.cuda.is_available():
        return {"allocated": 0, "reserved": 0, "free": 0, "total": 0}
    
    device = torch.device("cuda:0")
    allocated = torch.cuda.memory_allocated(device)
    reserved = torch.cuda.memory_reserved(device)
    
    # Get total memory from NVML
    try:
        handle = pynvml.nvmlDeviceGetHandleByIndex(0)
        memory_info = pynvml.nvmlDeviceGetMemoryInfo(handle)
        total = memory_info.total
        free = memory_info.free
    except:
        total = torch.cuda.get_device_properties(device).total_memory
        free = total - allocated
    
    return {
        "allocated": allocated,
        "reserved": reserved,
        "free": free,
        "total": total
    }

def load_audio(file_path: str, sr: int = 16000) -> np.ndarray:
    """Load audio file and resample if necessary."""
    try:
        # Load audio
        audio, orig_sr = librosa.load(file_path, sr=sr)
        return audio
    except Exception as e:
        logger.error(f"Failed to load audio: {e}")
        raise HTTPException(status_code=400, detail=f"Failed to load audio: {str(e)}")

@app.on_event("startup")
async def startup_event():
    """Initialize the server."""
    logger.info("Starting Whisper GPU Server...")
    
    # Initialize GPU
    if not initialize_gpu():
        logger.error("Failed to initialize GPU, exiting")
        sys.exit(1)
    
    # Load default model
    if not load_model("small"):
        logger.error("Failed to load default model, exiting")
        sys.exit(1)
    
    logger.info("Server started successfully")

@app.get("/health")
async def health_check():
    """Health check endpoint."""
    return {"status": "healthy", "timestamp": time.time()}

@app.get("/gpu/info", response_model=GPUInfo)
async def get_gpu_info():
    """Get GPU information."""
    return GPUInfo(**gpu_info)

@app.get("/gpu/utilization")
async def get_gpu_utilization_endpoint():
    """Get current GPU utilization."""
    return get_gpu_utilization()

@app.get("/gpu/memory", response_model=GPUMemory)
async def get_gpu_memory():
    """Get GPU memory information."""
    return GPUMemory(**get_torch_memory_info())

@app.post("/transcribe", response_model=TranscriptionResponse)
async def transcribe_audio(
    background_tasks: BackgroundTasks,
    file: UploadFile = File(...),
    model: str = "small",
    language: Optional[str] = None,
    task: str = "transcribe"
):
    """Transcribe audio file."""
    global current_model
    
    # Validate model
    if model not in ["tiny", "base", "small", "medium", "large"]:
        raise HTTPException(status_code=400, detail="Invalid model name")
    
    # Load model if needed
    if current_model is None or current_model.__class__.__name__ != model:
        if not load_model(model):
            raise HTTPException(status_code=500, detail=f"Failed to load model: {model}")
    
    # Get initial GPU memory
    initial_memory = get_torch_memory_info()
    
    # Save uploaded file
    with tempfile.NamedTemporaryFile(delete=False, suffix=".wav") as tmp_file:
        content = await file.read()
        tmp_file.write(content)
        tmp_file_path = tmp_file.name
    
    try:
        # Load audio
        start_time = time.time()
        audio = load_audio(tmp_file_path)
        
        # Transcribe
        result = current_model.transcribe(
            audio,
            language=language,
            task=task,
            fp16=torch.cuda.is_available()
        )
        
        processing_time = time.time() - start_time
        
        # Get final GPU memory
        final_memory = get_torch_memory_info()
        memory_used = final_memory["allocated"] - initial_memory["allocated"]
        
        # Schedule cleanup
        background_tasks.add_task(os.unlink, tmp_file_path)
        
        return TranscriptionResponse(
            text=result["text"],
            language=result.get("language", "unknown"),
            model=model,
            processing_time=processing_time,
            gpu_memory_used=memory_used
        )
        
    except Exception as e:
        # Cleanup on error
        if os.path.exists(tmp_file_path):
            os.unlink(tmp_file_path)
        
        logger.error(f"Transcription failed: {e}")
        raise HTTPException(status_code=500, detail=f"Transcription failed: {str(e)}")

@app.get("/models")
async def list_models():
    """List available models."""
    return {
        "available": ["tiny", "base", "small", "medium", "large"],
        "loaded": list(models.keys()),
        "current": current_model.__class__.__name__ if current_model else None
    }

@app.post("/models/{model_name}")
async def load_model_endpoint(model_name: str):
    """Load a specific model."""
    if model_name not in ["tiny", "base", "small", "medium", "large"]:
        raise HTTPException(status_code=400, detail="Invalid model name")
    
    if load_model(model_name):
        return {"message": f"Model {model_name} loaded successfully"}
    else:
        raise HTTPException(status_code=500, detail=f"Failed to load model: {model_name}")

@app.get("/system/info")
async def get_system_info():
    """Get system information."""
    return {
        "cpu_count": psutil.cpu_count(),
        "memory_total": psutil.virtual_memory().total,
        "memory_available": psutil.virtual_memory().available,
        "disk_usage": psutil.disk_usage('/').percent,
        "gpu_info": gpu_info,
        "models_loaded": list(models.keys()),
        "current_model": current_model.__class__.__name__ if current_model else None
    }

if __name__ == "__main__":
    uvicorn.run(
        "main:app",
        host="0.0.0.0",
        port=8000,
        workers=1,
        log_level="info"
    )