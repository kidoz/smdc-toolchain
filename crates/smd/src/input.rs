//! Controller input handling for Sega Mega Drive
//!
//! The Genesis supports two controller ports. This module provides
//! functions to read button states from both 3-button and 6-button controllers.
//!
//! # Example
//!
//! ```no_run
//! use smd::input::{self, Button};
//!
//! input::init();
//!
//! loop {
//!     let buttons = input::read(0);
//!
//!     if buttons.contains(Button::UP) {
//!         player_y -= 2;
//!     }
//!     if buttons.contains(Button::A) {
//!         shoot();
//!     }
//! }
//! ```

/// Controller 1 data port
const JOY1_DATA: *mut u8 = 0xA10003 as *mut u8;
/// Controller 1 control port
const JOY1_CTRL: *mut u8 = 0xA10009 as *mut u8;
/// Controller 2 data port
const JOY2_DATA: *mut u8 = 0xA10005 as *mut u8;
/// Controller 2 control port
const JOY2_CTRL: *mut u8 = 0xA1000B as *mut u8;

/// Button flags
///
/// These flags represent individual buttons on the controller.
/// Use with [`Buttons`] type for type-safe button checking.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Button {
    /// D-pad Up
    UP = 0x0001,
    /// D-pad Down
    DOWN = 0x0002,
    /// D-pad Left
    LEFT = 0x0004,
    /// D-pad Right
    RIGHT = 0x0008,
    /// A button
    A = 0x0010,
    /// B button
    B = 0x0020,
    /// C button
    C = 0x0040,
    /// Start button
    START = 0x0080,
    /// X button (6-button controller)
    X = 0x0100,
    /// Y button (6-button controller)
    Y = 0x0200,
    /// Z button (6-button controller)
    Z = 0x0400,
    /// Mode button (6-button controller)
    MODE = 0x0800,
}

/// Container for button state
///
/// Holds the state of all buttons as a bitfield.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct Buttons(pub u16);

impl Buttons {
    /// No buttons pressed
    pub const NONE: Buttons = Buttons(0);

    /// Check if a specific button is pressed
    ///
    /// # Example
    /// ```no_run
    /// if buttons.contains(Button::A) {
    ///     // A is pressed
    /// }
    /// ```
    #[inline]
    pub const fn contains(self, button: Button) -> bool {
        (self.0 & button as u16) != 0
    }

    /// Check if any of the specified buttons are pressed
    #[inline]
    pub const fn contains_any(self, buttons: Buttons) -> bool {
        (self.0 & buttons.0) != 0
    }

    /// Check if all of the specified buttons are pressed
    #[inline]
    pub const fn contains_all(self, buttons: Buttons) -> bool {
        (self.0 & buttons.0) == buttons.0
    }

    /// Check if D-pad up is pressed
    #[inline]
    pub const fn up(self) -> bool {
        self.contains(Button::UP)
    }

    /// Check if D-pad down is pressed
    #[inline]
    pub const fn down(self) -> bool {
        self.contains(Button::DOWN)
    }

    /// Check if D-pad left is pressed
    #[inline]
    pub const fn left(self) -> bool {
        self.contains(Button::LEFT)
    }

    /// Check if D-pad right is pressed
    #[inline]
    pub const fn right(self) -> bool {
        self.contains(Button::RIGHT)
    }

    /// Get raw button flags
    #[inline]
    pub const fn raw(self) -> u16 {
        self.0
    }
}

impl core::ops::BitOr for Button {
    type Output = Buttons;
    fn bitor(self, other: Button) -> Buttons {
        Buttons(self as u16 | other as u16)
    }
}

impl core::ops::BitOr<Button> for Buttons {
    type Output = Buttons;
    fn bitor(self, other: Button) -> Buttons {
        Buttons(self.0 | other as u16)
    }
}

/// Initialize controller ports
///
/// Sets up the control registers for reading controllers.
/// Call this once during initialization.
pub fn init() {
    unsafe {
        // Set TH pin as output (directly addressable)
        JOY1_CTRL.write_volatile(0x40);
        JOY2_CTRL.write_volatile(0x40);
    }
}

/// Read controller raw state (3-button, active-low)
///
/// Returns raw hardware value where 0 = pressed, 1 = released.
///
/// # Arguments
/// * `port` - Controller port (0 or 1)
pub fn read_raw(port: u8) -> u8 {
    unsafe {
        let (data, ctrl) = if port == 0 {
            (JOY1_DATA, JOY1_CTRL)
        } else {
            (JOY2_DATA, JOY2_CTRL)
        };

        // Set TH high to read A/Start
        ctrl.write_volatile(0x40);
        let high = data.read_volatile();

        high
    }
}

/// Read controller state (processed, active-high)
///
/// Returns [`Buttons`] with pressed buttons as set bits.
/// This is more convenient than `read_raw()`.
///
/// # Arguments
/// * `port` - Controller port (0 or 1)
pub fn read(port: u8) -> Buttons {
    let raw = read_raw(port);

    // Convert from active-low to active-high and remap bits
    let mut buttons: u16 = 0;

    // Raw format (active-low):
    // Bit 0: Up
    // Bit 1: Down
    // Bit 2: Left
    // Bit 3: Right
    // Bit 4: B
    // Bit 5: C
    // Bit 6: A (when TH=1)
    // Bit 7: Start (when TH=1)

    if (raw & 0x01) == 0 { buttons |= Button::UP as u16; }
    if (raw & 0x02) == 0 { buttons |= Button::DOWN as u16; }
    if (raw & 0x04) == 0 { buttons |= Button::LEFT as u16; }
    if (raw & 0x08) == 0 { buttons |= Button::RIGHT as u16; }
    if (raw & 0x10) == 0 { buttons |= Button::B as u16; }
    if (raw & 0x20) == 0 { buttons |= Button::C as u16; }
    if (raw & 0x40) == 0 { buttons |= Button::A as u16; }
    if (raw & 0x80) == 0 { buttons |= Button::START as u16; }

    Buttons(buttons)
}

/// Static storage for previous frame's button state
static mut PREV_BUTTONS: [Buttons; 2] = [Buttons::NONE, Buttons::NONE];
static mut CURR_BUTTONS: [Buttons; 2] = [Buttons::NONE, Buttons::NONE];

/// Update input state (call once per frame)
///
/// Reads both controllers and updates internal state for
/// `held()`, `pressed()`, and `released()` functions.
pub fn update() {
    unsafe {
        PREV_BUTTONS[0] = CURR_BUTTONS[0];
        PREV_BUTTONS[1] = CURR_BUTTONS[1];
        CURR_BUTTONS[0] = read(0);
        CURR_BUTTONS[1] = read(1);
    }
}

/// Get buttons currently held
///
/// Requires calling [`update()`] each frame.
pub fn held(port: u8) -> Buttons {
    unsafe { CURR_BUTTONS[port as usize] }
}

/// Get buttons just pressed this frame
///
/// Returns buttons that transitioned from released to pressed.
/// Requires calling [`update()`] each frame.
pub fn pressed(port: u8) -> Buttons {
    unsafe {
        let curr = CURR_BUTTONS[port as usize].0;
        let prev = PREV_BUTTONS[port as usize].0;
        Buttons(curr & !prev)
    }
}

/// Get buttons just released this frame
///
/// Returns buttons that transitioned from pressed to released.
/// Requires calling [`update()`] each frame.
pub fn released(port: u8) -> Buttons {
    unsafe {
        let curr = CURR_BUTTONS[port as usize].0;
        let prev = PREV_BUTTONS[port as usize].0;
        Buttons(!curr & prev)
    }
}
