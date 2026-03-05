use smd::types::{Fix16, Vec2};

#[test]
fn test_fix16_constants() {
    assert_eq!(Fix16::ZERO.to_raw(), 0);
    assert_eq!(Fix16::ONE.to_raw(), 0x10000);
    assert_eq!(Fix16::HALF.to_raw(), 0x8000);
}

#[test]
fn test_fix16_conversions() {
    let f1 = Fix16::from_int(5);
    assert_eq!(f1.to_int(), 5);
    assert_eq!(f1.to_raw(), 5 << 16);

    let f2 = Fix16::from_raw(0x28000); // 2.5
    assert_eq!(f2.to_int(), 2);
    assert_eq!(f2.to_raw(), 0x28000);
}

#[test]
fn test_fix16_math() {
    let a = Fix16::from_int(5);
    let b = Fix16::from_int(3);

    // Add
    assert_eq!((a + b).to_int(), 8);

    // Sub
    assert_eq!((a - b).to_int(), 2);

    // Mul
    assert_eq!(a.mul(b).to_int(), 15);

    // Div
    let c = Fix16::from_int(10);
    let d = Fix16::from_int(2);
    assert_eq!(c.div(d).to_int(), 5);
}

#[test]
fn test_fix16_ops() {
    let f1 = Fix16::from_int(-5);
    assert_eq!(f1.abs().to_int(), 5);

    let f2 = Fix16::from_int(5);
    assert_eq!(f2.neg().to_int(), -5);
}

#[test]
fn test_vec2_operations() {
    let v1 = Vec2::from_ints(5, 10);
    let v2 = Vec2::from_ints(2, 3);

    let v_add = v1.add(v2);
    assert_eq!(v_add.x.to_int(), 7);
    assert_eq!(v_add.y.to_int(), 13);

    let v_sub = v1.sub(v2);
    assert_eq!(v_sub.x.to_int(), 3);
    assert_eq!(v_sub.y.to_int(), 7);
}
