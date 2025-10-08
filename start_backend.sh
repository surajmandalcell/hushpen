#!/bin/bash

# ASR Pro Backend Startup Script
# This script starts the Python backend server as a sidecar for native frontends

echo "==================================="
echo "  ASR Pro Backend Sidecar Service"
echo "==================================="

# Check if Python is installed
if ! command -v python3 &> /dev/null; then
    echo "Error: Python 3 is not installed or not in PATH"
    exit 1
fi

# Check if we're in the right directory
if [ ! -f "sidecar/main.py" ]; then
    echo "Error: sidecar/main.py not found. Please run this script from the project root directory."
    exit 1
fi

# Change to sidecar directory
cd sidecar

# Check if virtual environment exists
if [ ! -d "venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv venv
    if [ $? -ne 0 ]; then
        echo "Error: Failed to create virtual environment"
        exit 1
    fi
fi

# Activate virtual environment
echo "Activating virtual environment..."
source venv/bin/activate
if [ $? -ne 0 ]; then
    echo "Error: Failed to activate virtual environment"
    exit 1
fi

# Install or update dependencies
echo "Installing/updating dependencies..."
pip install -q -r requirements.txt
if [ $? -ne 0 ]; then
    echo "Error: Failed to install dependencies"
    exit 1
fi

# Check if Docker is available
echo "Checking Docker availability..."
if ! command -v docker &> /dev/null; then
    echo "Warning: Docker is not installed or not in PATH"
    echo "The backend will start but transcription functionality will be limited"
else
    echo "Docker is available"
fi

# Get server configuration
HOST="0.0.0.0"
PORT="8000"

# Check if config.json exists in root and override settings
if [ -f "../config.json" ]; then
    echo "Loading configuration from config.json..."
    HOST=$(python3 -c "import json; print(json.load(open('../config.json')).get('server', {}).get('host', '0.0.0.0'))")
    PORT=$(python3 -c "import json; print(json.load(open('../config.json')).get('server', {}).get('port', 8000))")
fi

echo ""
echo "==================================="
echo "Starting ASR Pro Backend Server..."
echo "Host: $HOST"
echo "Port: $PORT"
echo "API Documentation: http://$HOST:$PORT/docs"
echo "Health Check: http://$HOST:$PORT/health"
echo "==================================="
echo ""

# Start the server
python3 main.py

# If we get here, the server has stopped
echo ""
echo "ASR Pro Backend Server has stopped"