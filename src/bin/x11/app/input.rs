//! Core-protocol pointer input on the desktop windows, forwarded to the
//! daemon in the same units the layer-shell client uses.

use super::App;
use waywallen_display as sys;
use x11rb::protocol::xproto::{ButtonPressEvent, MotionNotifyEvent};

/// linux/input-event-codes.h
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;
const BTN_SIDE: u32 = 0x113;
const BTN_EXTRA: u32 = 0x114;

/// One wheel notch. libinput reports a notch as 15 on wl_pointer.axis and
/// the layer-shell client divides that by 10.
const WHEEL_NOTCH: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerAction {
    Button(u32),
    Scroll { dx: f32, dy: f32 },
}

/// Maps a core X11 button number to an evdev button code or a wheel step.
pub fn map_button(detail: u8) -> Option<PointerAction> {
    Some(match detail {
        1 => PointerAction::Button(BTN_LEFT),
        2 => PointerAction::Button(BTN_MIDDLE),
        3 => PointerAction::Button(BTN_RIGHT),
        4 => PointerAction::Scroll {
            dx: 0.0,
            dy: -WHEEL_NOTCH,
        },
        5 => PointerAction::Scroll {
            dx: 0.0,
            dy: WHEEL_NOTCH,
        },
        6 => PointerAction::Scroll {
            dx: -WHEEL_NOTCH,
            dy: 0.0,
        },
        7 => PointerAction::Scroll {
            dx: WHEEL_NOTCH,
            dy: 0.0,
        },
        8 => PointerAction::Button(BTN_SIDE),
        9 => PointerAction::Button(BTN_EXTRA),
        _ => return None,
    })
}

impl App {
    fn ready_display(&self, window: u32) -> Option<(&str, *mut sys::waywallen_display_t)> {
        let entry = self.outputs.get(&window)?;
        let binding = entry.binding.as_ref()?;
        let session = entry
            .session
            .as_ref()
            .filter(|session| session.is_ready())?;
        Some((binding.display_name.as_str(), session.display.0))
    }

    pub(super) fn handle_motion(&self, event: &MotionNotifyEvent) {
        let Some((name, display)) = self.ready_display(event.event) else {
            return;
        };
        // Core event times wrap at 32 bits; let the display library stamp
        // CLOCK_MONOTONIC instead, as layer-shell does.
        let rc = unsafe {
            sys::waywallen_display_send_pointer_motion(
                display,
                f32::from(event.event_x),
                f32::from(event.event_y),
                0,
                0,
            )
        };
        if rc < 0 {
            log::debug!("[{name}] send pointer_motion failed: {rc}");
        }
    }

    pub(super) fn handle_button(&self, event: &ButtonPressEvent, pressed: bool) {
        let Some((name, display)) = self.ready_display(event.event) else {
            return;
        };
        let (x, y) = (f32::from(event.event_x), f32::from(event.event_y));
        let rc = match map_button(event.detail) {
            Some(PointerAction::Button(button)) => {
                let state = if pressed {
                    sys::WAYWALLEN_POINTER_BUTTON_STATE_PRESSED
                } else {
                    sys::WAYWALLEN_POINTER_BUTTON_STATE_RELEASED
                };
                unsafe {
                    sys::waywallen_display_send_pointer_button(display, x, y, button, state, 0, 0)
                }
            }
            // The server reports every wheel step as a press/release pair.
            Some(PointerAction::Scroll { dx, dy }) if pressed => unsafe {
                sys::waywallen_display_send_pointer_axis(
                    display,
                    x,
                    y,
                    dx,
                    dy,
                    sys::WAYWALLEN_POINTER_AXIS_SOURCE_WHEEL,
                    0,
                    0,
                )
            },
            _ => return,
        };
        if rc < 0 {
            log::debug!("[{name}] send pointer input failed: {rc}");
        } else {
            log::trace!(
                "[{name}] pointer button {} {} at ({x}, {y})",
                event.detail,
                if pressed { "pressed" } else { "released" }
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_buttons_map_to_evdev_codes_and_wheel_steps() {
        assert_eq!(map_button(1), Some(PointerAction::Button(BTN_LEFT)));
        assert_eq!(map_button(2), Some(PointerAction::Button(BTN_MIDDLE)));
        assert_eq!(map_button(3), Some(PointerAction::Button(BTN_RIGHT)));
        assert_eq!(
            map_button(5),
            Some(PointerAction::Scroll {
                dx: 0.0,
                dy: WHEEL_NOTCH
            })
        );
        assert_eq!(map_button(9), Some(PointerAction::Button(BTN_EXTRA)));
        assert_eq!(map_button(12), None);
    }
}
