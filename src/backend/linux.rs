//! Linux touchpad backend utilizing libinput and udev.
//!
//! Directly interacts with `/dev/input/event*` devices via libinput, avoiding
//! compositor lock-in (works across X11, Wayland, and head-less environments).
//! Requires the running user to be in the `input` group or active uaccess udev rule.

use crate::backend::GestureBackend;
use crate::gesture::{Gesture, GestureRecognizer, GestureSensitivity, TouchFrame, TouchPoint};
use crossbeam_channel::Sender;
use input::event::gesture::{
    GestureEvent, GestureEventCoordinates, GestureEventTrait, GesturePinchEvent,
    GesturePinchEventTrait, GestureSwipeEvent,
};
use input::event::touch::{TouchEvent, TouchEventPosition, TouchEventSlot};
use input::event::Event;
use input::{Libinput, LibinputInterface};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

pub struct LinuxLibinputBackend {
    worker_handle: Option<thread::JoinHandle<()>>,
    is_running: Arc<AtomicBool>,
}

impl LinuxLibinputBackend {
    pub fn new() -> Self {
        Self {
            worker_handle: None,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }
}

struct RestrictedInterface;

impl LibinputInterface for RestrictedInterface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        let acc_mode = flags & libc::O_ACCMODE;
        OpenOptions::new()
            .custom_flags(flags)
            .read(acc_mode == libc::O_RDONLY || acc_mode == libc::O_RDWR)
            .write(acc_mode == libc::O_WRONLY || acc_mode == libc::O_RDWR)
            .open(path)
            .map(|f| f.into())
            .map_err(|err| err.raw_os_error().unwrap_or(libc::EACCES))
    }

    fn close_restricted(&mut self, fd: OwnedFd) {
        drop(fd);
    }
}

