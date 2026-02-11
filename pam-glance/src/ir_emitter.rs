use anyhow::{Result, Context};
use log::{info, debug, warn};
use std::process::{Command, Child, Stdio};
use std::time::Duration;
use std::path::Path;

pub struct IrEmitter {
    device: String,
    enabled: bool,
    child_process: Option<Child>,
}

impl IrEmitter {
    pub fn new(device: &str) -> Self {
        Self {
            device: device.to_string(),
            enabled: false,
            child_process: None,
        }
    }
    
    pub fn is_installed() -> bool {
        Self::find_executable().is_some()
    }
    
    /// Find the linux-enable-ir-emitter executable in common locations
    fn find_executable() -> Option<String> {
        let paths = [
            "/usr/bin/linux-enable-ir-emitter",
            "/usr/local/bin/linux-enable-ir-emitter",
            "/home/ziyaadsmada/.local/bin/linux-enable-ir-emitter",  // User install location
            "/opt/linux-enable-ir-emitter/linux-enable-ir-emitter",
        ];
        
        for path in &paths {
            if Path::new(path).exists() {
                return Some(path.to_string());
            }
        }
        
        // Fallback to PATH lookup
        if let Ok(output) = Command::new("which").arg("linux-enable-ir-emitter").output() {
            if output.status.success() {
                if let Ok(path) = String::from_utf8(output.stdout) {
                    let path = path.trim();
                    if !path.is_empty() {
                        return Some(path.to_string());
                    }
                }
            }
        }
        
        None
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
        let executable = match Self::find_executable() {
            Some(path) => path,
            None => {
                debug!("linux-enable-ir-emitter not installed — skipping");
                return Ok(());
            }
        };
        
        info!("Enabling IR emitter for {} using {}", self.device, executable);
        
        // Start the IR emitter as a background process so we can kill it later
        let child = Command::new(&executable)
            .arg("--device")
            .arg(&self.device)
            .arg("run")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        
        match child {
            Ok(c) => {
                self.child_process = Some(c);
                self.enabled = true;
                // Give it a moment to initialize
                std::thread::sleep(Duration::from_millis(150));
                info!("IR emitter enabled for {}", self.device);
            }
            Err(e) => {
                warn!("Failed to start IR emitter: {}", e);
            }
        }
        
        Ok(())
    }
    
    /// Check if the emitter process was started and is still alive
    pub fn is_running(&self) -> bool {
        self.enabled && self.child_process.is_some()
    }
    
    pub fn disable(&mut self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        
        // Kill the child process if it exists
        if let Some(mut child) = self.child_process.take() {
            debug!("Killing IR emitter process");
            if let Err(e) = child.kill() {
                // Process may have already exited
                debug!("Could not kill IR emitter process: {}", e);
            }
            // Wait for it to fully terminate
            let _ = child.wait();
        }
        
        // Also try to kill any orphaned linux-enable-ir-emitter processes for this device
        let _ = Command::new("pkill")
            .arg("-f")
            .arg(format!("linux-enable-ir-emitter.*{}", self.device))
            .output();
        
        self.enabled = false;
        debug!("IR emitter disabled for {}", self.device);
        
        Ok(())
    }
    
    pub fn run_with_config(&mut self, config_file: Option<&str>) -> Result<()> {
        if !Self::is_installed() {
            warn!("linux-enable-ir-emitter is not installed");
            return Ok(());
        }
        
        let mut cmd = Command::new("linux-enable-ir-emitter");
        
        cmd.arg("--device").arg(&self.device);
        
        if let Some(config) = config_file {
            cmd.arg("--config").arg(config);
        }
        
        cmd.arg("run");
        
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
        if self.enabled {
            let _ = self.disable();
        }
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
