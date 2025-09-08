"""Test script to verify dark mode detection and icon inversion."""

import sys
from pathlib import Path
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QPixmap
from PySide6.QtCore import Qt

# Add the project root to Python path
sys.path.insert(0, str(Path(__file__).parent))

from asrpro.ui.tray import is_dark_theme, invert_icon


def test_dark_mode_detection():
    """Test the dark mode detection function."""
    app = QApplication.instance() or QApplication(sys.argv)

    print("=== Dark Mode Detection Test ===")
    dark_mode = is_dark_theme()
    print(f"Dark theme detected: {dark_mode}")

    try:
        # Try Windows registry method
        import winreg

        try:
            key = winreg.OpenKey(
                winreg.HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            )
            value, _ = winreg.QueryValueEx(key, "SystemUsesLightTheme")
            winreg.CloseKey(key)
            print(f"Windows registry SystemUsesLightTheme: {value} (0=dark, 1=light)")
        except Exception as e:
            print(f"Registry method failed: {e}")
    except ImportError:
        print("Registry method not available (non-Windows system)")

    # Test Qt palette method
    if isinstance(app, QApplication):
        palette = app.palette()
        window_color = palette.color(palette.ColorRole.Window)
        print(f"Window background color lightness: {window_color.lightness()}/255")
        print(f"Qt palette suggests dark theme: {window_color.lightness() < 128}")

    return dark_mode


def test_icon_inversion():
    """Test the icon inversion function."""
    print("\n=== Icon Inversion Test ===")

    icon_path = Path(__file__).parent / "assets" / "icon.png"
    if icon_path.exists():
        print(f"Loading icon from: {icon_path}")
        original = QPixmap(str(icon_path))
        print(f"Original icon size: {original.width()}x{original.height()}")

        inverted = invert_icon(original)
        print(f"Inverted icon size: {inverted.width()}x{inverted.height()}")

        # Save inverted icon for comparison
        inverted_path = icon_path.parent / "icon_inverted_test.png"
        inverted.save(str(inverted_path))
        print(f"Inverted icon saved to: {inverted_path}")

        return True
    else:
        print(f"Icon file not found at: {icon_path}")
        return False


if __name__ == "__main__":
    app = QApplication.instance() or QApplication(sys.argv)

    print("asrpro Tray Icon Theme Test")
    print("=" * 40)

    dark_mode = test_dark_mode_detection()
    icon_test_passed = test_icon_inversion()

    print("\n=== Summary ===")
    print(f"Dark mode detected: {dark_mode}")
    print(f"Icon inversion test: {'PASSED' if icon_test_passed else 'FAILED'}")
    print(f"Recommended action: {'Invert icon' if dark_mode else 'Use original icon'}")

    if not app.exec():
        print("\nTest completed successfully!")
