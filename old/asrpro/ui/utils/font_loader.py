from __future__ import annotations

from pathlib import Path
import os
import platform
from typing import Optional

from PySide6.QtGui import QFontDatabase, QFont
from PySide6.QtWidgets import QApplication


def _assets_path() -> Path:
    # asrpro/ui/utils/font_loader.py -> project root / assets
    return Path(__file__).resolve().parents[3] / "assets"


def load_default_font(app: QApplication, point_size: Optional[int] = None) -> None:
    """Load Roboto variable font from assets and set as application default.

    Falls back to the existing system font if the asset cannot be loaded.
    """
    try:
        font_path = _assets_path() / "fonts/Roboto-VariableFont_wdth_wght.ttf"
        family: Optional[str] = None

        if font_path.exists():
            font_id = QFontDatabase.addApplicationFont(str(font_path))
            if font_id != -1:
                families = QFontDatabase.applicationFontFamilies(font_id)
                if families:
                    family = families[0]
        # Determine size with platform scaling
        if point_size is None:
            try:
                # Use our configured base size if available
                from ..styles.dark_theme import Fonts
                base_size = int(getattr(Fonts, "BASE_SIZE", 10))
                point_size = base_size
            except Exception:
                # Default size based on platform
                point_size = 11 if platform.system() == 'Darwin' else 10

        if family:
            f = QFont(family, point_size)
            try:
                pref = os.environ.get("ASRPRO_FONT_HINTING", "full").lower().strip()
                if pref in ("none", "off", "disable"):
                    f.setHintingPreference(QFont.HintingPreference.PreferNoHinting)
                else:
                    # Default to full hinting for crisper rendering on Windows
                    f.setHintingPreference(QFont.HintingPreference.PreferFullHinting)
            except Exception:
                pass
            try:
                f.setStyleStrategy(QFont.StyleStrategy.PreferAntialias)
                f.setStyleHint(QFont.StyleHint.SansSerif)
            except Exception:
                pass
            f.setKerning(True)
            
            # Platform-specific font weight adjustment
            # macOS renders fonts bolder, so we use a lighter weight
            if platform.system() == 'Darwin':
                f.setWeight(QFont.Weight.Light)  # Lighter weight on macOS
            else:
                f.setWeight(QFont.Weight.Normal)  # Normal weight on Windows/Linux
            
            app.setFont(f)
            print(f"[Font] Using Roboto ({family}) at {point_size}pt with AA")
        else:
            # Soft fallback to system font setting
            try:
                from ..styles.dark_theme import Fonts

                app.setFont(QFont(Fonts.get_system_font(), point_size))
                print("[Font] Roboto not found; using system font")
            except Exception:
                pass
    except Exception as e:
        print(f"[Font] Failed to load default font: {e}")
