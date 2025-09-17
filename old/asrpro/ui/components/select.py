### SIZE REFERENCE TAGS ###
# Search for :refSize: to find all size-related settings
# :refSize:MAIN_DIMENSIONS - Main widget width/height
# :refSize:MAX_VISIBLE - Max visible dropdown items
# :refSize:SHADOW - Shadow blur and offset
# :refSize:CHEVRON - Chevron arrow dimensions
# :refSize:CHEVRON_STROKE - Chevron line width
# :refSize:BUTTON_PADDING - Main button padding
# :refSize:BUTTON_RADIUS - Main button border radius
# :refSize:BUTTON_FONT - Main button font size
# :refSize:DROPDOWN_RADIUS - Dropdown container radius
# :refSize:DROPDOWN_FONT - Dropdown font size
# :refSize:DROPDOWN_CONTAINER_PADDING - Dropdown container padding
# :refSize:ITEM_PADDING - Individual item padding
# :refSize:ITEM_HEIGHT - Individual item min height
### END SIZE REFERENCE ###

from typing import Optional, List
from PySide6.QtCore import Qt, Signal, QPointF
from PySide6.QtGui import QPainter, QPen, QColor, QPolygonF
from PySide6.QtWidgets import QComboBox, QStyledItemDelegate, QGraphicsDropShadowEffect


class MacSelectDelegate(QStyledItemDelegate):
    def paint(self, painter, option, index):
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        super().paint(painter, option, index)


