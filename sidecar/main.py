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
    level=logging.INFO, format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger(__name__)


async def run_tests():
    """Run comprehensive model tests."""
    try:
        from tests import run_all_tests

        success = await run_all_tests()
        return success
    except Exception as e:
        logger.error(f"Error running tests: {e}")
        return False


async def run_performance_tests():
    """Run performance benchmark tests."""
    try:
        from tests import run_performance_test

        await run_performance_test()
    except Exception as e:
        logger.error(f"Error running performance tests: {e}")
        return False


def main():
    """Main entry point for the ASR Pro sidecar."""
    parser = argparse.ArgumentParser(description="ASR Pro Python Sidecar")
    parser.add_argument(
        "--test", action="store_true", help="Run comprehensive model tests"
    )
    parser.add_argument(
        "--perf", action="store_true", help="Run performance benchmark tests"
    )
    args = parser.parse_args()

    if args.test:
        logger.info("Running comprehensive model tests...")
        success = asyncio.run(run_tests())
        if success is None:
            logger.error("Test execution failed")
            sys.exit(1)
        sys.exit(0 if success else 1)

    if args.perf:
        logger.info("Running performance benchmark tests...")
        asyncio.run(run_performance_tests())
        sys.exit(0)

    try:
        logger.info("Starting ASR Pro Python Sidecar...")

        # Initialize configuration
        settings = Settings()
        await settings.load_config()

        # Check Docker environment
        logger.info("Checking Docker environment...")
        from config.docker_config import DockerConfig
        docker_config = DockerConfig(settings.get_docker_config())
        
        if not docker_config.is_docker_available():
            logger.error("Docker is not available on this system. Please install Docker and ensure it's running.")
            sys.exit(1)
        
        logger.info("Docker environment check passed")
        
        # Create FastAPI app
        app = create_app(settings)

        # Get server configuration
        server_config = settings.get_server_config()
        host = server_config.get("host", "127.0.0.1")
        port = server_config.get("port", 8000)

        logger.info(f"Starting server on {host}:{port}")

        # Start the server with graceful shutdown
        import uvicorn
        import signal

        def signal_handler(signum, frame):
            logger.info("Received shutdown signal, gracefully shutting down...")
            sys.exit(0)

        # Register signal handlers for graceful shutdown
        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGTERM, signal_handler)

        uvicorn.run(
            app,
            host=host,
            port=port,
            log_level="info",
            access_log=False  # Reduce log noise
        )

    except KeyboardInterrupt:
        logger.info("Gracefully shutting down...")
    except Exception as e:
        logger.error(f"Error starting sidecar: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
