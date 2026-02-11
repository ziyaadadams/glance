use crate::camera::{SmartCamera, CameraType, CameraInfo, detect_cameras_fast};
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
    pub ir_tolerance: f64,
    pub rgb_tolerance: f64,
    pub target_user: Option<String>,
    pub min_brightness: f64,
    pub enable_ir_emitter: bool,
    pub ir_device: String,
    pub rgb_device: String,
    pub max_frames_per_camera: u32,
    pub frame_delay_ms: u64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(3),
            prefer_ir: true,
            data_dir: PathBuf::from("/var/lib/glance"),
            models_dir: PathBuf::from("/usr/share/glance/models"),
            tolerance: 0.6,
            ir_tolerance: 0.45,
            rgb_tolerance: 0.50,
            target_user: None,
            min_brightness: 20.0,
            enable_ir_emitter: true,
            ir_device: "/dev/video2".to_string(),
            rgb_device: "/dev/video0".to_string(),
            max_frames_per_camera: 15,
            frame_delay_ms: 33,      // ~30 FPS
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
            ir_tolerance: config.recognition.ir_tolerance,
            rgb_tolerance: config.recognition.rgb_tolerance,
            target_user: None,
            min_brightness: config.camera.min_brightness,
            enable_ir_emitter: config.ir_emitter.enabled,
            ir_device: config.camera.ir_device,
            rgb_device: config.camera.rgb_device,
            max_frames_per_camera: 15,
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
    let config_clone = config.clone();
    
    let (tx, rx) = mpsc::channel();
    
    let handle = thread::spawn(move || {
        let result = authenticate_inner(&config_clone);
        let _ = tx.send(result);
    });
    
    // Hard timeout = configured timeout + 500ms grace
    match rx.recv_timeout(timeout + Duration::from_millis(500)) {
        Ok(result) => {
            let _ = handle.join();
            result
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            warn!("Hard timeout — killing stale auth worker");
            let _ = std::process::Command::new("pkill")
                .arg("-f")
                .arg("linux-enable-ir-emitter")
                .output();
            AuthResult::Timeout
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            error!("Auth worker panicked");
            let _ = std::process::Command::new("pkill")
                .arg("-f")
                .arg("linux-enable-ir-emitter")
                .output();
            AuthResult::Error("Internal error".to_string())
        }
    }
}

