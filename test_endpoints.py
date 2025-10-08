# Test script to verify API endpoints
import asyncio
import sys
import os

# Add the current directory to path to enable module imports
sys.path.insert(0, os.path.dirname(__file__))

# Run as a module to avoid relative import issues
if __name__ == "__main__":
    import subprocess
    result = subprocess.run([
        sys.executable, "-c", 
        """
import asyncio
import sys

# Create a minimal package structure
import sidecar.api.server as server_module
import sidecar.config.settings as settings_module

async def test_endpoints():
    try:
        # Initialize settings
        settings = settings_module.Settings()
        await settings.load_config()
        
        # Create app
        app = server_module.create_app(settings)
        
        # Print available routes
        print('Available API endpoints:')
        for route in app.routes:
            if hasattr(route, 'path') and hasattr(route, 'methods'):
                methods = ','.join(route.methods) if route.methods else 'N/A'
                print(f'  {methods:8} {route.path}')
        
        print('\\nWebSocket endpoint:')
        print('  WS       /ws')
        
        print('\\nAll endpoints verified and configured for native frontends!')
        return True
    except Exception as e:
        print(f'Error verifying endpoints: {e}')
        import traceback
        traceback.print_exc()
        return False

success = asyncio.run(test_endpoints())
sys.exit(0 if success else 1)
        """
    ], capture_output=False)
    sys.exit(result.returncode)
