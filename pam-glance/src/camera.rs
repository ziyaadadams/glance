use opencv::{
    prelude::*,
    videoio::{self, VideoCapture, VideoCaptureAPIs},
    core::{Mat, Vector},
};
use anyhow::{Result, Context};
use log::{info, debug, warn};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraType {
    Infrared,
    Rgb,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct CameraInfo {
    pub device_id: i32,
    pub device_path: String,
    pub name: String,
    pub camera_type: CameraType,
}

pub struct SmartCamera {
    capture: VideoCapture,
    pub camera_info: CameraInfo,
    pub is_ir: bool,
}

impl SmartCamera {
    pub fn open(prefer_ir: bool, ir_device: &str, rgb_device: &str) -> Result<Self> {
        let cameras = detect_cameras()?;
        
        if cameras.is_empty() {
            anyhow::bail!("No cameras detected");
        }
        
        // Build prioritized list of cameras to try
        let mut cameras_to_try: Vec<&CameraInfo> = if prefer_ir {
            // Try IR first, then RGB, then Unknown
            let mut list: Vec<&CameraInfo> = cameras.iter()
                .filter(|c| c.camera_type == CameraType::Infrared)
                .collect();
            list.extend(cameras.iter().filter(|c| c.camera_type == CameraType::Rgb));
            list.extend(cameras.iter().filter(|c| c.camera_type == CameraType::Unknown));
            list
        } else {
            // Try RGB first, then IR, then Unknown
            let mut list: Vec<&CameraInfo> = cameras.iter()
                .filter(|c| c.camera_type == CameraType::Rgb)
                .collect();
            list.extend(cameras.iter().filter(|c| c.camera_type == CameraType::Infrared));
            list.extend(cameras.iter().filter(|c| c.camera_type == CameraType::Unknown));
            list
        };
        
        // Try each camera until one works
        let mut last_error = String::new();
        for camera_info in cameras_to_try {
            info!("Trying camera: {} ({})", camera_info.name, 
                  match camera_info.camera_type {
                      CameraType::Infrared => "IR",
                      CameraType::Rgb => "RGB",
                      CameraType::Unknown => "Unknown",
                  });
            
            match VideoCapture::new(camera_info.device_id, videoio::CAP_V4L2) {
                Ok(mut capture) => {
                    if capture.is_opened().unwrap_or(false) {
                        // Try to read a test frame
                        let mut test_frame = Mat::default();
                        if capture.read(&mut test_frame).is_ok() && !test_frame.empty() {
                            capture.set(videoio::CAP_PROP_FRAME_WIDTH, 640.0)?;
                            capture.set(videoio::CAP_PROP_FRAME_HEIGHT, 480.0)?;
                            
                            let is_ir = camera_info.camera_type == CameraType::Infrared;
                            info!("Successfully opened camera video{}", camera_info.device_id);
                            
                            return Ok(Self {
                                capture,
                                camera_info: camera_info.clone(),
                                is_ir,
                            });
                        } else {
                            warn!("Camera video{} opened but couldn't read frames", camera_info.device_id);
                            last_error = format!("Camera {} cannot read frames", camera_info.device_id);
                        }
                    } else {
                        warn!("Failed to open camera video{}", camera_info.device_id);
                        last_error = format!("Camera {} failed to open", camera_info.device_id);
                    }
                    let _ = capture.release();
                }
                Err(e) => {
                    warn!("Error opening camera video{}: {}", camera_info.device_id, e);
                    last_error = format!("Camera {} error: {}", camera_info.device_id, e);
                }
            }
        }
        
        anyhow::bail!("No working camera found. Last error: {}", last_error)
    }
    
    pub fn read(&mut self) -> Result<Mat> {
        let mut frame = Mat::default();
        self.capture.read(&mut frame)?;
        
        if frame.empty() {
            anyhow::bail!("Empty frame captured");
        }
        
        Ok(frame)
    }
    
    pub fn check_brightness(&mut self, min_brightness: f64) -> Result<bool> {
        let mut total_brightness = 0.0;
        let mut count = 0;
        
        for _ in 0..5 {
            if let Ok(frame) = self.read() {
                if let Ok(brightness) = calculate_brightness(&frame) {
                    total_brightness += brightness;
                    count += 1;
                }
            }
        }
        
        if count == 0 {
            return Ok(false);
        }
        
        let avg_brightness = total_brightness / count as f64;
        debug!("Camera brightness: {:.1}", avg_brightness);
        
        Ok(avg_brightness >= min_brightness)
    }
}

impl Drop for SmartCamera {
    fn drop(&mut self) {
        let _ = self.capture.release();
    }
}

fn detect_cameras() -> Result<Vec<CameraInfo>> {
    let mut cameras = Vec::new();
    
    let video_dir = Path::new("/sys/class/video4linux");
    
    if video_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(video_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                
                if !name.starts_with("video") {
                    continue;
                }
                
                let device_id: i32 = name.strip_prefix("video")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(-1);
                
                if device_id < 0 {
                    continue;
                }
                
                let name_path = entry.path().join("name");
                let camera_name = std::fs::read_to_string(name_path)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| format!("Camera {}", device_id));
                
                let camera_type = detect_camera_type(&camera_name);
                
                if is_capture_device(device_id) {
                    cameras.push(CameraInfo {
                        device_id,
                        device_path: format!("/dev/video{}", device_id),
                        name: camera_name,
                        camera_type,
                    });
                }
            }
        }
    }
    
    cameras.sort_by(|a, b| {
        match (&a.camera_type, &b.camera_type) {
            (CameraType::Infrared, CameraType::Infrared) => a.device_id.cmp(&b.device_id),
            (CameraType::Infrared, _) => std::cmp::Ordering::Less,
            (_, CameraType::Infrared) => std::cmp::Ordering::Greater,
            _ => a.device_id.cmp(&b.device_id),
        }
    });
    
    info!("Detected {} cameras", cameras.len());
    for cam in &cameras {
        debug!("  video{}: {} ({:?})", cam.device_id, cam.name, cam.camera_type);
    }
    
    Ok(cameras)
}

