"""
Pydantic models for API responses
"""

from pydantic import BaseModel
from typing import List, Optional

class ModelResponse(BaseModel):
    id: str
    object: str = "model"
    owned_by: str = "asrpro"
    ready: bool

class ModelListResponse(BaseModel):
    object: str = "list"
    data: List[ModelResponse]

class ModelSettingRequest(BaseModel):
    model_id: str

class ModelSettingResponse(BaseModel):
    status: str
    model: str

class HealthResponse(BaseModel):
    status: str
    current_model: Optional[str] = None
    device: Optional[str] = None

class TranscriptionResponse(BaseModel):
    text: str
    language: Optional[str] = None
    language_probability: Optional[float] = None
    duration: Optional[float] = None
    segments: Optional[List[dict]] = None
    metadata: Optional[dict] = None
    specials: Optional[List[dict]] = None
    # Docker-specific fields
    model_id: Optional[str] = None
    processing_time: Optional[float] = None
    backend: Optional[str] = "docker"
    container_info: Optional[dict] = None
