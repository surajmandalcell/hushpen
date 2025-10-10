#!/bin/bash

# ASRPro Linux Frontend Installation Script
# This script installs the ASRPro application system-wide

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Application details
APP_ID="com.asrpro.ASRPro"
APP_NAME="ASRPro"
EXECUTABLE_NAME="asrpro-gtk4"
INSTALL_DIR="/usr/local/bin"
DESKTOP_DIR="/usr/share/applications"
MIME_DIR="/usr/share/mime/packages"
ICON_DIR="/usr/share/icons/hicolor"
SCALABLE_ICON_DIR="/usr/share/icons/hicolor/scalable/apps"

# Function to print colored output
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if running as root
check_root() {
    if [[ $EUID -ne 0 ]]; then
        print_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

# Function to check if executable exists
check_executable() {
    if [ ! -f "target/release/$EXECUTABLE_NAME" ]; then
        print_error "Executable not found at target/release/$EXECUTABLE_NAME"
        print_error "Please build the application first with: cargo build --release"
        exit 1
    fi
}

# Function to install executable
install_executable() {
    print_status "Installing executable to $INSTALL_DIR..."
    cp target/release/$EXECUTABLE_NAME $INSTALL_DIR/
    chmod 755 $INSTALL_DIR/$EXECUTABLE_NAME
    print_status "Executable installed successfully"
}

# Function to install desktop file
install_desktop_file() {
    print_status "Installing desktop entry..."
    mkdir -p $DESKTOP_DIR
    cp data/$APP_ID.desktop $DESKTOP_DIR/
    chmod 644 $DESKTOP_DIR/$APP_ID.desktop
    print_status "Desktop entry installed successfully"
}

# Function to install MIME types
install_mime_types() {
    print_status "Installing MIME type registrations..."
    mkdir -p $MIME_DIR
    cp data/asrpro-mime.xml $MIME_DIR/
    chmod 644 $MIME_DIR/asrpro-mime.xml
    print_status "MIME types registered successfully"
}

# Function to install icons
install_icons() {
    print_status "Installing application icons..."
    
    # Install PNG icons
    for size in 16 32 48 64 128 256 512; do
        icon_dir="$ICON_DIR/${size}x${size}/apps"
        mkdir -p $icon_dir
        if [ -f "data/icons/${size}x${size}/$APP_ID.png" ]; then
            cp data/icons/${size}x${size}/$APP_ID.png $icon_dir/
            chmod 644 $icon_dir/$APP_ID.png
            print_status "Installed ${size}x${size} icon"
        else
            print_warning "Icon size ${size}x${size} not found, skipping"
        fi
    done
    
    # Install SVG icon
    mkdir -p $SCALABLE_ICON_DIR
    if [ -f "data/icons/scalable/$APP_ID.svg" ]; then
        cp data/icons/scalable/$APP_ID.svg $SCALABLE_ICON_DIR/
        chmod 644 $SCALABLE_ICON_DIR/$APP_ID.svg
        print_status "Installed scalable icon"
    else
        print_warning "Scalable icon not found, skipping"
    fi
}

# Function to update databases
update_databases() {
    print_status "Updating desktop and MIME databases..."
    
    # Update desktop database
    if command -v update-desktop-database &> /dev/null; then
        update-desktop-database $DESKTOP_DIR
        print_status "Desktop database updated"
    else
        print_warning "update-desktop-database not found, skipping"
    fi
    
    # Update MIME database
    if command -v update-mime-database &> /dev/null; then
        update-mime-database /usr/share/mime
        print_status "MIME database updated"
    else
        print_warning "update-mime-database not found, skipping"
    fi
    
    # Update icon cache
    if command -v gtk-update-icon-cache &> /dev/null; then
        gtk-update-icon-cache -f -t $ICON_DIR
        print_status "Icon cache updated"
    else
        print_warning "gtk-update-icon-cache not found, skipping"
    fi
}

# Function to create uninstall script
create_uninstall_script() {
    print_status "Creating uninstall script..."
    
    cat > scripts/uninstall.sh << 'EOF'
#!/bin/bash

# ASRPro Linux Frontend Uninstall Script
# This script removes the ASRPro application from the system

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Application details
APP_ID="com.asrpro.ASRPro"
APP_NAME="ASRPro"
EXECUTABLE_NAME="asrpro-gtk4"
INSTALL_DIR="/usr/local/bin"
DESKTOP_DIR="/usr/share/applications"
MIME_DIR="/usr/share/mime/packages"
ICON_DIR="/usr/share/icons/hicolor"

# Function to print colored output
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if running as root
check_root() {
    if [[ $EUID -ne 0 ]]; then
        print_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

# Function to remove executable
remove_executable() {
    if [ -f "$INSTALL_DIR/$EXECUTABLE_NAME" ]; then
        print_status "Removing executable..."
        rm -f $INSTALL_DIR/$EXECUTABLE_NAME
        print_status "Executable removed"
    else
        print_warning "Executable not found, skipping"
    fi
}

# Function to remove desktop file
remove_desktop_file() {
    if [ -f "$DESKTOP_DIR/$APP_ID.desktop" ]; then
        print_status "Removing desktop entry..."
        rm -f $DESKTOP_DIR/$APP_ID.desktop
        print_status "Desktop entry removed"
    else
        print_warning "Desktop entry not found, skipping"
    fi
}

# Function to remove MIME types
remove_mime_types() {
    if [ -f "$MIME_DIR/asrpro-mime.xml" ]; then
        print_status "Removing MIME type registrations..."
        rm -f $MIME_DIR/asrpro-mime.xml
        print_status "MIME types removed"
    else
        print_warning "MIME types not found, skipping"
    fi
}

# Function to remove icons
remove_icons() {
    print_status "Removing application icons..."
    
    # Remove PNG icons
    for size in 16 32 48 64 128 256 512; do
        icon_path="$ICON_DIR/${size}x${size}/apps/$APP_ID.png"
        if [ -f "$icon_path" ]; then
            rm -f $icon_path
            print_status "Removed ${size}x${size} icon"
        fi
    done
    
    # Remove SVG icon
    svg_path="$ICON_DIR/scalable/apps/$APP_ID.svg"
    if [ -f "$svg_path" ]; then
        rm -f $svg_path
        print_status "Removed scalable icon"
    fi
}

# Function to update databases
update_databases() {
    print_status "Updating desktop and MIME databases..."
    
    # Update desktop database
    if command -v update-desktop-database &> /dev/null; then
        update-desktop-database $DESKTOP_DIR
        print_status "Desktop database updated"
    else
        print_warning "update-desktop-database not found, skipping"
    fi
    
    # Update MIME database
    if command -v update-mime-database &> /dev/null; then
        update-mime-database /usr/share/mime
        print_status "MIME database updated"
    else
        print_warning "update-mime-database not found, skipping"
    fi
    
    # Update icon cache
    if command -v gtk-update-icon-cache &> /dev/null; then
        gtk-update-icon-cache -f -t $ICON_DIR
        print_status "Icon cache updated"
    else
        print_warning "gtk-update-icon-cache not found, skipping"
    fi
}

# Main uninstallation
main() {
    print_status "Uninstalling $APP_NAME..."
    
    check_root
    
    remove_executable
    remove_desktop_file
    remove_mime_types
    remove_icons
    update_databases
    
    print_status "$APP_NAME has been successfully uninstalled from your system."
    print_status "Note: User configuration files in ~/.config/$APP_ID have been preserved."
}

# Check if script is being sourced or executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
EOF

    chmod +x scripts/uninstall.sh
    print_status "Uninstall script created at scripts/uninstall.sh"
}

# Main installation
main() {
    print_status "Installing $APP_NAME..."
    
    check_root
    check_executable
    
    install_executable
    install_desktop_file
    install_mime_types
    install_icons
    update_databases
    create_uninstall_script
    
    print_status "$APP_NAME has been successfully installed on your system!"
    print_status "You can now run the application from your desktop menu or by typing '$EXECUTABLE_NAME' in the terminal."
    print_status "To uninstall, run: sudo ./scripts/uninstall.sh"
}

# Check if script is being sourced or executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi