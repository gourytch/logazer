use std::sync::{Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

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
use crate::types::{Meta, Screenshot};

const WINDOW_TITLE: &'static str = "Last Oasis  ";
const COFFEE_BREAK_FOR_WATCHER: u64 = 100;

// get focused Last Oasis window 
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

///////////////////////////////////

struct CaptureSettings {
    capture_item: Window,
    stop_flag: Arc<AtomicBool>,
    is_capturing: Arc<AtomicBool>,
    frame_tx: Sender<Screenshot>,
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
        eprintln!("on_frame_arrived begin");
        // Construct and send the frame to processing queue
        let image = image_from_frame(frame)?;
        let ss = Screenshot {
            pit_captured: Instant::now(),
            pit_received: None,
            pit_parsed: None,
            image: image,
            meta: Meta::empty(),            
        };

        match self.settings.frame_tx.try_send(ss) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => { eprintln!("frame dropped"); }
            Err(TrySendError::Disconnected(_)) => {
                eprintln!("disconnected. stop capture.");
                self.settings.is_capturing.store(false, Ordering::Relaxed);
                capture_control.stop();
                return Ok(());
            }
        }
        // if self.settings.frame_tx.send(Arc::new(cap)).is_err() { break; }

        // Check if the stop flag has been set (e.g., by Ctrl+C).
        if self.settings.stop_flag.load(Ordering::Relaxed) {
            eprintln!("on_frame_arrived got stop_flag");
            self.settings.is_capturing.store(false, Ordering::Relaxed);
            // Signal the capture loop to stop.
            capture_control.stop();
            return Ok(());
        }
        self.settings.is_capturing.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Optional handler for when the capture item (e.g., a window) is closed.
    fn on_closed(&mut self) -> Result<(), Self::Error> {
        eprintln!("on_closed begin");
        // Stop the capture gracefully.
        self.settings.stop_flag.store(true, Ordering::Relaxed);
        eprintln!("on_closed end");
        Ok(())
    }
}

pub struct Watcher {
    frame_tx: Sender<Screenshot>, // where to send captured frames
    running: Arc<AtomicBool>, // true if Watcher is already running
    capturing_started: Arc<AtomicBool>, // true if Capture is active
    is_capturing: Arc<AtomicBool>, // true if on_frame_arrived() called
    stop_flag: Arc<AtomicBool>, // true if Watcher is need to stop running
    thread_handle: Option<std::thread::JoinHandle<()>>, // my own thread
    // below this line is the things for the thread
}

impl Watcher {
    pub fn new(frame_tx: Sender<Screenshot>) -> Self {
        Self {
            frame_tx: frame_tx,
            running: Arc::new(AtomicBool::new(false)),
            capturing_started: Arc::new(AtomicBool::new(false)),
            is_capturing: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
        }
    }
    
    #[allow(unused)] 
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn is_capturing(&self) -> bool {
        self.is_capturing.load(Ordering::Relaxed)
    }

    pub fn is_grabbing(&self) -> bool {
        self.capturing_started.load(Ordering::Relaxed)
    }

    #[allow(unused)]
    pub fn stop(&mut self) {
        if !self.is_running() {
            return;
        }
        self.stop_flag.store(true, Ordering::Relaxed); // tell the threaded process to stop
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join(); // should be not for long
            self.thread_handle = None;

        }
    }

    pub fn start(&mut self) {
        eprintln!("watcher.start() called");

        if self.running.swap(true, Ordering::Acquire) {return;}

        self.capturing_started.store(false, Ordering::Relaxed);
        self.stop_flag.store(false, Ordering::Relaxed);

        let frame_tx = self.frame_tx.clone();
        let running = Arc::clone(&self.running);
        let capturing_started = Arc::clone(&self.capturing_started);
        let is_capturing = Arc::clone(&self.is_capturing);
        let stop_flag = Arc::clone(&self.stop_flag);
        
        self.thread_handle = Some(thread::spawn(move || {
            println!("[WATCHER] spawned");
            let mut prev_window: Option<Window> = None;
            let stop_grabbing: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

            while !stop_flag.load(Ordering::Relaxed) {
                let new_window = get_focused();
                if new_window != prev_window {
                    // Let' assume we DON'T HAVE multiple LO windows,
                    // so we're start recording when we got new active LO window 
                    // AND the record is is not started                    
                    if let Some(fg) = new_window {
                        // stop previous grabbing if need
                        if capturing_started.load(Ordering::Relaxed) {
                            println!("capture process already exists");
                        } else {
                            // start new grabbing
                            println!("[WATCHER] start capture process");
                            let settings = CaptureSettings {
                                capture_item: fg.clone(),
                                stop_flag: stop_grabbing.clone(),
                                is_capturing: is_capturing.clone(),
                                frame_tx: frame_tx.clone(),
                            };
                            start_capture(settings).expect("start_capture error");
                            capturing_started.store(true, Ordering::Relaxed);
                        }
                    } else {
                        // stop grabbing if need
                        if capturing_started.load(Ordering::Relaxed) {
                            println!("[WATCHER] stop capture process");
                            stop_grabbing.store(true, Ordering::Relaxed);
                            capturing_started.store(false, Ordering::Relaxed);
                        }
                    }
                    prev_window = new_window;
                    println!("... capturing_started = {:?}", capturing_started.load(Ordering::Relaxed));
                }
                thread::sleep(Duration::from_millis(COFFEE_BREAK_FOR_WATCHER));
            }
            println!("[WATCHER] loop finished");
            if capturing_started.load(Ordering::Relaxed) {
                stop_grabbing.store(true, Ordering::Relaxed);
                capturing_started.store(false, Ordering::Relaxed);
            }
            running.store(false, Ordering::Relaxed); // I am not the king anymore
            println!("[WATCHER] finished");
        }));
    }   
}

/// Starts the capture process with the specified settings.
fn start_capture(settings: CaptureSettings) -> Result<(), Error> {

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
    Capture::start(capture_settings).expect("Screen capture failed");
    Ok(())
}
