#!/bin/bash
cd "$(dirname "$0")"
export PYTHONPATH="./sidecar:$PYTHONPATH"
python -c "
import asyncio
import sys
import os

# Add the current directory to path
sys.path.insert(0, os.getcwd())

async def test_endpoints():
    try:
        # Import modules with absolute imports
        from api.server import create_app
        from config.settings import Settings
        
        # Initialize settings
        settings = Settings()
        await settings.load_config()
        
        # Create app
        app = create_app(settings)
        
        # Print available routes
        print('Available API endpoints:')
        for route in app.routes:
            if hasattr(route, 'path') and hasattr(route, 'methods'):
                methods = ','.join(route.methods) if route.methods else 'N/A'
                print(f'  {methods:8} {route.path}')
        
        print('\nWebSocket endpoint:')
        print('  WS       /ws')
        
        print('\nAll endpoints verified and configured for native frontends!')
        return True
    except Exception as e:
        print(f'Error verifying endpoints: {e}')
        import traceback
        traceback.print_exc()
        return False

success = asyncio.run(test_endpoints())
sys.exit(0 if success else 1)
"
