# Whisper GPU Container for ASRPro
# Optimized for RTX 4070 Super

FROM nvidia/cuda:12.1.0-runtime-ubuntu22.04

# Set environment variables
ENV PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    DEBIAN_FRONTEND=noninteractive \
    CUDA_VISIBLE_DEVICES=0

# Set working directory
WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    python3.10 \
    python3.10-dev \
    python3-pip \
    python3-venv \
    ffmpeg \
    libsndfile1 \
    libsox-fmt-all \
    sox \
    curl \
    wget \
    git \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Create Python virtual environment
RUN python3.10 -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"

# Upgrade pip
RUN pip install --no-cache-dir --upgrade pip setuptools wheel

# Install PyTorch with CUDA support
RUN pip install --no-cache-dir \
    torch==2.1.0 \
    torchvision==0.16.0 \
    torchaudio==2.1.0 \
    --index-url https://download.pytorch.org/whl/cu121

# Copy requirements and install Python dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy application code
COPY main.py .
COPY download_models.py .
COPY health_check.sh .

# Create directories
RUN mkdir -p /app/temp /app/logs /app/models

# Set permissions
RUN chmod +x health_check.sh download_models.py

# Download Whisper models (small and medium for testing)
RUN python3 download_models.py

# Expose port
EXPOSE 8000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD ./health_check.sh

# Set up logging
ENV LOG_LEVEL=INFO
ENV LOG_FORMAT=json

# Create non-root user
RUN useradd -m -u 1000 whisper && \
    chown -R whisper:whisper /app
USER whisper

# Start command
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000", "--workers", "1"]