"""macOS-style Select dropdown component."""

from typing import Optional, List, Any, Callable
from PySide6.QtCore import Qt, Signal, QPropertyAnimation, QEasingCurve, QRect
from PySide6.QtGui import QFont, QPainter, QPen, QColor, QIcon
from PySide6.QtWidgets import (
    QComboBox,
    QStyledItemDelegate,
    QStyleOptionComboBox,
    QStyle,
)

from ..styles.dark_theme import DarkTheme, Fonts, Dimensions
from ..utils.icon_loader import IconLoader


class MacSelectDelegate(QStyledItemDelegate):
    """Custom delegate for macOS-style dropdown items."""

    def paint(self, painter, option, index):
        """Custom paint for dropdown items."""
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        # Background
        if option.state & QStyle.StateFlag.State_Selected:
            painter.fillRect(option.rect, DarkTheme.ACCENT_BLUE)
            text_color = DarkTheme.MAIN_BG
        elif option.state & QStyle.StateFlag.State_MouseOver:
            painter.fillRect(option.rect, DarkTheme.HOVER_BG)
            text_color = DarkTheme.PRIMARY_TEXT
        else:
            text_color = DarkTheme.PRIMARY_TEXT

        # Text
        painter.setPen(QPen(text_color))
        font = QFont()
        font.setPointSize(Fonts.CONTROL_SIZE)
        font.setWeight(Fonts.NORMAL)
        painter.setFont(font)

        text_rect = option.rect.adjusted(12, 0, -12, 0)
        painter.drawText(
            text_rect,
            Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter,
            index.data(Qt.ItemDataRole.DisplayRole),
        )


