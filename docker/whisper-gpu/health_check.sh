#!/bin/bash
# Health check script for Whisper GPU container

# Check if the main process is running
if ! pgrep -f "uvicorn main:app" > /dev/null; then
    echo "Main process not running"
    exit 1
fi

# Check if the API is responding
if ! curl -f -s http://localhost:8000/health > /dev/null; then
    echo "API health check failed"
    exit 1
fi

# Check GPU availability
if ! curl -f -s http://localhost:8000/gpu/info > /dev/null; then
    echo "GPU info endpoint failed"
    exit 1
fi

# Check if GPU is detected
GPU_INFO=$(curl -s http://localhost:8000/gpu/info)
if echo "$GPU_INFO" | grep -q '"gpu_available":false'; then
    echo "GPU not available"
    exit 1
fi

echo "Health check passed"
exit 0