#!/usr/bin/env python3
"""
Convert PPM image to Genesis/Megadrive tile format.

Genesis VDP Graphics:
- 9-bit color: 3 bits per channel (8 levels each)
- BGR format: 0x0BGR where B, G, R are 0-7
- 4 palettes × 16 colors = 64 max on-screen
- First color = transparency (or background for palette 0)
- Tiles: 8×8 pixels, 4bpp, 32 bytes each
- Each pixel is a 4-bit palette index (0-15)

Color levels (8-bit to 3-bit mapping):
  0-17   -> 0    (Genesis 0x0)
  18-53  -> 1    (Genesis 0x2)
  54-89  -> 2    (Genesis 0x4)
  90-125 -> 3    (Genesis 0x6)
  126-161-> 4    (Genesis 0x8)
  162-197-> 5    (Genesis 0xA)
  198-233-> 6    (Genesis 0xC)
  234-255-> 7    (Genesis 0xE)
"""

import sys
import struct
from collections import OrderedDict

# Genesis color levels in 8-bit RGB
GENESIS_LEVELS = [0, 36, 72, 108, 144, 180, 216, 255]

def read_ppm(filename):
    """Read a PPM P6 (binary) file."""
    with open(filename, 'rb') as f:
        magic = f.readline().decode().strip()
        if magic != 'P6':
            raise ValueError(f"Expected P6, got {magic}")

        line = f.readline().decode().strip()
        while line.startswith('#'):
            line = f.readline().decode().strip()

        width, height = map(int, line.split())
        maxval = int(f.readline().decode().strip())

        pixels = []
        for y in range(height):
            row = []
            for x in range(width):
                r = f.read(1)[0]
                g = f.read(1)[0]
                b = f.read(1)[0]
                row.append((r, g, b))
            pixels.append(row)

        return width, height, maxval, pixels

def rgb8_to_rgb3(val):
    """Convert 8-bit color value (0-255) to 3-bit Genesis value (0-7).

    Uses proper thresholds for best visual match.
    """
    # Find closest Genesis level
    best = 0
    best_diff = abs(val - GENESIS_LEVELS[0])
    for i, level in enumerate(GENESIS_LEVELS[1:], 1):
        diff = abs(val - level)
        if diff < best_diff:
            best = i
            best_diff = diff
    return best

def rgb_to_genesis(r, g, b):
    """Convert 8-bit RGB to Genesis BGR format (0x0BGR)."""
    gr = rgb8_to_rgb3(r)
    gg = rgb8_to_rgb3(g)
    gb = rgb8_to_rgb3(b)
    return (gb << 8) | (gg << 4) | gr

def genesis_to_rgb8(genesis_color):
    """Convert Genesis BGR to approximate 8-bit RGB for comparison."""
    r = GENESIS_LEVELS[genesis_color & 0x7]
    g = GENESIS_LEVELS[(genesis_color >> 4) & 0x7]
    b = GENESIS_LEVELS[(genesis_color >> 8) & 0x7]
    return (r, g, b)

def color_distance(c1, c2):
    """Calculate perceptual color distance (weighted RGB)."""
    r1, g1, b1 = c1
    r2, g2, b2 = c2
    # Human eye is more sensitive to green, less to blue
    rmean = (r1 + r2) // 2
    dr = r1 - r2
    dg = g1 - g2
    db = b1 - b2
    # Weighted Euclidean distance
    return ((2 + rmean/256) * dr * dr + 4 * dg * dg + (2 + (255-rmean)/256) * db * db)

def extract_palette(pixels, max_colors=16):
    """Extract unique colors and create an optimized palette."""
    # Count Genesis color usage
    color_count = {}
    for row in pixels:
        for r, g, b in row:
            genesis_color = rgb_to_genesis(r, g, b)
            color_count[genesis_color] = color_count.get(genesis_color, 0) + 1

    # Sort by frequency
    sorted_colors = sorted(color_count.items(), key=lambda x: -x[1])

    print(f"  Found {len(sorted_colors)} unique Genesis colors")

    if len(sorted_colors) > max_colors:
        print(f"  Warning: Truncating to {max_colors} colors (losing {len(sorted_colors) - max_colors})")
        sorted_colors = sorted_colors[:max_colors]

    # Create palette
    palette = [c[0] for c in sorted_colors]

    # Pad to max_colors
    while len(palette) < max_colors:
        palette.append(0)

    # Create mapping
    color_to_index = {c: i for i, c in enumerate(palette)}

    return palette, color_to_index, color_count

def find_closest_color(genesis_color, color_to_index, palette):
    """Find the closest palette color for a given Genesis color."""
    if genesis_color in color_to_index:
        return color_to_index[genesis_color]

    # Find closest by color distance
    best_idx = 0
    best_dist = float('inf')

    src_rgb = genesis_to_rgb8(genesis_color)

    for idx, pal_color in enumerate(palette):
        if pal_color == 0 and idx > 0:  # Skip padding zeros
            continue
        pal_rgb = genesis_to_rgb8(pal_color)
        dist = color_distance(src_rgb, pal_rgb)
        if dist < best_dist:
            best_dist = dist
            best_idx = idx

    return best_idx

def pixels_to_indexed(pixels, color_to_index, palette):
    """Convert RGB pixels to palette indices."""
    indexed = []
    for row in pixels:
        indexed_row = []
        for r, g, b in row:
            genesis_color = rgb_to_genesis(r, g, b)
            idx = find_closest_color(genesis_color, color_to_index, palette)
            indexed_row.append(idx)
        indexed.append(indexed_row)
    return indexed

