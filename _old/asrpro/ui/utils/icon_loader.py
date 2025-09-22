"""SVG icon loading and caching utilities."""

from pathlib import Path
from typing import Dict, Optional
from PySide6.QtGui import QIcon, QPixmap, QPainter
from PySide6.QtSvg import QSvgRenderer
from PySide6.QtCore import QSize, Qt


class IconLoader:
    """Centralized SVG icon loading with caching and color tinting."""
    
    _cache: Dict[str, QIcon] = {}
    _renderers: Dict[str, QSvgRenderer] = {}
    
    @classmethod
    def load_icon(cls, name: str, size: int = 16, color: Optional[str] = None) -> QIcon:
        """Load an SVG icon with optional color tinting and caching.
        
        Args:
            name: Icon name (without .svg extension)
            size: Icon size in pixels
            color: Optional hex color for tinting (e.g., "#ffffff")
            
        Returns:
            QIcon object, empty icon if file not found
        """
        cache_key = f"{name}_{size}_{color or 'original'}"
        
        if cache_key in cls._cache:
            return cls._cache[cache_key]
        
        # Try to find the SVG file
        icon_path = cls._find_icon_file(name)
        if not icon_path:
            print(f"[IconLoader] Warning: Icon '{name}' not found")
            cls._cache[cache_key] = QIcon()
            return cls._cache[cache_key]
        
        # Load and render the icon
        icon = cls._render_icon(icon_path, size, color)
        cls._cache[cache_key] = icon
        return icon
    
    @classmethod 
    def _find_icon_file(cls, name: str) -> Optional[Path]:
        """Find the icon file in the assets directory."""
        # Get the project root (now from asrpro/ui/utils/)
        current_dir = Path(__file__).parent
        project_root = current_dir.parent.parent.parent
        
        icon_candidates = [
            project_root / "assets" / "icons" / f"{name}.svg",
            project_root / "assets" / f"{name}.svg",
            # Fallback locations
            current_dir.parent / "assets" / "icons" / f"{name}.svg",
        ]
        
        for candidate in icon_candidates:
            if candidate.exists():
                return candidate
                
        return None
    
    @classmethod
    def _render_icon(cls, icon_path: Path, size: int, color: Optional[str]) -> QIcon:
        """Render SVG to QIcon with optional color tinting."""
        # Get or create SVG renderer
        path_str = str(icon_path)
        if path_str not in cls._renderers:
            cls._renderers[path_str] = QSvgRenderer(path_str)
        
        renderer = cls._renderers[path_str]
        if not renderer.isValid():
            print(f"[IconLoader] Warning: Invalid SVG file: {icon_path}")
            return QIcon()
        
        # Create pixmap
        pixmap = QPixmap(size, size)
        pixmap.fill(Qt.GlobalColor.transparent)
        
        # Render SVG to pixmap
        painter = QPainter(pixmap)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        renderer.render(painter)
        painter.end()
        
        # Apply color tint if requested
        if color:
            tinted_pixmap = cls._apply_color_tint(pixmap, color)
            return QIcon(tinted_pixmap)
        
        return QIcon(pixmap)
    
    @classmethod
    def _apply_color_tint(cls, pixmap: QPixmap, color: str) -> QPixmap:
        """Apply a color tint to a pixmap while preserving alpha."""
        from PySide6.QtGui import QColor, QBrush
        
        tinted = QPixmap(pixmap.size())
        tinted.fill(Qt.GlobalColor.transparent)
        
        painter = QPainter(tinted)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        # Draw the original pixmap
        painter.drawPixmap(0, 0, pixmap)
        
        # Apply color overlay with SourceIn composition mode
        painter.setCompositionMode(QPainter.CompositionMode.CompositionMode_SourceIn)
        painter.fillRect(tinted.rect(), QBrush(QColor(color)))
        
        painter.end()
        return tinted
    
    @classmethod
    def clear_cache(cls):
        """Clear the icon cache (useful for theme changes)."""
        cls._cache.clear()
    
    @classmethod
    def preload_common_icons(cls):
        """Preload commonly used icons for better performance."""
        common_icons = [
            "settings", "cpu", "keyboard", "mic", "info", "power",
            "monitor", "folder-open", "x", "circle-check", "download",
            "headphones"
        ]
        
        for icon_name in common_icons:
            # Preload in common sizes and colors
            cls.load_icon(icon_name, 16)
            cls.load_icon(icon_name, 20)
            cls.load_icon(icon_name, 16, "#999999")  # Default icon color
            cls.load_icon(icon_name, 16, "#ffffff")  # Hover color
            cls.load_icon(icon_name, 16, "#0a84ff")  # Active color