class MacSelect(QComboBox):
    selection_changed = Signal(int, str)

    def __init__(
        self,
        options: Optional[List[str]] = None,
        placeholder: str = "Select",
        width: Optional[int] = None,
        height: Optional[int] = None,
        parent=None,
    ):
        super().__init__(parent)
        self.placeholder = placeholder
        self._options = options or []
        self._setup_ui(width, height)
        self._apply_styles()
        self.currentIndexChanged.connect(self._on_selection_changed)
        if self._options:
            self.set_options(self._options)

    def _setup_ui(self, width: Optional[int], height: Optional[int]):
        self.setItemDelegate(MacSelectDelegate(self))

        ### <:refSize:MAIN_DIMENSIONS> ###
        self.setFixedWidth(width or 154)
        self.setFixedHeight(height or 28)
        ### </:refSize:MAIN_DIMENSIONS> ###

        ### <:refSize:MAX_VISIBLE> ###
        self.setMaxVisibleItems(10)
        ### </:refSize:MAX_VISIBLE> ###

        self.setEditable(False)
        self.setAttribute(Qt.WidgetAttribute.WA_MacShowFocusRect, False)
        self.setFrame(False)
        self.setProperty("open", False)

        ### <:refSize:SHADOW> ###
        shadow = QGraphicsDropShadowEffect()
        shadow.setBlurRadius(10)
        shadow.setXOffset(0)
        shadow.setYOffset(2)
        shadow.setColor(QColor(0, 0, 0, 40))
        self.setGraphicsEffect(shadow)
        ### </:refSize:SHADOW> ###

    def _on_selection_changed(self, index: int):
        if index >= 0:
            text = self.itemText(index)
            self.selection_changed.emit(index, text)
        self.update()

    def showPopup(self):
        super().showPopup()
        self.setProperty("open", True)
        self.update()

    def hidePopup(self):
        super().hidePopup()
        self.setProperty("open", False)
        self.update()

    def paintEvent(self, event):
        super().paintEvent(event)
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)

        ### <:refSize:CHEVRON_STROKE> ###
        pen = QPen(QColor(255, 255, 255, 180))
        pen.setWidthF(1.5)
        ### </:refSize:CHEVRON_STROKE> ###

        pen.setCapStyle(Qt.PenCapStyle.RoundCap)
        pen.setJoinStyle(Qt.PenJoinStyle.RoundJoin)
        p.setPen(pen)

        ### <:refSize:CHEVRON> ###
        w = 6  # chevron width
        h = 4  # chevron height
        r = self.rect()
        x = r.right() - 17 - w  # right margin + chevron width
        y = r.top() + (r.height() - h) // 2
        ### </:refSize:CHEVRON> ###

        if self.property("open"):
            p.drawPolyline(
                QPolygonF(
                    [
                        QPointF(x, y + h),
                        QPointF(x + w / 2, y),
                        QPointF(x + w, y + h),
                    ]
                )
            )
        else:
            p.drawPolyline(
                QPolygonF(
                    [
                        QPointF(x, y),
                        QPointF(x + w / 2, y + h),
                        QPointF(x + w, y),
                    ]
                )
            )
        p.end()

    def _apply_styles(self):
        self.setStyleSheet(
            """
            /* THE MAIN BUTTON (closed dropdown) */
            MacSelect {
                /* ### <:refSize:BUTTON_PADDING> ### */
                padding: 5px 28px 5px 12px;
                
                /* BUTTON BACKGROUND */
                background: rgba(60, 60, 60, 180);
                border: 1px solid rgba(255, 255, 255, 0.08);
                
                /* ### <:refSize:BUTTON_RADIUS> ### */
                border-radius: 7px;
                
                color: rgba(255, 255, 255, 0.85);
                
                /* ### <:refSize:BUTTON_FONT> ### */
                font-size: 12px;
                
                font-family: "Roboto", -apple-system, "SF Pro Text", "Segoe UI", "Helvetica Neue", sans-serif;
            }
            
            /* MAIN BUTTON WHEN MOUSE HOVERS */
            MacSelect:hover {
                background: rgba(70, 70, 70, 200);
            }

            /* THE DROPDOWN ARROW AREA (hidden) */
            MacSelect::drop-down {
                border: none;
                width: 0px;
            }
            
            /* THE DEFAULT DROPDOWN ARROW ICON (hidden) */
            MacSelect::down-arrow {
                image: none;
            }

            /* THE DROPDOWN CONTAINER/POPUP WINDOW */
            MacSelect QAbstractItemView {
                margin-top: 6px;
                
                
                /* DROPDOWN BACKGROUND */
                background: rgba(40, 40, 40, 250);
                border: 1px solid rgba(255, 255, 255, 0.04);
                
                /* ### <:refSize:DROPDOWN_RADIUS> ### */
                border-radius: 6px;
                
                /* ### <:refSize:DROPDOWN_FONT> ### */
                font-size: 12px;
                font-weight: 600;
                
                /* ### <:refSize:DROPDOWN_CONTAINER_PADDING> ### */
                padding: 1px 0px;
            }

            /* EACH INDIVIDUAL OPTION IN THE DROPDOWN LIST */
            MacSelect QAbstractItemView::item {
                /* ### <:refSize:ITEM_PADDING> ### */
                padding: 6px 12px;
                
                color: rgba(255, 255, 255, 0.85);
                
                /* ### <:refSize:ITEM_HEIGHT> ### */
                min-height: 12px;
            }

            /* THE CURRENTLY SELECTED/HIGHLIGHTED OPTION IN DROPDOWN */
            MacSelect QAbstractItemView::item:selected {
                background: rgba(50, 125, 255, 0.3);
            }
            
            MacSelect QAbstractItemView {
                outline: none;
            }
        """
        )

    def set_options(self, options: List[str], current_index: int = 0):
        self.clear()
        self._options = options.copy()
        for option in options:
            self.addItem(option)
        if 0 <= current_index < len(options):
            self.setCurrentIndex(current_index)

    def get_selected_index(self) -> int:
        return self.currentIndex()

    def get_selected_text(self) -> str:
        return self.currentText()

    def set_selected_index(self, index: int):
        if 0 <= index < self.count():
            self.setCurrentIndex(index)

    def set_selected_text(self, text: str):
        index = self.findText(text)
        if index >= 0:
            self.setCurrentIndex(index)

    def add_option(self, option: str):
        self._options.append(option)
        self.addItem(option)

    def remove_option(self, option: str):
        index = self.findText(option)
        if index >= 0:
            self._options.remove(option)
            self.removeItem(index)

    def clear_options(self):
        self.clear()
        self._options.clear()
