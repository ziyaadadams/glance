use opencv::prelude::*;
use opencv::videoio::{self, VideoCapture, CAP_V4L2};
use std::fs;

#[derive(Debug, Clone)]
pub struct CameraInfo {
    pub device_id: i32,
    pub name: String,
    pub is_ir: bool,
}

#[derive(Debug, Clone)]
pub struct CameraFrame {
    pub rgb_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct Camera {
    cap: VideoCapture,
}

impl Camera {
    pub fn new(device_id: i32) -> Result<Self, String> {
        let mut cap = VideoCapture::new(device_id, CAP_V4L2)
            .map_err(|e| format!("Failed to open camera: {}", e))?;
        
        if !cap.is_opened().unwrap_or(false) {
            return Err("Camera not opened".to_string());
        }
        
        cap.set(videoio::CAP_PROP_FRAME_WIDTH, 640.0).ok();
        cap.set(videoio::CAP_PROP_FRAME_HEIGHT, 480.0).ok();
        cap.set(videoio::CAP_PROP_FPS, 30.0).ok();
        
        Ok(Self { cap })
    }
    
    pub fn read_frame(&mut self) -> Result<CameraFrame, String> {
        let mut mat = opencv::core::Mat::default();
        self.cap.read(&mut mat)
            .map_err(|e| format!("Failed to read frame: {}", e))?;
        
        if mat.empty() {
            return Err("Empty frame".to_string());
        }
        
        let mut rgb_mat = opencv::core::Mat::default();
        opencv::imgproc::cvt_color(&mat, &mut rgb_mat, opencv::imgproc::COLOR_BGR2RGB, 0)
            .map_err(|e| format!("Color conversion failed: {}", e))?;
        
        let width = rgb_mat.cols() as u32;
        let height = rgb_mat.rows() as u32;
        let data = rgb_mat.data_bytes()
            .map_err(|e| format!("Failed to get frame data: {}", e))?
            .to_vec();
        
        Ok(CameraFrame {
            rgb_data: data,
            width,
            height,
        })
    }
    
    pub fn detect_cameras() -> Option<CameraInfo> {
        let mut cameras = Vec::new();
        
        for device_id in (0..10).step_by(2) {
            if let Ok(cap) = VideoCapture::new(device_id, CAP_V4L2) {
                if cap.is_opened().unwrap_or(false) {
                    let name = Self::get_camera_name(device_id);
                    let is_ir = Self::is_ir_camera(&name);
                    eprintln!("Found camera {}: {} (IR: {})", device_id, name, is_ir);
                    cameras.push(CameraInfo {
                        device_id,
                        name,
                        is_ir,
                    });
                }
            }
        }
        
        if let Some(ir_cam) = cameras.iter().find(|c| c.is_ir) {
            eprintln!("Selected IR camera: {} (device {})", ir_cam.name, ir_cam.device_id);
            return Some(ir_cam.clone());
        }
        
        if let Some(cam) = cameras.first() {
            eprintln!("Selected camera: {} (device {})", cam.name, cam.device_id);
        }
        
        cameras.into_iter().next()
    }
    
    fn get_camera_name(device_id: i32) -> String {
        let path = format!("/sys/class/video4linux/video{}/name", device_id);
        fs::read_to_string(&path)
            .unwrap_or_else(|_| format!("video{}", device_id))
            .trim()
            .to_string()
    }
    
    fn is_ir_camera(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("infrared") ||
        lower.contains("ir camera") ||
        lower.contains("ir sensor") ||
        lower.contains("infra red") ||
        (lower.contains("integrated") && lower.ends_with(" i")) ||
        lower.contains(": integrated i")
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        let _ = self.cap.release();
    }
}
