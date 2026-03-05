use smd::sprite::{SpriteSize, attr};

#[test]
fn test_sprite_size_dimensions() {
    assert_eq!(SpriteSize::Size1x1.width(), 8);
    assert_eq!(SpriteSize::Size1x1.height(), 8);

    assert_eq!(SpriteSize::Size2x3.width(), 16);
    assert_eq!(SpriteSize::Size2x3.height(), 24);

    assert_eq!(SpriteSize::Size4x4.width(), 32);
    assert_eq!(SpriteSize::Size4x4.height(), 32);
}

#[test]
fn test_sprite_size_tiles() {
    assert_eq!(SpriteSize::Size1x1.width_tiles(), 1);
    assert_eq!(SpriteSize::Size1x1.height_tiles(), 1);

    assert_eq!(SpriteSize::Size3x2.width_tiles(), 3);
    assert_eq!(SpriteSize::Size3x2.height_tiles(), 2);
}

#[test]
fn test_attr_flags() {
    assert_eq!(attr::PRIORITY, 0x8000);
    assert_eq!(attr::PAL0, 0x0000);
    assert_eq!(attr::PAL1, 0x2000);
    assert_eq!(attr::PAL2, 0x4000);
    assert_eq!(attr::PAL3, 0x6000);
    assert_eq!(attr::VFLIP, 0x1000);
    assert_eq!(attr::HFLIP, 0x0800);
}
