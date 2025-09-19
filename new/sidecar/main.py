#!/usr/bin/env python3
"""
ASR Pro Python Sidecar - Main Entry Point
"""

import asyncio
import logging
import sys
from pathlib import Path

# Add the current directory to Python path
sys.path.insert(0, str(Path(__file__).parent))

from api.server import create_app
from config.manager import ConfigManager
from system.process import ProcessManager

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

async def main():
    """Main entry point for the ASR Pro sidecar."""
    try:
        logger.info("Starting ASR Pro Python Sidecar...")
        
        # Initialize configuration
        config_manager = ConfigManager()
        await config_manager.load_config()
        
        # Check for existing instances
        process_manager = ProcessManager()
        if process_manager.is_instance_running():
            logger.warning("Another instance is already running")
            return
        
        # Create FastAPI app
        app = create_app(config_manager)
        
        # Get server configuration
        server_config = config_manager.get_server_config()
        host = server_config.get('host', '127.0.0.1')
        port = server_config.get('port', 8000)
        
        logger.info(f"Starting server on {host}:{port}")
        
        # Start the server
        import uvicorn
        await uvicorn.run(
            app,
            host=host,
            port=port,
            log_level="info"
        )
        
    except KeyboardInterrupt:
        logger.info("Shutting down...")
    except Exception as e:
        logger.error(f"Error starting sidecar: {e}")
        sys.exit(1)

if __name__ == "__main__":
    asyncio.run(main())