impl GestureBackend for LinuxLibinputBackend {
    fn start(
        &mut self,
        sender: Sender<Gesture>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.is_running.store(true, Ordering::SeqCst);
        let is_running_clone = self.is_running.clone();

        let handle = thread::Builder::new()
            .name("linux-libinput-backend".into())
            .spawn(move || {
                let mut input = Libinput::new_with_udev(RestrictedInterface);
                if let Err(e) = input.udev_assign_seat("seat0") {
                    log::error!(
                        "Failed to assign seat0 for libinput: {:?}. Ensure user is in 'input' group.",
                        e
                    );
                    is_running_clone.store(false, Ordering::SeqCst);
                    return;
                }

                let mut recognizer = GestureRecognizer::new(GestureSensitivity::default());
                let mut active_slots: HashMap<i32, TouchPoint> = HashMap::new();
                let mut swipe_fingers: usize = 0;
                let mut swipe_accum_dx = 0.0;
                let mut swipe_accum_dy = 0.0;
                let mut pinch_fingers: usize = 2;

                log::info!("Linux libinput gesture capture active on seat0.");

                let raw_fd = input.as_raw_fd();

                while is_running_clone.load(Ordering::SeqCst) {
                    let mut pfd = libc::pollfd {
                        fd: raw_fd,
                        events: libc::POLLIN,
                        revents: 0,
                    };

                    // Poll with 100ms timeout for clean cancellation check
                    let poll_res = unsafe { libc::poll(&mut pfd, 1, 100) };
                    if poll_res < 0 {
                        let err = std::io::Error::last_os_error();
                        if err.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        log::error!("libinput poll error: {}", err);
                        break;
                    }

                    if poll_res == 0 {
                        continue;
                    }

                    if (pfd.revents & libc::POLLIN) != 0 {
                        if let Err(e) = input.dispatch() {
                            log::error!("libinput dispatch error: {:?}", e);
                            continue;
                        }

                        for event in input.by_ref() {
                            match event {
                                Event::Gesture(gesture_event) => match gesture_event {
                                    GestureEvent::Swipe(swipe_event) => match swipe_event {
                                        GestureSwipeEvent::Begin(begin) => {
                                            swipe_fingers = begin.finger_count() as usize;
                                            swipe_accum_dx = 0.0;
                                            swipe_accum_dy = 0.0;
                                        }
                                        GestureSwipeEvent::Update(update) => {
                                            swipe_accum_dx += update.dx();
                                            swipe_accum_dy += update.dy();
                                            if let Some(g) = recognizer.process_delta_swipe(
                                                swipe_fingers,
                                                swipe_accum_dx,
                                                swipe_accum_dy,
                                            ) {
                                                log::info!("Linux libinput swipe triggered: {}", g);
                                                let _ = sender.send(g);
                                                swipe_accum_dx = 0.0;
                                                swipe_accum_dy = 0.0;
                                            }
                                        }
                                        GestureSwipeEvent::End(_) => {
                                            swipe_accum_dx = 0.0;
                                            swipe_accum_dy = 0.0;
                                        }
                                        _ => {}
                                    },
                                    GestureEvent::Pinch(pinch_event) => match pinch_event {
                                        GesturePinchEvent::Begin(begin) => {
                                            pinch_fingers = begin.finger_count() as usize;
                                        }
                                        GesturePinchEvent::Update(update) => {
                                            let scale = update.scale();
                                            if let Some(g) = recognizer
                                                .process_delta_pinch(pinch_fingers, scale)
                                            {
                                                log::info!("Linux libinput pinch triggered: {}", g);
                                                let _ = sender.send(g);
                                            }
                                        }
                                        GesturePinchEvent::End(_) => {}
                                        _ => {}
                                    },
                                    _ => {}
                                },

                                Event::Touch(touch_event) => {
                                    match touch_event {
                                        TouchEvent::Down(down) => {
                                            let slot = down.slot().map(|s| s as i32).unwrap_or(0);
                                            active_slots.insert(
                                                slot,
                                                TouchPoint {
                                                    id: slot,
                                                    x: down.x(),
                                                    y: down.y(),
                                                },
                                            );
                                            let frame = TouchFrame::new(
                                                active_slots.values().copied().collect(),
                                            );
                                            if let Some(g) = recognizer.process_frame(&frame) {
                                                log::info!("Linux touch frame gesture: {}", g);
                                                let _ = sender.send(g);
                                            }
                                        }
                                        TouchEvent::Motion(motion) => {
                                            let slot = motion.slot().map(|s| s as i32).unwrap_or(0);
                                            active_slots.insert(
                                                slot,
                                                TouchPoint {
                                                    id: slot,
                                                    x: motion.x(),
                                                    y: motion.y(),
                                                },
                                            );
                                            let frame = TouchFrame::new(
                                                active_slots.values().copied().collect(),
                                            );
                                            if let Some(g) = recognizer.process_frame(&frame) {
                                                log::info!("Linux touch frame gesture: {}", g);
                                                let _ = sender.send(g);
                                            }
                                        }
                                        TouchEvent::Up(up) => {
                                            let slot = up.slot().map(|s| s as i32).unwrap_or(0);
                                            active_slots.remove(&slot);
                                            let frame = TouchFrame::new(
                                                active_slots.values().copied().collect(),
                                            );
                                            if let Some(g) = recognizer.process_frame(&frame) {
                                                log::info!("Linux touch frame gesture: {}", g);
                                                let _ = sender.send(g);
                                            }
                                        }
                                        TouchEvent::Cancel(cancel) => {
                                            let slot = cancel.slot().map(|s| s as i32).unwrap_or(0);
                                            active_slots.remove(&slot);
                                            let frame = TouchFrame::new(
                                                active_slots.values().copied().collect(),
                                            );
                                            let _ = recognizer.process_frame(&frame);
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                is_running_clone.store(false, Ordering::SeqCst);
                log::info!("Linux libinput gesture worker terminated.");
            })?;

        self.worker_handle = Some(handle);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
        Ok(())
    }
}
