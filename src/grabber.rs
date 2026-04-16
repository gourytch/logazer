use std::sync::{Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use log::{info, trace, warn};
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;

use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

use windows_capture::window::{Window, Error};
use crossbeam_channel::{Sender, TrySendError};

use crate::imagework::image_from_frame;
use crate::types::Screenshot;

const COFFEE_BREAK_FOR_WATCHER: u64 = 1000;

const WINDOW_TITLE: &'static str = "Last Oasis  ";

// get focused Last Oasis window 
#[allow(unused)]
fn get_focused() -> Option<Window> {
    match Window::foreground() {
        Err(_) => {
            return None;
        }
        Ok(wnd) => {
            match wnd.title() {
                Err(_) => {
                    return None;
                }
                Ok(t) => {
                    if t == WINDOW_TITLE {
                        return Some(wnd);
                    } else {
                        return None;
                    }
                }
            }
        }
    }
}


#[allow(unused)]
fn get_by_title() -> Option<Window> {
    match Window::from_name(WINDOW_TITLE) {
        Err(_) => {
            return None;
        }
        Ok(wnd) => {
            return Some(wnd);
        }
    }
}

///////////////////////////////////

struct CaptureSettings {
    capture_item: Window,
    comm: Arc<CommBlock>,
}

struct Capture {
    settings: CaptureSettings
}

impl GraphicsCaptureApiHandler for Capture {
    /// The type of flags used to pass settings to the `new` function.
    type Flags = CaptureSettings;

    /// The error type that can be returned from the capture handlers.
    type Error = Box<dyn std::error::Error + Send + Sync>;

    /// Called by the library to create a new instance of the handler.
    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            settings: ctx.flags,
        })
    }

    /// Called for each new frame that is captured.
    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        trace!("[Capture] on_frame_arrived begin");
        // Construct and send the frame to processing queue
        let image = image_from_frame(frame)?;
        let ss = Screenshot::new(image);
        self.settings.comm.send_got_frame();
        match self.settings.comm.frame_tx.try_send(ss) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => { warn!("[Capture] frame dropped"); }
            Err(TrySendError::Disconnected(_)) => {
                info!("[Capture] disconnected. stop capture.");
                capture_control.stop();
                return Ok(());
            }
        }
        // if self.settings.frame_tx.send(Arc::new(cap)).is_err() { break; }

        // Check if the stop flag has been set (e.g., by Ctrl+C).
        if self.settings.comm.recv_capture_stop() {
            info!("[Capture] got stop_flag");
            // Signal the capture loop to stop.
            capture_control.stop();
            info!("[Capture] return by stop_flag [1]");
            return Ok(());
        }
        trace!("[Capture] return for continue [2]");
        Ok(())
    }

    /// Optional handler for when the capture item (e.g., a window) is closed.
    fn on_closed(&mut self) -> Result<(), Self::Error> {
        info!("[Capture] on_closed");
        // Stop the capture gracefully.
        self.settings.comm.send_capture_stop();
        Ok(())
    }
}


// Communication Block for sharing among all the threads
struct CommBlock  {
    frame_tx: Sender<Screenshot>, // where to send captured frames (managed by Main)
    watcher_started: AtomicBool, // true if Watcher process is running (managed by Watcher)
    watcher_stop_flag: AtomicBool, // true if Watcher is need to stop running (managed by Main)
    capture_started: AtomicBool, // true if Capture process is running (managed by Capture thread)
    capture_active: AtomicBool, // true if Capture process is active (managed by Capture thread)
    capture_got_frame: AtomicBool, // true if on_frame_arrived() called (managed by Capture callback & main)
    capture_stop_flag: AtomicBool, // true if Watcher is need to stop running (managed by Watcher)
}

impl CommBlock {
    pub fn new(frame_tx: Sender<Screenshot>) -> Self {
        Self {
            frame_tx: frame_tx,
            watcher_started: AtomicBool::new(false),
            watcher_stop_flag: AtomicBool::new(false),
            capture_started: AtomicBool::new(false),
            capture_active: AtomicBool::new(false),
            capture_got_frame: AtomicBool::new(false),
            capture_stop_flag: AtomicBool::new(false),
        }
    }

    #[allow(unused)]
    pub fn watcher_started_swap(&self, value: bool) -> bool {
        self.watcher_started.swap(value, Ordering::Acquire)
    }

    #[allow(unused)]
    pub fn watcher_is_started(&self) -> bool {
        self.watcher_started.load(Ordering::Relaxed)
    }

    #[allow(unused)]
    pub fn capture_started_swap(&self, value: bool) -> bool {
        self.capture_started.swap(value, Ordering::Acquire)
    }

    #[allow(unused)]
    pub fn capture_is_started(&self) -> bool {
        self.capture_started.load(Ordering::Relaxed)
    }

    #[allow(unused)]
    pub fn capture_is_active(&self) -> bool {
        self.capture_active.load(Ordering::Relaxed)
    }

    #[allow(unused)]
    pub fn send_got_frame(&self) {
        self.capture_got_frame.swap(true, Ordering::Acquire);
    }

