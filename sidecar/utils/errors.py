"""
Error classes for ASR Pro Python Sidecar
"""

class SidecarError(Exception):
    """Base error class for ASR Pro sidecar."""
    pass

class ModelError(SidecarError):
    """Error related to model operations."""
    pass

class DeviceError(SidecarError):
    """Error related to device operations."""
    pass

class ConfigurationError(SidecarError):
    """Error related to configuration."""
    pass

class APIError(SidecarError):
    """Error related to API operations."""
    pass