class MacSelect(QComboBox):
    """macOS-style select dropdown component."""

    selection_changed = Signal(int, str)  # index, text

    def __init__(
        self,
        options: Optional[List[str]] = None,
        placeholder: str = "Select an option",
        width: Optional[int] = None,
        parent=None,
    ):
        super().__init__(parent)

        self.placeholder = placeholder
        self._options = options or []

        self._setup_ui(width)
        self._apply_styles()
        self._setup_signals()

        # Set initial options
        if self._options:
            self.set_options(self._options)

    def _setup_ui(self, width: Optional[int]):
        """Set up the select UI."""
        # Set custom delegate for dropdown items
        self.setItemDelegate(MacSelectDelegate(self))

        # Configure size
        if width:
            self.setFixedWidth(width)
        self.setMinimumHeight(32)

        # Configure behavior
        self.setMaxVisibleItems(8)
        self.setEditable(False)
        
        # Force disable native styling
        self.setAttribute(Qt.WidgetAttribute.WA_MacShowFocusRect, False)
        self.setFrame(False)

    def _setup_signals(self):
        """Set up signal connections."""
        self.currentIndexChanged.connect(self._on_selection_changed)

    def _on_selection_changed(self, index: int):
        """Handle selection change."""
        if index >= 0:
            text = self.itemText(index)
            self.selection_changed.emit(index, text)

    def _apply_styles(self):
        """Apply dark theme macOS-style dropdown styling."""
        self.setStyleSheet(
            f"""
            MacSelect {{
                background-color: {DarkTheme.CONTROL_BG.name()};
                color: {DarkTheme.PRIMARY_TEXT.name()};
                border: none;
                padding: 8px 32px 8px 12px;
                border-radius: 6px;
                font-size: {Fonts.CONTROL_SIZE}px;
                min-width: 120px;
            }}
            
            MacSelect:focus {{
                border: 2px solid {DarkTheme.FOCUS_BORDER.name()};
            }}
            
            MacSelect::drop-down {{
                subcontrol-origin: padding;
                subcontrol-position: top right;
                width: 20px;
                border: none;
            }}
            MacSelect::down-arrow {{
                image: none;
                border-left: 4px solid transparent;
                border-right: 4px solid transparent;
                border-top: 4px solid {DarkTheme.SECONDARY_TEXT.name()};
                width: 0px;
                height: 0px;
            }}
            MacSelect QAbstractItemView {{
                background-color: {DarkTheme.CONTROL_BG.name()};
                color: {DarkTheme.PRIMARY_TEXT.name()};
                selection-background-color: {DarkTheme.ACCENT_BLUE.name()};
                border: 1px solid {DarkTheme.CARD_BORDER.name()};
                border-radius: 6px;
            }}
            
            MacSelect QAbstractItemView::item {{
                min-height: 20px;
                padding: 4px 12px;
                border: none;
                color: {DarkTheme.PRIMARY_TEXT.name()};
                font-size: {Fonts.CONTROL_SIZE}px;
                font-weight: 400;
            }}
            
            MacSelect QAbstractItemView::item:selected {{
                background-color: {DarkTheme.ACCENT_BLUE.name()};
                color: {DarkTheme.MAIN_BG.name()};
                border-radius: 4px;
                margin: 0px 6px;
            }}
            
            MacSelect QAbstractItemView::item:hover:!selected {{
                background-color: {DarkTheme.HOVER_BG.name()};
                border-radius: 4px;
                margin: 0px 6px;
            }}
        """
        )

    def set_options(self, options: List[str], current_index: int = -1):
        """Set the dropdown options."""
        self.clear()
        self._options = options.copy()

        # Add placeholder if no current selection
        if current_index == -1 and self.placeholder:
            self.addItem(self.placeholder)
            # Disable placeholder - need to cast to QStandardItemModel
            from PySide6.QtGui import QStandardItemModel
            model = self.model()
            if isinstance(model, QStandardItemModel):
                item = model.item(0)
                if item:
                    item.setEnabled(False)
            self.setCurrentIndex(0)

        # Add actual options
        for option in options:
            self.addItem(option)

        # Set current selection
        if current_index >= 0 and current_index < len(options):
            offset = 1 if self.placeholder and current_index == -1 else 0
            self.setCurrentIndex(current_index + offset)

    def get_selected_index(self) -> int:
        """Get the currently selected option index (excluding placeholder)."""
        current = self.currentIndex()
        if self.placeholder and self.count() > 0:
            # Account for placeholder
            if current == 0:
                return -1  # Placeholder selected
            return current - 1
        return current

    def get_selected_text(self) -> str:
        """Get the currently selected option text."""
        index = self.get_selected_index()
        if index >= 0 and index < len(self._options):
            return self._options[index]
        return ""

    def set_selected_index(self, index: int):
        """Set the selected option by index."""
        if 0 <= index < len(self._options):
            offset = 1 if self.placeholder else 0
            self.setCurrentIndex(index + offset)

    def set_selected_text(self, text: str):
        """Set the selected option by text."""
        try:
            index = self._options.index(text)
            self.set_selected_index(index)
        except ValueError:
            pass  # Text not found in options

    def add_option(self, option: str):
        """Add a single option to the dropdown."""
        self._options.append(option)
        self.addItem(option)

    def remove_option(self, option: str):
        """Remove an option from the dropdown."""
        try:
            index = self._options.index(option)
            self._options.remove(option)
            offset = 1 if self.placeholder else 0
            self.removeItem(index + offset)
        except ValueError:
            pass  # Option not found

    def clear_options(self):
        """Clear all options."""
        self.clear()
        self._options.clear()
        if self.placeholder:
            self.addItem(self.placeholder)
            # Disable placeholder - need to cast to QStandardItemModel
            from PySide6.QtGui import QStandardItemModel
            model = self.model()
            if isinstance(model, QStandardItemModel):
                item = model.item(0)
                if item:
                    item.setEnabled(False)
            self.setCurrentIndex(0)

    def is_placeholder_selected(self) -> bool:
        """Check if the placeholder is currently selected."""
        return bool(self.placeholder and self.currentIndex() == 0)

    def set_placeholder(self, placeholder: str):
        """Update the placeholder text."""
        old_selection = self.get_selected_index()
        self.placeholder = placeholder
        self.set_options(self._options, old_selection)