    #[allow(unused)]
    pub fn recv_got_frame(&mut self) -> bool {
        self.capture_got_frame.swap(false, Ordering::Acquire)
    }

    #[allow(unused)]
    pub fn send_watcher_stop(&self) {
        self.watcher_stop_flag.store(true, Ordering::Relaxed); // swap(true, Ordering::Acquire);
    }

    #[allow(unused)]
    pub fn recv_watcher_stop(&self) -> bool {
        self.watcher_stop_flag.swap(false, Ordering::Acquire)
    }

    #[allow(unused)]
    pub fn send_capture_stop(&self) {
        self.capture_stop_flag.store(true, Ordering::Relaxed); // swap(true, Ordering::Acquire);
    }

    #[allow(unused)]
    pub fn recv_capture_stop(&self) -> bool {
        self.capture_stop_flag.swap(false, Ordering::Acquire)
    }
}


pub struct Watcher {
    comm: Arc<CommBlock>,
    thread_handle: Option<std::thread::JoinHandle<()>>, // my own thread
    // below this line are the things for the thread
}

impl Watcher {
    pub fn new(frame_tx: Sender<Screenshot>) -> Self {
        Self {
            comm: Arc::new(CommBlock::new(frame_tx)),
            thread_handle: None,
        }
    }

    pub fn active(&self) -> bool {
        self.comm.capture_is_started()
    }

    #[allow(unused)]
    pub fn stop(&mut self) {
        if !self.comm.watcher_is_started() {
            info!("[WATCHER::stop] watcher is not started, quit");
            return;
        }
        info!("[WATCHER::stop] send the STOP signal");
        self.comm.send_watcher_stop(); // tell the threaded process to stop
        if let Some(handle) = self.thread_handle.take() {
            info!("[WATCHER::stop] join to the thread");
            let _ = handle.join(); // should be not for long
            self.thread_handle = None;
            info!("[WATCHER::stop] ... joined");
        } else {
            info!("[WATCHER::stop] No handle - no awaiting");
        }
        self.comm.watcher_started_swap(false);
    }

    pub fn start(&mut self) {
        if self.comm.watcher_started_swap(true) {return;} // return if already started or set the started flag
        info!("watcher.start() called");
        let comm_clone = self.comm.clone();
        self.thread_handle = Some(thread::spawn(move || {
            info!("[WATCHER] window watching thread spawned");
            let mut prev_window = None;
            let mut capture_thread_handle: Option<JoinHandle<()>> = None;
            while !comm_clone.recv_watcher_stop() {
                // let window = get_focused();
                let window = get_by_title();
                if prev_window == window {
                    // everything is the same... do nothing
                    trace!("[WATCHER] nothing happened");
                    thread::sleep(Duration::from_millis(COFFEE_BREAK_FOR_WATCHER));
                    continue;
                } else {
                    info!("[WATCHER] window has been changed {:?} -> {:?}", &prev_window, &window);
                    prev_window = window; // save for lather
                    if let Some(w) = window {
                        // start capture
                        info!("[WATCHER] start capture for {:?}", &window);
                        comm_clone.recv_capture_stop(); // reset the capture flag if it is set
                        capture_thread_handle = start_capture_thread(comm_clone.clone(), w.clone());
                    } else {
                        // stop capture
                        info!("[WATCHER] stop capture");
                        comm_clone.send_capture_stop();
                        if let Some(h) = capture_thread_handle.take() {
                            let _ = h.join(); // should be not for long. at least I hope so.
                            capture_thread_handle = None;
                        }
                        info!("[WATCHER] capture stopped");
                    }
                }
            }
            comm_clone.watcher_started_swap(false);
            info!("[WATCHER] window watching thread finished");
        }));
    }


}

fn start_capture_thread(comm: Arc<CommBlock>, window: Window) -> Option<JoinHandle<()>> {
    info!("start_capture_thread(window={:?}) called", &window);
    let window_clone = window.clone();
    let comm_clone = comm.clone();
    
    let handle = Some(thread::spawn(move || {
        info!("[CAPTURE] capture thread spawned");
        let settings = CaptureSettings {
            capture_item: window_clone,
            comm: comm_clone.clone(),
        };
        info!("[CAPTURE] call start_capture ...");
        comm.capture_started_swap(true);
        match do_capture_session(settings) {
            Ok(_) => { info!("start_capture returned normally"); }
            Err(err) => { warn!("start_capture returned with error {}", err); }
        }
        comm.capture_started_swap(false);
        info!("[CAPTURE] thread finished");
    }));
    return handle;
}   

/// Starts the capture process with the specified settings.
fn do_capture_session(settings: CaptureSettings) -> Result<(), Error> {
    // Create the settings struct for the capture session.
    let capture_settings = Settings::new(
        settings.capture_item,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Include,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        // BGRA8 is the default and most common format.
        ColorFormat::Bgra8,
        // but we will use Rgba8
        // ColorFormat::Rgba8,
        settings,
    );

    // Start the capture and take control of the current thread.
    // Any errors from the capture handler will be propagated here.
    match Capture::start(capture_settings) {
        Ok(_) => { info!("Capture::start finished"); },
        Err(err) => { warn!("Capture::start returned with error {}", err); }
    }
    Ok(())
}

