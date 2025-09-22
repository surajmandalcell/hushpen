#!/usr/bin/env python3
"""Test the simplified keyboard page."""

import sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from PySide6.QtWidgets import QApplication, QMainWindow
from asrpro.ui.pages.keyboard_page import KeyboardPage


def main():
    app = QApplication(sys.argv)
    
    # Create a main window to hold the page
    window = QMainWindow()
    window.setWindowTitle("Keyboard Page Test")
    window.resize(800, 600)
    
    # Create the keyboard page
    keyboard_page = KeyboardPage()
    window.setCentralWidget(keyboard_page)
    
    # Show the window
    window.show()
    
    sys.exit(app.exec())


if __name__ == "__main__":
    main()