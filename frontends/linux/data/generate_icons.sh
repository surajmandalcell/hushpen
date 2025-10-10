#!/bin/bash

# Script to generate different icon sizes from SVG
# Requires ImageMagick's convert tool

set -e

ICON_NAME="com.asrpro.ASRPro"
SVG_FILE="icons/${ICON_NAME}.svg"
SIZES=(16 32 48 64 128 256 512)

echo "Generating icons from SVG..."

# Check if convert is available
if ! command -v convert &> /dev/null; then
    echo "Error: ImageMagick convert tool not found."
    echo "Please install ImageMagick: sudo apt-get install imagemagick (Ubuntu/Debian)"
    exit 1
fi

# Check if SVG file exists
if [ ! -f "$SVG_FILE" ]; then
    echo "Error: SVG file $SVG_FILE not found."
    exit 1
fi

# Create directories for each size
for size in "${SIZES[@]}"; do
    dir="icons/${size}x${size}"
    mkdir -p "$dir"
    
    echo "Generating ${size}x${size} icon..."
    convert -background none -size "${size}x${size}" "$SVG_FILE" "$dir/$ICON_NAME.png"
done

# Create scalable directory for SVG
mkdir -p icons/scalable
cp "$SVG_FILE" icons/scalable/

echo "Icon generation complete!"
echo "Generated sizes: ${SIZES[*]} and scalable SVG"