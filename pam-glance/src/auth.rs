use crate::camera::{SmartCamera, CameraType};
use crate::config::GlanceConfig;
use crate::face::{FaceRecognizer, load_all_faces};
use crate::ir_emitter::IrEmitter;

use anyhow::Result;
use log::{info, debug, warn, error};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone)]
pub enum AuthResult {
    Success {
        username: String,
        confidence: f64,
        camera_type: CameraType,
    },
    NoFaceDetected,
    NoMatch,
    Error(String),
    Timeout,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub timeout: Duration,
    pub prefer_ir: bool,
    pub data_dir: PathBuf,
    pub models_dir: PathBuf,
    pub tolerance: f64,
    pub target_user: Option<String>,
    pub min_brightness: f64,
    pub enable_ir_emitter: bool,
    pub ir_device: String,
    pub rgb_device: String,
    pub max_attempts: u32,
    pub frame_delay_ms: u64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            prefer_ir: true,
            data_dir: PathBuf::from("/var/lib/glance"),
            models_dir: PathBuf::from("/usr/share/glance/models"),
            tolerance: 0.6,
            target_user: None,
            min_brightness: 20.0,
            enable_ir_emitter: true,
            ir_device: "/dev/video2".to_string(),
            rgb_device: "/dev/video0".to_string(),
            max_attempts: 150,       // Max frames to process before giving up
            frame_delay_ms: 33,      // ~30 FPS, prevents CPU spinning
        }
    }
}

impl AuthConfig {
    pub fn from_file(path: &Path) -> Result<Self> {
        let path_str = path.to_str().unwrap_or("");
        let config = GlanceConfig::load(path_str)?;
        
        Ok(Self {
            timeout: Duration::from_secs_f64(config.recognition.auth_timeout),
            prefer_ir: config.camera.prefer_ir,
            data_dir: PathBuf::from("/var/lib/glance"),
            models_dir: PathBuf::from("/usr/share/glance/models"),
            tolerance: if config.camera.prefer_ir { 
                config.recognition.ir_tolerance 
            } else { 
                config.recognition.rgb_tolerance 
            },
            target_user: None,
            min_brightness: config.camera.min_brightness,
            enable_ir_emitter: config.ir_emitter.enabled,
            ir_device: config.camera.ir_device,
            rgb_device: config.camera.rgb_device,
            max_attempts: 150,
            frame_delay_ms: 33,
        })
    }
    
    pub fn load() -> Self {
        if let Some(home) = std::env::var_os("HOME") {
            let user_config = Path::new(&home).join(".config/glance/config.json");
            if let Ok(config) = Self::from_file(&user_config) {
                return config;
            }
        }
        
        let system_config = Path::new("/etc/glance/config.json");
        if let Ok(config) = Self::from_file(system_config) {
            return config;
        }
        
        Self::default()
    }
}

/// Wrapper to run authentication with a hard timeout using a separate thread.
/// This ensures we never block indefinitely even if camera operations hang.
pub fn authenticate(config: &AuthConfig) -> AuthResult {
    let timeout = config.timeout;
    
    // Clone what we need for the thread
    let config_clone = config.clone();
    
    let (tx, rx) = mpsc::channel();
    
    // Spawn auth worker thread
    let handle = thread::spawn(move || {
        let result = authenticate_inner(&config_clone);
        let _ = tx.send(result);
    });
    
    // Wait for result with timeout
    match rx.recv_timeout(timeout + Duration::from_millis(500)) {
        Ok(result) => {
            // Clean up thread (it should be done)
            let _ = handle.join();
            result
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            warn!("Authentication hard timeout - worker thread may be stuck");
            // Try to kill any orphaned IR emitter processes
            let _ = std::process::Command::new("pkill")
                .arg("-f")
                .arg("linux-enable-ir-emitter")
                .output();
            AuthResult::Timeout
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            error!("Authentication worker thread panicked");
            // Try to kill any orphaned IR emitter processes
            let _ = std::process::Command::new("pkill")
                .arg("-f")
                .arg("linux-enable-ir-emitter")
                .output();
            AuthResult::Error("Internal error".to_string())
        }
    }
}

