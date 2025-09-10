from PySide6.QtWidgets import QVBoxLayout, QWidget


def subtle_separator(layout: QVBoxLayout, opacity=0.04):
    separator = QWidget()
    separator.setFixedHeight(1)
    separator.setStyleSheet(
        f"margin-left: 12px; margin-right: 12px;"
        f"background-color: rgba(255, 255, 255, {opacity});"
    )
    layout.addWidget(separator)
