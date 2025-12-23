use anyhow::{Result, Context};
use log::{info, debug, warn};
use std::process::Command;
use std::time::Duration;
use std::path::Path;

pub struct IrEmitter {
    device: String,
    enabled: bool,
}

impl IrEmitter {
    pub fn new(device: &str) -> Self {
        Self {
            device: device.to_string(),
            enabled: false,
        }
    }
    
    pub fn is_installed() -> bool {
        Command::new("which")
            .arg("linux-enable-ir-emitter")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    
    pub fn is_configured(device: &str) -> bool {
        let config_dir = Path::new("/etc/linux-enable-ir-emitter");
        if !config_dir.exists() {
            return false;
        }
        
        let device_name = Path::new(device)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("video0");
        
        let config_files = [
            config_dir.join(format!("{}.ini", device_name)),
            config_dir.join("default.ini"),
        ];
        
        for config in &config_files {
            if config.exists() {
                debug!("Found IR emitter config: {:?}", config);
                return true;
            }
        }
        
        if let Ok(entries) = std::fs::read_dir(config_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().map_or(false, |e| e == "ini") {
                    return true;
                }
            }
        }
        
        false
    }
    
    pub fn enable(&mut self) -> Result<()> {
        if !Self::is_installed() {
            warn!("linux-enable-ir-emitter is not installed");
            return Ok(());
        }
        
        debug!("Enabling IR emitter for {}", self.device);
        
        let output = Command::new("linux-enable-ir-emitter")
            .arg("run")
            .arg("--device")
            .arg(&self.device)
            .output()
            .context("Failed to run linux-enable-ir-emitter")?;
        
        if output.status.success() {
            self.enabled = true;
            info!("IR emitter enabled for {}", self.device);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to enable IR emitter: {}", stderr);
        }
        
        Ok(())
    }
    
    pub fn disable(&mut self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        
        self.enabled = false;
        debug!("IR emitter disabled");
        
        Ok(())
    }
    
    pub fn run_with_config(&mut self, config_file: Option<&str>) -> Result<()> {
        if !Self::is_installed() {
            warn!("linux-enable-ir-emitter is not installed");
            return Ok(());
        }
        
        let mut cmd = Command::new("linux-enable-ir-emitter");
        cmd.arg("run");
        
        if let Some(config) = config_file {
            cmd.arg("--config").arg(config);
        }
        
        cmd.arg("--device").arg(&self.device);
        
        let output = cmd.output()
            .context("Failed to run linux-enable-ir-emitter")?;
        
        if output.status.success() {
            self.enabled = true;
            info!("IR emitter enabled with custom config");
        }
        
        Ok(())
    }
}

impl Drop for IrEmitter {
    fn drop(&mut self) {
        let _ = self.disable();
    }
}

pub fn start_ir_emitter_background(device: &str) -> Result<std::process::Child> {
    let child = Command::new("linux-enable-ir-emitter")
        .arg("run")
        .arg("--device")
        .arg(device)
        .spawn()
        .context("Failed to start linux-enable-ir-emitter")?;
    
    std::thread::sleep(Duration::from_millis(100));
    
    Ok(child)
}

pub fn check_systemd_service() -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", "linux-enable-ir-emitter"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn detect_ir_device() -> Option<String> {
    let video_dir = Path::new("/sys/class/video4linux");
    
    if !video_dir.exists() {
        return None;
    }
    
    if let Ok(entries) = std::fs::read_dir(video_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            
            if !name.starts_with("video") {
                continue;
            }
            
            let name_path = entry.path().join("name");
            if let Ok(camera_name) = std::fs::read_to_string(name_path) {
                let lower_name = camera_name.to_lowercase();
                if lower_name.contains("ir") || lower_name.contains("infrared") {
                    return Some(format!("/dev/{}", name));
                }
            }
        }
    }
    
    if Path::new("/dev/video2").exists() {
        return Some("/dev/video2".to_string());
    }
    
    None
}