/// Inner authentication function that does the actual work.
/// This runs in a separate thread so we can enforce a hard timeout.
fn authenticate_inner(config: &AuthConfig) -> AuthResult {
    let start_time = Instant::now();
    let frame_delay = Duration::from_millis(config.frame_delay_ms);
    
    info!("Starting face authentication (timeout: {:?})", config.timeout);
    
    // Check timeout immediately
    if start_time.elapsed() >= config.timeout {
        return AuthResult::Timeout;
    }
    
    // Try to enable IR emitter - we'll manage its lifetime explicitly
    let mut ir_emitter: Option<IrEmitter> = if config.enable_ir_emitter && config.prefer_ir {
        let mut emitter = IrEmitter::new(&config.ir_device);
        if let Err(e) = emitter.enable() {
            warn!("Failed to enable IR emitter: {}", e);
            None
        } else {
            Some(emitter)
        }
    } else {
        None
    };
    
    // Helper macro to cleanup IR emitter before returning
    macro_rules! cleanup_and_return {
        ($result:expr) => {{
            if let Some(ref mut emitter) = ir_emitter {
                let _ = emitter.disable();
            }
            drop(ir_emitter.take());
            $result
        }};
    }
    
    // Check timeout after IR setup
    if start_time.elapsed() >= config.timeout {
        return cleanup_and_return!(AuthResult::Timeout);
    }
    
    let recognizer = match FaceRecognizer::new(&config.models_dir, config.tolerance) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to initialize face recognizer: {}", e);
            return cleanup_and_return!(AuthResult::Error(format!("Face recognizer init failed: {}", e)));
        }
    };
    
    // Check timeout after recognizer init
    if start_time.elapsed() >= config.timeout {
        return cleanup_and_return!(AuthResult::Timeout);
    }
    
    let registered_faces = match load_registered_faces(config) {
        Ok(faces) => {
            if faces.is_empty() {
                warn!("No registered faces found");
                return cleanup_and_return!(AuthResult::NoMatch);  // Return NoMatch instead of Error to allow password fallback
            }
            faces
        }
        Err(e) => {
            error!("Failed to load registered faces: {}", e);
            return cleanup_and_return!(AuthResult::Error(format!("Failed to load faces: {}", e)));
        }
    };
    
    info!("Loaded {} registered user(s)", registered_faces.len());
    
    // Check timeout before camera open (this can be slow/blocking)
    if start_time.elapsed() >= config.timeout {
        return cleanup_and_return!(AuthResult::Timeout);
    }
    
    // Try to open camera with a quick check
    let mut camera = match open_camera_with_timeout(config, Duration::from_secs(2)) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to open camera: {}", e);
            // Return NoFaceDetected instead of Error so PAM falls through to password
            return cleanup_and_return!(AuthResult::NoFaceDetected);
        }
    };
    
    let camera_type = if camera.is_ir { CameraType::Infrared } else { CameraType::Rgb };
    info!("Using {:?} camera", camera_type);
    
    // Skip brightness check to save time, go straight to detection
    
    let mut frames_processed: u32 = 0;
    let mut faces_detected = 0;
    let mut consecutive_failures: u32 = 0;
    const MAX_CONSECUTIVE_FAILURES: u32 = 10;
    
    loop {
        // Check timeout at start of each iteration
        if start_time.elapsed() >= config.timeout {
            info!("Authentication timeout ({} frames, {} faces)", 
                  frames_processed, faces_detected);
            return cleanup_and_return!(AuthResult::Timeout);
        }
        
        // Check max attempts to prevent infinite loops
        if frames_processed >= config.max_attempts {
            info!("Max attempts reached ({} frames)", frames_processed);
            return cleanup_and_return!(AuthResult::Timeout);
        }
        
        // Rate limiting - sleep between frames to prevent CPU spinning
        if frames_processed > 0 {
            thread::sleep(frame_delay);
        }
        
        // Check timeout again after sleep
        if start_time.elapsed() >= config.timeout {
            return cleanup_and_return!(AuthResult::Timeout);
        }
        
        let frame = match camera.read() {
            Ok(f) => {
                consecutive_failures = 0;
                f
            }
            Err(e) => {
                consecutive_failures += 1;
                debug!("Frame capture failed ({}): {}", consecutive_failures, e);
                
                // If camera keeps failing, give up
                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    error!("Too many consecutive camera failures");
                    return cleanup_and_return!(AuthResult::NoFaceDetected);
                }
                
                thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        
        frames_processed += 1;
        
        let faces = match recognizer.detect_faces(&frame) {
            Ok(f) => f,
            Err(e) => {
                debug!("Face detection failed: {}", e);
                continue;
            }
        };
        
        if faces.is_empty() {
            continue;
        }
        
        faces_detected += 1;
        debug!("Detected {} face(s) in frame {}", faces.len(), frames_processed);
        
        for face in &faces {
            let faces_to_check: Vec<_> = if let Some(ref target) = config.target_user {
                registered_faces.iter()
                    .filter(|(u, _)| u == target)
                    .cloned()
                    .collect()
            } else {
                registered_faces.clone()
            };
            
            if let Some((username, distance)) = recognizer.match_face(&face.encoding, &faces_to_check) {
                let elapsed = start_time.elapsed();
                info!("Authentication successful for '{}' in {:?} (distance: {:.4})", 
                      username, elapsed, distance);
                
                return cleanup_and_return!(AuthResult::Success {
                    username,
                    confidence: 1.0 - distance,
                    camera_type,
                });
            }
        }
    }
}

