//! Windows Precision Touchpad (PTP) Raw Input backend implementation.

use crate::backend::GestureBackend;
use crate::gesture::{Gesture, GestureRecognizer, GestureSensitivity, TouchFrame, TouchPoint};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
    RAWINPUTHEADER, RIDEV_INPUTSINK, RID_INPUT, RIM_TYPEHID,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    PostMessageW, PostQuitMessage, RegisterClassExW, TranslateMessage, MSG,
    WINDOW_EX_STYLE, WM_DESTROY, WM_INPUT, WM_USER, WNDCLASSEXW, WS_POPUP,
};

const WM_STOP_CAPTURE: u32 = WM_USER + 101;

pub struct WindowsPrecisionTouchpadBackend {
    worker_handle: Option<thread::JoinHandle<()>>,
    target_hwnd: Option<isize>,
    is_running: Arc<AtomicBool>,
}

unsafe impl Send for WindowsPrecisionTouchpadBackend {}

impl WindowsPrecisionTouchpadBackend {
    pub fn new() -> Self {
        Self {
            worker_handle: None,
            target_hwnd: None,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }
}

struct WorkerContext {
    sender: Sender<Gesture>,
    recognizer: GestureRecognizer,
}

static WORKER_CTX: Mutex<Option<WorkerContext>> = Mutex::new(None);

impl GestureBackend for WindowsPrecisionTouchpadBackend {
    fn start(
        &mut self,
        sender: Sender<Gesture>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.is_running.store(true, Ordering::SeqCst);
        let is_running_clone = self.is_running.clone();
        let (hwnd_tx, hwnd_rx) = crossbeam_channel::bounded::<isize>(1);

        let handle = thread::Builder::new()
            .name("win-ptp-backend".into())
            .spawn(move || {
                {
                    let mut lock = WORKER_CTX.lock().unwrap();
                    *lock = Some(WorkerContext {
                        sender,
                        recognizer: GestureRecognizer::new(GestureSensitivity::default()),
                    });
                }

                let hwnd = match create_hidden_input_window() {
                    Ok(h) => h,
                    Err(e) => {
                        log::error!("Failed to create Windows input window: {}", e);
                        is_running_clone.store(false, Ordering::SeqCst);
                        return;
                    }
                };

                let _ = hwnd_tx.send(hwnd.0 as isize);

                // Register for Precision Touchpad Raw Input
                // Usage Page 0x0D = Digitizer, Usage 0x05 = Touch Pad
                let raw_device = RAWINPUTDEVICE {
                    usUsagePage: 0x0D,
                    usUsage: 0x05,
                    dwFlags: RIDEV_INPUTSINK,
                    hwndTarget: hwnd,
                };

                unsafe {
                    if let Err(e) = RegisterRawInputDevices(
                        &[raw_device],
                        std::mem::size_of::<RAWINPUTDEVICE>() as u32,
                    ) {
                        log::error!("Failed to register Precision Touchpad Raw Input device: {:?}", e);
                        let _ = DestroyWindow(hwnd);
                        is_running_clone.store(false, Ordering::SeqCst);
                        return;
                    }
                }

                log::info!("Windows Precision Touchpad raw input capture initialized successfully.");

                // Win32 message pump
                let mut msg = MSG::default();
                unsafe {
                    while GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() {
                        if msg.message == WM_STOP_CAPTURE {
                            let _ = DestroyWindow(hwnd);
                            PostQuitMessage(0);
                            continue;
                        }
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }

                {
                    let mut lock = WORKER_CTX.lock().unwrap();
                    *lock = None;
                }
                is_running_clone.store(false, Ordering::SeqCst);
                log::info!("Windows Precision Touchpad capture worker terminated.");
            })?;

        let hwnd_raw = hwnd_rx.recv_timeout(std::time::Duration::from_secs(3))?;
        self.target_hwnd = Some(hwnd_raw);
        self.worker_handle = Some(handle);

        Ok(())
    }

    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(hwnd_raw) = self.target_hwnd.take() {
            unsafe {
                let hwnd = HWND(hwnd_raw as *mut _);
                let _ = PostMessageW(hwnd, WM_STOP_CAPTURE, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
        Ok(())
    }
}

unsafe extern "system" fn raw_input_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_INPUT => {
            let mut size: u32 = 0;
            let header_size = std::mem::size_of::<RAWINPUTHEADER>() as u32;

            GetRawInputData(
                HRAWINPUT(lparam.0 as _),
                RID_INPUT,
                None,
                &mut size,
                header_size,
            );

            if size > 0 {
                let mut buffer = vec![0u8; size as usize];
                let bytes_read = GetRawInputData(
                    HRAWINPUT(lparam.0 as _),
                    RID_INPUT,
                    Some(buffer.as_mut_ptr() as *mut _),
                    &mut size,
                    header_size,
                );

                if bytes_read == size {
                    let raw_input = &*(buffer.as_ptr() as *const RAWINPUT);
                    if raw_input.header.dwType == RIM_TYPEHID.0 {
                        let hid = &raw_input.data.hid;
                        let raw_len = (hid.dwSizeHid * hid.dwCount) as usize;
                        let raw_slice = std::slice::from_raw_parts(
                            hid.bRawData.as_ptr(),
                            raw_len,
                        );

                        process_hid_report(raw_slice);
                    }
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn create_hidden_input_window() -> Result<HWND, Box<dyn std::error::Error + Send + Sync>> {
    let class_name = w!("GestureManagerPTPClass");
    let window_title = w!("GestureManagerPTPWindow");

    unsafe {
        let hinstance = GetModuleHandleW(PCWSTR::null())?;
        let wnd_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(raw_input_wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };

        RegisterClassExW(&wnd_class);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            window_title,
            WS_POPUP,
            0,
            0,
            0,
            0,
            HWND(std::ptr::null_mut()),
            None,
            hinstance,
            None,
        )?;

        Ok(hwnd)
    }
}

/// Parse Windows Precision Touchpad (PTP) HID multi-touch reports into contact points.
fn process_hid_report(bytes: &[u8]) {
    if bytes.len() < 3 {
        return;
    }

    // Windows PTP reports typically begin with a Report ID followed by contact packets.
    // Each standard contact descriptor is 5 to 10 bytes:
    // [Flags (TipSwitch/InRange), ContactID, X_low, X_high, Y_low, Y_high, ...]
    let mut points = Vec::new();
    let mut offset = 1; // Skip potential Report ID

    while offset + 5 < bytes.len() {
        let flags = bytes[offset];
        let tip_switch = (flags & 0x01) != 0;
        let in_range = (flags & 0x02) != 0;

        let contact_id = bytes[offset + 1] as i32;
        let x = u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]) as f64;
        let y = u16::from_le_bytes([bytes[offset + 4], bytes[offset + 5]]) as f64;

        if tip_switch && in_range && x > 0.0 && y > 0.0 {
            points.push(TouchPoint {
                id: contact_id,
                x,
                y,
            });
        }

        offset += 6;
    }

    let frame = TouchFrame::new(points);

    if let Ok(mut lock) = WORKER_CTX.lock() {
        if let Some(ctx) = lock.as_mut() {
            if let Some(gesture) = ctx.recognizer.process_frame(&frame) {
                log::info!("Windows PTP recognized gesture: {}", gesture);
                let _ = ctx.sender.send(gesture);
            }
        }
    }
}