def extract_tile(indexed, tile_x, tile_y):
    """Extract an 8x8 tile from indexed pixel data."""
    tile = []
    for py in range(8):
        row = []
        for px in range(8):
            x = tile_x * 8 + px
            y = tile_y * 8 + py
            row.append(indexed[y][x])
        tile.append(row)
    return tuple(tuple(row) for row in tile)

def tile_to_genesis_data(tile):
    """Convert a tile to Genesis 4bpp format (8 × 32-bit words).

    Each row is 8 pixels packed into 32 bits.
    Pixel 0 in bits 31-28, pixel 7 in bits 3-0.
    """
    words = []
    for row in tile:
        word = 0
        for i, pixel in enumerate(row):
            word |= (pixel & 0xF) << (28 - i * 4)
        words.append(word)
    return words

def convert_image(input_ppm, output_header, var_prefix):
    """Convert PPM to Genesis tile data header file."""
    print(f"Reading {input_ppm}...")
    width, height, maxval, pixels = read_ppm(input_ppm)

    if width % 8 != 0 or height % 8 != 0:
        print(f"Warning: Dimensions {width}x{height} not divisible by 8, will be cropped")

    tiles_x = width // 8
    tiles_y = height // 8
    print(f"Image: {width}x{height} pixels = {tiles_x}x{tiles_y} tiles")

    # Extract palette
    print("Extracting palette...")
    palette, color_to_index, color_count = extract_palette(pixels)

    # Show palette info
    print("  Palette colors (Genesis BGR format):")
    for i, color in enumerate(palette):
        if color != 0 or i == 0:
            r8, g8, b8 = genesis_to_rgb8(color)
            print(f"    [{i:2d}] 0x{color:03X} -> RGB({r8:3d},{g8:3d},{b8:3d})")

    # Convert to indexed
    print("Converting to indexed colors...")
    indexed = pixels_to_indexed(pixels, color_to_index, palette)

    # Extract unique tiles and create tilemap
    print("Extracting tiles...")
    unique_tiles = OrderedDict()
    tilemap = []

    for ty in range(tiles_y):
        row = []
        for tx in range(tiles_x):
            tile = extract_tile(indexed, tx, ty)
            if tile not in unique_tiles:
                unique_tiles[tile] = len(unique_tiles)
            row.append(unique_tiles[tile])
        tilemap.append(row)

    print(f"  Unique tiles: {len(unique_tiles)} (of {tiles_x * tiles_y} total)")
    print(f"  Tile data size: {len(unique_tiles) * 32} bytes")

    # Generate header file
    print(f"Writing {output_header}...")
    with open(output_header, 'w') as f:
        f.write(f"/* Generated from {input_ppm} */\n")
        f.write(f"#ifndef {var_prefix.upper()}_ASSETS_H\n")
        f.write(f"#define {var_prefix.upper()}_ASSETS_H\n\n")

        f.write(f"#define {var_prefix.upper()}_TILE_WIDTH {tiles_x}\n")
        f.write(f"#define {var_prefix.upper()}_TILE_HEIGHT {tiles_y}\n")
        f.write(f"#define {var_prefix.upper()}_TILE_COUNT {len(unique_tiles)}\n\n")

        # Palette (16 words)
        f.write(f"static const unsigned short {var_prefix}_palette[16] = {{\n")
        for i, color in enumerate(palette):
            # Genesis expects 0x0BGR with even values: 0,2,4,6,8,A,C,E
            # Convert our 0-7 values to 0,2,4,6,8,10,12,14
            b = ((color >> 8) & 0x7) * 2
            g = ((color >> 4) & 0x7) * 2
            r = (color & 0x7) * 2
            genesis_word = (b << 8) | (g << 4) | r
            f.write(f"    0x{genesis_word:04X}")
            if i < 15:
                f.write(",")
            f.write(f"  /* [{i:2d}] */\n")
        f.write("};\n\n")

        # Tiles (8 longs per tile = 32 bytes)
        f.write(f"static const int {var_prefix}_tiles[] = {{\n")
        for tile_idx, tile in enumerate(unique_tiles.keys()):
            words = tile_to_genesis_data(tile)
            f.write(f"    /* Tile {tile_idx} */\n    ")
            f.write(", ".join(f"0x{w:08X}" for w in words))
            f.write(",\n")
        f.write("};\n\n")

        # Tilemap
        f.write(f"static const unsigned short {var_prefix}_tilemap[] = {{\n")
        for y, row in enumerate(tilemap):
            f.write("    ")
            f.write(", ".join(f"{idx:3d}" for idx in row))
            f.write(",\n")
        f.write("};\n\n")

        f.write("#endif\n")

    print("Done!")
    print(f"\nMemory usage:")
    print(f"  Palette: 32 bytes")
    print(f"  Tiles:   {len(unique_tiles) * 32} bytes")
    print(f"  Tilemap: {tiles_x * tiles_y * 2} bytes")
    print(f"  Total:   {32 + len(unique_tiles) * 32 + tiles_x * tiles_y * 2} bytes")

if __name__ == "__main__":
    if len(sys.argv) != 4:
        print(f"Usage: {sys.argv[0]} <input.ppm> <output.h> <var_prefix>")
        print(f"\nConverts PPM image to Sega Genesis/Megadrive tile format.")
        print(f"\nGenesis VDP specifications:")
        print(f"  - 9-bit color (512 colors): 3 bits per R/G/B channel")
        print(f"  - BGR format: 0x0BGR stored as 0x0BBB0GGG0RRR")
        print(f"  - 16 colors per palette, 4 palettes available")
        print(f"  - Tiles: 8x8 pixels, 4 bits per pixel, 32 bytes each")
        sys.exit(1)

    convert_image(sys.argv[1], sys.argv[2], sys.argv[3])
