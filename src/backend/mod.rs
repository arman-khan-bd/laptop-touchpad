//! Cross-platform touchpad input backend abstraction.

use crate::gesture::Gesture;

/// Trait defining the platform-specific touchpad event capture lifecycle.
pub trait GestureBackend: Send + 'static {
    /// Start capturing touchpad events and forwarding unified gesture events.
    fn start(
        &mut self,
        sender: crossbeam_channel::Sender<Gesture>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Gracefully stop the input capture loop and release OS resources.
    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

/// Factory function instantiating the appropriate OS backend.
pub fn create_backend() -> Box<dyn GestureBackend> {
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsPrecisionTouchpadBackend::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxLibinputBackend::new())
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        compile_error!("Gesture Manager currently supports Windows and Linux targets only.");
    }
}