fn detect_camera_type(name: &str) -> CameraType {
    let name_lower = name.to_lowercase();
    
    if name_lower.ends_with(" i") || name_lower.ends_with(": i") {
        return CameraType::Infrared;
    }
    
    let ir_keywords = ["ir", "infrared", "depth", "tof"];
    for keyword in &ir_keywords {
        if name_lower.contains(keyword) {
            return CameraType::Infrared;
        }
    }
    
    if name_lower.ends_with(" c") || name_lower.ends_with(": c") {
        return CameraType::Rgb;
    }
    
    let rgb_keywords = ["rgb", "color", "webcam", "hd camera", "usb camera"];
    for keyword in &rgb_keywords {
        if name_lower.contains(keyword) {
            return CameraType::Rgb;
        }
    }
    
    if name_lower.contains("integrated") {
        return CameraType::Unknown;
    }
    
    CameraType::Unknown
}

fn is_capture_device(device_id: i32) -> bool {
    // Check V4L2 capabilities to see if this is a real capture device
    let device_path = format!("/dev/video{}", device_id);
    
    // Read device capabilities from sysfs
    let caps_path = format!("/sys/class/video4linux/video{}/device/video4linux/video{}/dev", device_id, device_id);
    
    // Try to check if device supports video capture via ioctl info
    // Metadata devices typically have index 1 or 3 on integrated cameras
    let index_path = format!("/sys/class/video4linux/video{}/index", device_id);
    if let Ok(index_str) = std::fs::read_to_string(&index_path) {
        if let Ok(index) = index_str.trim().parse::<i32>() {
            // Index 0 is typically the main capture device, index 1 is metadata
            if index != 0 {
                debug!("Skipping video{} (index {}), likely metadata device", device_id, index);
                return false;
            }
        }
    }
    
    // Verify we can actually open and read frames
    if let Ok(mut cap) = VideoCapture::new(device_id, videoio::CAP_V4L2) {
        if cap.is_opened().unwrap_or(false) {
            // Try to read a frame to verify it's a real capture device
            let mut frame = Mat::default();
            if cap.read(&mut frame).is_ok() && !frame.empty() {
                let _ = cap.release();
                return true;
            }
        }
        let _ = cap.release();
    }
    false
}

fn calculate_brightness(frame: &Mat) -> Result<f64> {
    use opencv::imgproc;
    
    let gray = if frame.channels() > 1 {
        let mut gray = Mat::default();
        imgproc::cvt_color(frame, &mut gray, imgproc::COLOR_BGR2GRAY, 0)?;
        gray
    } else {
        frame.clone()
    };
    
    let mean = opencv::core::mean(&gray, &Mat::default())?;
    
    Ok(mean[0])
}

pub fn verify_camera_type(device_id: i32) -> Result<CameraType> {
    let mut cap = VideoCapture::new(device_id, videoio::CAP_V4L2)?;
    
    if !cap.is_opened()? {
        return Ok(CameraType::Unknown);
    }
    
    let mut color_scores = 0;
    let mut gray_scores = 0;
    
    for _ in 0..5 {
        let mut frame = Mat::default();
        if cap.read(&mut frame).is_ok() && !frame.empty() {
            if is_grayscale_frame(&frame)? {
                gray_scores += 1;
            } else {
                color_scores += 1;
            }
        }
    }
    
    let _ = cap.release();
    
    if gray_scores > color_scores {
        Ok(CameraType::Infrared)
    } else if color_scores > 0 {
        Ok(CameraType::Rgb)
    } else {
        Ok(CameraType::Unknown)
    }
}

fn is_grayscale_frame(frame: &Mat) -> Result<bool> {
    if frame.channels() != 3 {
        return Ok(true);
    }
    
    let mut channels = Vector::<Mat>::new();
    opencv::core::split(frame, &mut channels)?;
    
    if channels.len() < 3 {
        return Ok(false);
    }
    
    let mut diff1 = Mat::default();
    let mut diff2 = Mat::default();
    
    opencv::core::absdiff(&channels.get(0)?, &channels.get(1)?, &mut diff1)?;
    opencv::core::absdiff(&channels.get(0)?, &channels.get(2)?, &mut diff2)?;
    
    let mean1 = opencv::core::mean(&diff1, &Mat::default())?[0];
    let mean2 = opencv::core::mean(&diff2, &Mat::default())?[0];
    
    Ok(mean1 < 10.0 && mean2 < 10.0)
}
