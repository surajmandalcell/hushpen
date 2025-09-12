from __future__ import annotations

from pathlib import Path
from typing import Optional

from PySide6.QtGui import QFontDatabase, QFont
from PySide6.QtWidgets import QApplication


def _assets_path() -> Path:
    # asrpro/ui/utils/font_loader.py -> project root / assets
    return Path(__file__).resolve().parents[3] / "assets"


def load_default_font(app: QApplication, point_size: Optional[int] = None) -> None:
    """Load DM Sans variable font from assets and set as application default.

    Falls back to the existing system font if the asset cannot be loaded.
    """
    try:
        font_path = _assets_path() / "fonts/DMSans-VariableFont_opsz_wght.ttf"
        family: Optional[str] = None

        if font_path.exists():
            font_id = QFontDatabase.addApplicationFont(str(font_path))
            if font_id != -1:
                families = QFontDatabase.applicationFontFamilies(font_id)
                if families:
                    family = families[0]
        # Determine size
        if point_size is None:
            try:
                # Use our configured base size if available
                from ..styles.dark_theme import Fonts

                point_size = int(getattr(Fonts, "BASE_SIZE", 10))
            except Exception:
                point_size = 10

        if family:
            app.setFont(QFont(family, point_size))
            print(f"[Font] Using DM Sans ({family}) at {point_size}pt")
        else:
            # Soft fallback to system font setting
            try:
                from ..styles.dark_theme import Fonts

                app.setFont(QFont(Fonts.get_system_font(), point_size))
                print("[Font] DM Sans not found; using system font")
            except Exception:
                pass
    except Exception as e:
        print(f"[Font] Failed to load default font: {e}")
