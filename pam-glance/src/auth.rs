use crate::camera::{SmartCamera, CameraType};
use crate::config::GlanceConfig;
use crate::face::{FaceRecognizer, load_all_faces};
use crate::ir_emitter::IrEmitter;

use anyhow::Result;
use log::{info, debug, warn, error};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

pub fn authenticate(config: &AuthConfig) -> AuthResult {
    let start_time = Instant::now();
    
    info!("Starting face authentication (timeout: {:?})", config.timeout);
    
    let mut ir_emitter = if config.enable_ir_emitter && config.prefer_ir {
        let mut emitter = IrEmitter::new(&config.ir_device);
        if let Err(e) = emitter.enable() {
            warn!("Failed to enable IR emitter: {}", e);
        }
        Some(emitter)
    } else {
        None
    };
    
    let recognizer = match FaceRecognizer::new(&config.models_dir, config.tolerance) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to initialize face recognizer: {}", e);
            return AuthResult::Error(format!("Face recognizer init failed: {}", e));
        }
    };
    
    let registered_faces = match load_registered_faces(config) {
        Ok(faces) => {
            if faces.is_empty() {
                warn!("No registered faces found");
                return AuthResult::Error("No registered faces".to_string());
            }
            faces
        }
        Err(e) => {
            error!("Failed to load registered faces: {}", e);
            return AuthResult::Error(format!("Failed to load faces: {}", e));
        }
    };
    
    info!("Loaded {} registered user(s)", registered_faces.len());
    
    let mut camera = match SmartCamera::open(
        config.prefer_ir, 
        &config.ir_device, 
        &config.rgb_device
    ) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to open camera: {}", e);
            return AuthResult::Error(format!("Camera failed: {}", e));
        }
    };
    
    let camera_type = if camera.is_ir { CameraType::Infrared } else { CameraType::Rgb };
    info!("Using {:?} camera", camera_type);
    
    if let Ok(bright_enough) = camera.check_brightness(config.min_brightness) {
        if !bright_enough {
            if camera.is_ir {
                warn!("IR camera too dark, IR emitter may not be active");
            } else {
                warn!("Camera too dark for reliable detection");
            }
        }
    }
    
    let mut frames_processed = 0;
    let mut faces_detected = 0;
    
    loop {
        if start_time.elapsed() >= config.timeout {
            info!("Authentication timeout ({} frames, {} faces)", 
                  frames_processed, faces_detected);
            return AuthResult::Timeout;
        }
        
        let frame = match camera.read() {
            Ok(f) => f,
            Err(e) => {
                debug!("Frame capture failed: {}", e);
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
                
                return AuthResult::Success {
                    username,
                    confidence: 1.0 - distance,
                    camera_type,
                };
            }
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
