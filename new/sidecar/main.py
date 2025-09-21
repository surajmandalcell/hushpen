#!/usr/bin/env python3
"""
ASR Pro Python Sidecar - Main Entry Point
"""

import logging
import sys
import argparse
import asyncio
from pathlib import Path

# Add the current directory to Python path
sys.path.insert(0, str(Path(__file__).parent))

from api.server import create_app
from config.settings import Settings

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

async def run_tests():
    """Run comprehensive model tests."""
    try:
        from tests.test_runner import run_all_tests
        success = await run_all_tests()
        return success
    except Exception as e:
        logger.error(f"Error running tests: {e}")
        return False

def main():
    """Main entry point for the ASR Pro sidecar."""
    parser = argparse.ArgumentParser(description='ASR Pro Python Sidecar')
    parser.add_argument('--test', action='store_true', help='Run comprehensive model tests')
    args = parser.parse_args()
    
    if args.test:
        logger.info("Running comprehensive model tests...")
        success = asyncio.run(run_tests())
        sys.exit(0 if success else 1)
    
    try:
        logger.info("Starting ASR Pro Python Sidecar...")
        
        # Initialize configuration
        settings = Settings()
        
        # Create FastAPI app
        app = create_app(settings)
        
        # Get server configuration
        server_config = settings.get_server_config()
        host = server_config.get('host', '127.0.0.1')
        port = server_config.get('port', 8000)
        
        logger.info(f"Starting server on {host}:{port}")
        
        # Start the server
        import uvicorn
        uvicorn.run(
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
    main()
