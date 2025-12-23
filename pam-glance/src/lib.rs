mod config;
mod camera;
mod face;
mod auth;
mod ir_emitter;

use pam::{PamHandle, PamModule, PamReturnCode, export_pam_module, get_user};
use std::ffi::CStr;
use std::os::raw::c_uint;
use log::{info, error};

pub struct PamGlance;

export_pam_module!(PamGlance);

impl PamModule for PamGlance {
    fn authenticate(handle: &PamHandle, args: Vec<&CStr>, _flags: c_uint) -> PamReturnCode {
        init_logging();
        
        let config = match parse_args(&args) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to parse PAM arguments: {}", e);
                return PamReturnCode::Auth_Err;
            }
        };
        
        let username = match get_user(handle, None) {
            Ok(u) => u.to_string(),
            Err(e) => {
                error!("Failed to get username: {:?}", e);
                return PamReturnCode::User_Unknown;
            }
        };
        
        info!("Glance authentication attempt for user: {}", username);
        
        let mut auth_config = auth::AuthConfig::load();
        auth_config.target_user = Some(username.clone());
        auth_config.timeout = std::time::Duration::from_secs_f64(config.timeout);
        auth_config.prefer_ir = config.prefer_ir;
        
        if !config.data_dir.is_empty() {
            auth_config.data_dir = std::path::PathBuf::from(&config.data_dir);
        }
        
        match auth::authenticate(&auth_config) {
            auth::AuthResult::Success { username: matched_user, confidence, camera_type } => {
                info!("Glance: User '{}' authenticated via {:?} (confidence: {:.2})", 
                      matched_user, camera_type, confidence);
                PamReturnCode::Success
            }
            auth::AuthResult::NoFaceDetected => {
                info!("Glance: No face detected for user '{}'", username);
                PamReturnCode::Auth_Err
            }
            auth::AuthResult::NoMatch => {
                info!("Glance: Face not recognized for user '{}'", username);
                PamReturnCode::Auth_Err
            }
            auth::AuthResult::Timeout => {
                info!("Glance: Authentication timeout for user '{}'", username);
                PamReturnCode::Auth_Err
            }
            auth::AuthResult::Error(e) => {
                error!("Glance: Authentication error for '{}': {}", username, e);
                PamReturnCode::Auth_Err
            }
        }
    }

    fn set_credentials(_handle: &PamHandle, _args: Vec<&CStr>, _flags: c_uint) -> PamReturnCode {
        PamReturnCode::Success
    }

    fn account_management(_handle: &PamHandle, _args: Vec<&CStr>, _flags: c_uint) -> PamReturnCode {
        PamReturnCode::Success
    }
}

#[derive(Debug, Clone)]
pub struct PamConfig {
    pub timeout: f64,
    pub prefer_ir: bool,
    pub data_dir: String,
    pub config_file: String,
    pub debug: bool,
}

impl Default for PamConfig {
    fn default() -> Self {
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/root".to_string());
        
        Self {
            timeout: 5.0,
            prefer_ir: true,
            data_dir: "/var/lib/glance".to_string(),
            config_file: format!("{}/.config/glance/config.json", home),
            debug: false,
        }
    }
}

fn parse_args(args: &[&CStr]) -> anyhow::Result<PamConfig> {
    let mut config = PamConfig::default();
    
    for arg in args {
        let arg_str = arg.to_str()?;
        
        if let Some(value) = arg_str.strip_prefix("timeout=") {
            config.timeout = value.parse()?;
        } else if let Some(value) = arg_str.strip_prefix("data_dir=") {
            config.data_dir = value.to_string();
        } else if let Some(value) = arg_str.strip_prefix("config=") {
            config.config_file = value.to_string();
        } else if arg_str == "prefer_ir" {
            config.prefer_ir = true;
        } else if arg_str == "prefer_rgb" {
            config.prefer_ir = false;
        } else if arg_str == "debug" {
            config.debug = true;
        }
    }
    
    Ok(config)
}

fn init_logging() {
    use syslog::{Facility, Formatter3164, BasicLogger};
    use log::LevelFilter;
    
    let formatter = Formatter3164 {
        facility: Facility::LOG_AUTH,
        hostname: None,
        process: "pam_glance".into(),
        pid: std::process::id(),
    };
    
    if let Ok(logger) = syslog::unix(formatter) {
        let _ = log::set_boxed_logger(Box::new(BasicLogger::new(logger)))
            .map(|()| log::set_max_level(LevelFilter::Info));
    }
}
