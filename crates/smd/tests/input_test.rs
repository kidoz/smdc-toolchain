use smd::input::{Button, Buttons};

#[test]
fn test_button_flags() {
    assert_eq!(Button::UP as u16, 0x0001);
    assert_eq!(Button::DOWN as u16, 0x0002);
    assert_eq!(Button::LEFT as u16, 0x0004);
    assert_eq!(Button::RIGHT as u16, 0x0008);
    assert_eq!(Button::A as u16, 0x0010);
    assert_eq!(Button::B as u16, 0x0020);
    assert_eq!(Button::C as u16, 0x0040);
    assert_eq!(Button::START as u16, 0x0080);
}

#[test]
fn test_buttons_contains() {
    let state = Buttons(Button::UP as u16 | Button::A as u16);

    assert!(state.contains(Button::UP));
    assert!(state.contains(Button::A));
    assert!(!state.contains(Button::DOWN));
    assert!(!state.contains(Button::B));
}

#[test]
fn test_buttons_helpers() {
    let state = Buttons(Button::UP as u16 | Button::RIGHT as u16);

    assert!(state.up());
    assert!(!state.down());
    assert!(!state.left());
    assert!(state.right());
}

#[test]
fn test_bitor_overloads() {
    let state1 = Button::UP | Button::A;
    assert_eq!(state1.raw(), Button::UP as u16 | Button::A as u16);

    let state2 = state1 | Button::B;
    assert_eq!(
        state2.raw(),
        Button::UP as u16 | Button::A as u16 | Button::B as u16
    );
}

#[test]
fn test_contains_any_all() {
    let state = Buttons(Button::UP as u16 | Button::A as u16 | Button::B as u16);
    let check_any = Buttons(Button::UP as u16 | Button::DOWN as u16);
    let check_all = Buttons(Button::UP as u16 | Button::A as u16);
    let check_all_fail = Buttons(Button::UP as u16 | Button::C as u16);

    assert!(state.contains_any(check_any));
    assert!(state.contains_all(check_all));
    assert!(!state.contains_all(check_all_fail));
}
