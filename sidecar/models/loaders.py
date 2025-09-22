"""
Concrete model loaders built on ONNXBaseLoader.
Now DRY via a single ConfigDrivenLoader that reads candidates from config.
"""

from .base import ONNXBaseLoader


class ConfigDrivenLoader(ONNXBaseLoader):
    def _get_model_name(self):
        # Prefer a list of candidates; fall back to single string if provided
        candidates = self.config.get("candidates")
        if candidates:
            return candidates
        # Back-compat: allow a single model name under 'model_name'
        model_name = self.config.get("model_name")
        return model_name or self.model_id
