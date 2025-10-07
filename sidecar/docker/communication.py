"""
Communication adapter for interacting with model containers
"""

import logging
import asyncio
import time
from typing import Dict, Any, Optional, List, BinaryIO
import json
import httpx
from dataclasses import dataclass
from enum import Enum

logger = logging.getLogger(__name__)

class ConnectionStatus(Enum):
    """Status of connection to a container."""
    DISCONNECTED = "disconnected"
    CONNECTING = "connecting"
    CONNECTED = "connected"
    ERROR = "error"

@dataclass
class ContainerConnection:
    """Represents a connection to a container."""
    container_id: str
    base_url: str
    status: ConnectionStatus
    connected_at: float
    last_activity: float
    health_check_count: int = 0
    error_count: int = 0

class ContainerCommunicationAdapter:
    """Handles communication with model containers."""
    
    def __init__(self, timeout: int = 30, max_retries: int = 3):
        """
        Initialize communication adapter.
        
        Args:
            timeout: Request timeout in seconds
            max_retries: Maximum number of retry attempts
        """
        self.timeout = timeout
        self.max_retries = max_retries
        self.active_connections: Dict[str, ContainerConnection] = {}
        self.http_client = httpx.AsyncClient(timeout=httpx.Timeout(timeout))
        self.lock = asyncio.Lock()
        
        logger.info(f"ContainerCommunicationAdapter initialized with timeout={timeout}s, max_retries={max_retries}")
    
    async def connect_to_container(self, container_id: str, port: int, host: str = "localhost") -> bool:
        """
        Establish connection to a container endpoint.
        
        Args:
            container_id: Container identifier
            port: Container port
            host: Container host (default: localhost)
            
        Returns:
            True if connection was successful
        """
        async with self.lock:
            base_url = f"http://{host}:{port}"
            
            # Check if already connected
            if container_id in self.active_connections:
                connection = self.active_connections[container_id]
                if connection.status == ConnectionStatus.CONNECTED:
                    logger.debug(f"Already connected to container {container_id}")
                    return True
                else:
                    logger.warning(f"Existing connection to {container_id} has status {connection.status.value}")
            
            # Create new connection
            connection = ContainerConnection(
                container_id=container_id,
                base_url=base_url,
                status=ConnectionStatus.CONNECTING,
                connected_at=time.time(),
                last_activity=time.time()
            )
            
            self.active_connections[container_id] = connection
            
            # Test connection with health check
            try:
                health_response = await self._check_container_health_direct(base_url)
                if health_response:
                    connection.status = ConnectionStatus.CONNECTED
                    logger.info(f"Successfully connected to container {container_id} at {base_url}")
                    return True
                else:
                    connection.status = ConnectionStatus.ERROR
                    connection.error_count += 1
                    logger.error(f"Health check failed for container {container_id}")
                    return False
            except Exception as e:
                connection.status = ConnectionStatus.ERROR
                connection.error_count += 1
                logger.error(f"Failed to connect to container {container_id}: {e}")
                return False
    
    async def disconnect_from_container(self, container_id: str) -> bool:
        """
        Close connection to a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            True if disconnection was successful
        """
        async with self.lock:
            if container_id not in self.active_connections:
                logger.warning(f"No active connection found for container {container_id}")
                return False
            
            connection = self.active_connections.pop(container_id)
            logger.info(f"Disconnected from container {container_id}")
            return True
    
    async def transcribe_with_container(
        self, 
        container_id: str, 
        audio_data: bytes,
        response_format: str = "json",
        language: Optional[str] = None,
        progress_callback: Optional[callable] = None
    ) -> Dict[str, Any]:
        """
        Send transcription request to container.
        
        Args:
            container_id: Container identifier
            audio_data: Audio data as bytes
            response_format: Response format (json|text|srt)
            language: Optional language code
            progress_callback: Optional progress callback function
            
        Returns:
            Transcription result
        """
        if container_id not in self.active_connections:
            raise ValueError(f"No active connection for container {container_id}")
        
        connection = self.active_connections[container_id]
        if connection.status != ConnectionStatus.CONNECTED:
            raise ValueError(f"Container {container_id} not connected (status: {connection.status.value})")
        
        # Update last activity
        connection.last_activity = time.time()
        
        # Prepare request data
        files = {"file": ("audio.wav", audio_data, "audio/wav")}
        data = {"response_format": response_format}
        if language:
            data["language"] = language
        
        # Make request with retry logic
        last_error = None
        for attempt in range(self.max_retries + 1):
            try:
                if progress_callback:
                    progress_callback(0.1, f"Sending transcription request to {container_id} (attempt {attempt + 1})")
                
                async with self.http_client.stream(
                    "POST",
                    f"{connection.base_url}/transcribe",
                    files=files,
                    data=data
                ) as response:
                    if response.status_code == 200:
                        if progress_callback:
                            progress_callback(0.5, "Processing transcription...")
                        
                        result = response.json()
                        
                        if progress_callback:
                            progress_callback(1.0, "Transcription complete")
                        
                        # Update connection stats
                        connection.health_check_count += 1
                        
                        return result
                    else:
                        error_text = await response.aread()
                        last_error = f"HTTP {response.status_code}: {error_text.decode()}"
                        logger.warning(f"Transcription request failed (attempt {attempt + 1}): {last_error}")
                        
                        if attempt < self.max_retries:
                            await asyncio.sleep(1 * (attempt + 1))  # Exponential backoff
                        else:
                            connection.error_count += 1
                            raise Exception(f"Transcription failed after {attempt + 1} attempts: {last_error}")
            
            except httpx.RequestError as e:
                last_error = f"Request error: {str(e)}"
                logger.warning(f"Request error (attempt {attempt + 1}): {last_error}")
                
                if attempt < self.max_retries:
                    await asyncio.sleep(1 * (attempt + 1))  # Exponential backoff
                else:
                    connection.error_count += 1
                    raise Exception(f"Request failed after {attempt + 1} attempts: {last_error}")
        
        # This should not be reached
        raise Exception(f"Unexpected error in transcription request: {last_error}")
    
    async def check_container_health(self, container_id: str) -> bool:
        """
        Check if container is responding to health checks.
        
        Args:
            container_id: Container identifier
            
        Returns:
            True if container is healthy
        """
        if container_id not in self.active_connections:
            return False
        
        connection = self.active_connections[container_id]
        
        try:
            health_response = await self._check_container_health_direct(connection.base_url)
            if health_response:
                connection.health_check_count += 1
                connection.last_activity = time.time()
                return True
            else:
                connection.error_count += 1
                return False
        except Exception as e:
            connection.error_count += 1
            logger.warning(f"Health check failed for container {container_id}: {e}")
            return False
    
    async def _check_container_health_direct(self, base_url: str) -> Optional[Dict[str, Any]]:
        """
        Direct health check to a container URL.
        
        Args:
            base_url: Container base URL
            
        Returns:
            Health response or None if failed
        """
        try:
            response = await self.http_client.get(f"{base_url}/health")
            if response.status_code == 200:
                return response.json()
            else:
                return None
        except Exception:
            return None
    
    async def get_container_info(self, container_id: str) -> Optional[Dict[str, Any]]:
        """
        Get information about a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            Container information or None if not available
        """
        if container_id not in self.active_connections:
            return None
        
        connection = self.active_connections[container_id]
        
        try:
            response = await self.http_client.get(f"{connection.base_url}/info")
            if response.status_code == 200:
                return response.json()
            else:
                return None
        except Exception as e:
            logger.warning(f"Failed to get info for container {container_id}: {e}")
            return None
    
    async def list_available_models(self, container_id: str) -> Optional[List[str]]:
        """
        List available models in a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            List of model names or None if failed
        """
        if container_id not in self.active_connections:
            return None
        
        connection = self.active_connections[container_id]
        
        try:
            response = await self.http_client.get(f"{connection.base_url}/models")
            if response.status_code == 200:
                data = response.json()
                return data.get("models", [])
            else:
                return None
        except Exception as e:
            logger.warning(f"Failed to list models for container {container_id}: {e}")
            return None
    
    def get_connection_status(self, container_id: str) -> Optional[ConnectionStatus]:
        """
        Get the connection status for a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            Connection status or None if not found
        """
        connection = self.active_connections.get(container_id)
        return connection.status if connection else None
    
    def get_connection_info(self, container_id: str) -> Optional[ContainerConnection]:
        """
        Get detailed connection information for a container.
        
        Args:
            container_id: Container identifier
            
        Returns:
            Connection information or None if not found
        """
        return self.active_connections.get(container_id)
    
    def get_all_connections(self) -> Dict[str, ContainerConnection]:
        """
        Get all active connections.
        
        Returns:
            Dictionary of all active connections
        """
        return self.active_connections.copy()
    
    def get_connection_summary(self) -> Dict[str, Any]:
        """
        Get summary of all connections.
        
        Returns:
            Connection summary statistics
        """
        total_connections = len(self.active_connections)
        connected_count = sum(1 for conn in self.active_connections.values() if conn.status == ConnectionStatus.CONNECTED)
        error_count = sum(1 for conn in self.active_connections.values() if conn.status == ConnectionStatus.ERROR)
        
        total_health_checks = sum(conn.health_check_count for conn in self.active_connections.values())
        total_errors = sum(conn.error_count for conn in self.active_connections.values())
        
        return {
            "total_connections": total_connections,
            "connected_count": connected_count,
            "error_count": error_count,
            "total_health_checks": total_health_checks,
            "total_errors": total_errors,
            "connections": {
                container_id: {
                    "status": conn.status.value,
                    "base_url": conn.base_url,
                    "connected_at": conn.connected_at,
                    "last_activity": conn.last_activity,
                    "health_check_count": conn.health_check_count,
                    "error_count": conn.error_count
                }
                for container_id, conn in self.active_connections.items()
            }
        }
    
    async def cleanup_connections(self) -> int:
        """
        Close all active connections.
        
        Returns:
            Number of connections cleaned up
        """
        async with self.lock:
            count = len(self.active_connections)
            self.active_connections.clear()
            
            # Close HTTP client
            await self.http_client.aclose()
            
            # Create new HTTP client for future use
            self.http_client = httpx.AsyncClient(timeout=httpx.Timeout(self.timeout))
            
            logger.info(f"Cleaned up {count} container connections")
            return count
    
    async def __aenter__(self):
        """Async context manager entry."""
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.cleanup_connections()