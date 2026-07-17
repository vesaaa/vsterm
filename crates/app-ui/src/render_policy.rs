//! Renderer-aware repaint policy.
//!
//! Hardware adapters can animate at display cadence. CPU adapters (typically
//! WARP in RDP/VM sessions) remain fully supported, but continuous UI work is
//! capped so an otherwise idle terminal manager does not occupy CPU cores.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const HARDWARE_FRAME_INTERVAL: Duration = Duration::from_millis(16);
const SOFTWARE_FRAME_INTERVAL: Duration = Duration::from_millis(67);

static SOFTWARE_RENDERER: AtomicBool = AtomicBool::new(false);

pub fn set_software_renderer(enabled: bool) {
    SOFTWARE_RENDERER.store(enabled, Ordering::Release);
}

pub fn is_software_renderer() -> bool {
    SOFTWARE_RENDERER.load(Ordering::Acquire)
}

pub fn animation_interval() -> Duration {
    animation_interval_for(is_software_renderer())
}

/// Preserve slower functional polling while capping high-frequency redraws.
pub fn limit_interval(requested: Duration) -> Duration {
    if is_software_renderer() {
        requested.max(SOFTWARE_FRAME_INTERVAL)
    } else {
        requested
    }
}

fn animation_interval_for(software: bool) -> Duration {
    if software {
        SOFTWARE_FRAME_INTERVAL
    } else {
        HARDWARE_FRAME_INTERVAL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_animation_is_capped_near_fifteen_fps() {
        assert_eq!(
            animation_interval_for(true),
            Duration::from_millis(67)
        );
    }

    #[test]
    fn hardware_animation_keeps_display_cadence() {
        assert_eq!(
            animation_interval_for(false),
            Duration::from_millis(16)
        );
    }
}