/// Open camera with a timeout to prevent blocking indefinitely
fn open_camera_with_timeout(config: &AuthConfig, timeout: Duration) -> Result<SmartCamera> {
    let (tx, rx) = mpsc::channel();
    
    let prefer_ir = config.prefer_ir;
    let ir_device = config.ir_device.clone();
    let rgb_device = config.rgb_device.clone();
    
    thread::spawn(move || {
        let result = SmartCamera::open(prefer_ir, &ir_device, &rgb_device);
        let _ = tx.send(result);
    });
    
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(_) => {
            anyhow::bail!("Camera open timeout")
        }
    }
}

fn load_registered_faces(config: &AuthConfig) -> Result<Vec<(String, Vec<Vec<f64>>)>> {
    if config.data_dir.exists() {
        let faces = load_all_faces(&config.data_dir)?;
        if !faces.is_empty() {
            return Ok(faces);
        }
    }
    
    if let Some(home) = std::env::var_os("HOME") {
        let home_path = Path::new(&home);
        
        // Check XDG data directory first
        let xdg_data = home_path.join(".local/share/glance");
        if xdg_data.exists() {
            let faces = load_all_faces(&xdg_data)?;
            if !faces.is_empty() {
                return Ok(faces);
            }
        }
        
        // Fall back to config directory
        let user_config = home_path.join(".config/glance");
        if user_config.exists() {
            let faces = load_all_faces(&user_config)?;
            if !faces.is_empty() {
                return Ok(faces);
            }
        }
    }
    
    let system_data = Path::new("/var/lib/glance");
    if system_data.exists() {
        return load_all_faces(system_data);
    }
    
    Ok(Vec::new())
}

pub fn authenticate_quick(username: &str, timeout_secs: u64) -> bool {
    let mut config = AuthConfig::load();
    config.target_user = Some(username.to_string());
    config.timeout = Duration::from_secs(timeout_secs);
    
    matches!(authenticate(&config), AuthResult::Success { .. })
}

pub fn authenticate_any(timeout_secs: u64) -> Option<String> {
    let mut config = AuthConfig::load();
    config.timeout = Duration::from_secs(timeout_secs);
    
    match authenticate(&config) {
        AuthResult::Success { username, .. } => Some(username),
        _ => None,
    }
}