/// Fast authentication: detect cameras via sysfs, open directly, try a handful
/// of frames per camera. Fails fast so PAM falls through to password.
fn authenticate_inner(config: &AuthConfig) -> AuthResult {
    let start_time = Instant::now();
    let frame_delay = Duration::from_millis(config.frame_delay_ms);
    
    info!("Glance auth starting (timeout: {:?})", config.timeout);
    
    // --- IR emitter (always try if enabled — it will skip gracefully if not installed) ---
    let mut ir_emitter: Option<IrEmitter> = if config.enable_ir_emitter {
        let mut emitter = IrEmitter::new(&config.ir_device);
        if let Err(e) = emitter.enable() {
            warn!("IR emitter failed: {}", e);
            None
        } else if emitter.is_running() {
            Some(emitter)
        } else {
            debug!("IR emitter not running (tool may not be installed)");
            None
        }
    } else {
        None
    };
    
    macro_rules! cleanup_and_return {
        ($result:expr) => {{
            if let Some(ref mut emitter) = ir_emitter {
                let _ = emitter.disable();
            }
            drop(ir_emitter.take());
            $result
        }};
    }
    
    // --- Load registered faces ---
    let registered_faces = match load_registered_faces(config) {
        Ok(faces) if !faces.is_empty() => faces,
        Ok(_) => {
            warn!("No registered faces — use your password");
            return cleanup_and_return!(AuthResult::NoMatch);
        }
        Err(e) => {
            error!("Failed to load faces: {}", e);
            return cleanup_and_return!(AuthResult::Error(format!("Load faces: {}", e)));
        }
    };
    
    info!("Loaded {} registered user(s)", registered_faces.len());
    
    if start_time.elapsed() >= config.timeout {
        return cleanup_and_return!(AuthResult::Timeout);
    }
    
    // --- Fast camera detection (sysfs only, near-instant) ---
    let cameras = match detect_cameras_fast() {
        Ok(c) if !c.is_empty() => c,
        Ok(_) => {
            error!("No cameras detected");
            return cleanup_and_return!(AuthResult::Error("No cameras found".to_string()));
        }
        Err(e) => {
            error!("Camera detection failed: {}", e);
            return cleanup_and_return!(AuthResult::Error(format!("Detection: {}", e)));
        }
    };
    
    // Sort: preferred camera type first, but always include both IR and RGB
    let sorted_cameras: Vec<&CameraInfo> = if config.prefer_ir {
        cameras.iter()
            .filter(|c| c.camera_type == CameraType::Infrared)
            .chain(cameras.iter().filter(|c| c.camera_type == CameraType::Rgb))
            .chain(cameras.iter().filter(|c| c.camera_type == CameraType::Unknown))
            .collect()
    } else {
        cameras.iter()
            .filter(|c| c.camera_type == CameraType::Rgb)
            .chain(cameras.iter().filter(|c| c.camera_type == CameraType::Infrared))
            .chain(cameras.iter().filter(|c| c.camera_type == CameraType::Unknown))
            .collect()
    };
    
    // --- Try each camera quickly ---
    for cam_info in &sorted_cameras {
        if start_time.elapsed() >= config.timeout {
            break;
        }
        
        let is_ir = cam_info.camera_type == CameraType::Infrared;
        let tolerance = if is_ir { config.ir_tolerance } else { config.rgb_tolerance };
        let camera_label = if is_ir { "IR" } else { "RGB" };
        
        info!("Trying {} camera video{} (tolerance: {:.2})", 
              camera_label, cam_info.device_id, tolerance);
        
        // Init recognizer
        let recognizer = match FaceRecognizer::new(&config.models_dir, tolerance) {
            Ok(r) => r,
            Err(e) => {
                error!("Recognizer init failed: {}", e);
                continue;
            }
        };
        
        // Open camera directly — no redundant detection
        let mut camera = match SmartCamera::open_direct(cam_info) {
            Ok(c) => c,
            Err(e) => {
                warn!("{} camera open failed: {}", camera_label, e);
                continue;
            }
        };
        
        // Use actual camera type (in case name detection was wrong)
        let camera_type = if camera.is_ir { CameraType::Infrared } else { CameraType::Rgb };
        let effective_tolerance = if camera.is_ir { config.ir_tolerance } else { config.rgb_tolerance };
        let recognizer = if (effective_tolerance - tolerance).abs() > 0.001 {
            match FaceRecognizer::new(&config.models_dir, effective_tolerance) {
                Ok(r) => r,
                Err(_) => recognizer,
            }
        } else {
            recognizer
        };
        
        // --- Quick frame loop ---
        let mut frames: u32 = 0;
        let mut faces_seen: u32 = 0;
        let mut consecutive_failures: u32 = 0;
        
        loop {
            if start_time.elapsed() >= config.timeout {
                info!("{}: timeout after {} frames", camera_label, frames);
                break;
            }
            
            if frames >= config.max_frames_per_camera {
                info!("{}: {} frames processed, {} faces — moving on",
                      camera_label, frames, faces_seen);
                break;
            }
            
            if consecutive_failures >= 5 {
                warn!("{}: too many read failures", camera_label);
                break;
            }
            
            if frames > 0 {
                thread::sleep(frame_delay);
            }
            
            let frame = match camera.read() {
                Ok(f) => {
                    consecutive_failures = 0;
                    f
                }
                Err(_) => {
                    consecutive_failures += 1;
                    continue;
                }
            };
            
            frames += 1;
            
            let faces = match recognizer.detect_faces(&frame) {
                Ok(f) if !f.is_empty() => f,
                _ => continue,
            };
            
            faces_seen += 1;
            debug!("{}: {} face(s) in frame {}", camera_label, faces.len(), frames);
            
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
                    info!("Authenticated '{}' via {:?} in {:?} (distance: {:.4})",
                          username, camera_type, elapsed, distance);
                    
                    return cleanup_and_return!(AuthResult::Success {
                        username,
                        confidence: 1.0 - distance,
                        camera_type,
                    });
                }
            }
        }
        
        drop(camera);
    }
    
    // All cameras tried — face auth failed
    let elapsed = start_time.elapsed();
    info!("Face not recognized after {:?} — use your password", elapsed);
    cleanup_and_return!(AuthResult::NoMatch)
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
