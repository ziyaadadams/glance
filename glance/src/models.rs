//! Model downloading and management for Glance
//! 
//! Downloads the dlib face recognition models on first run if not present.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use log::info;

/// Model file information
#[allow(dead_code)]
pub struct ModelInfo {
    pub name: &'static str,
    pub url: &'static str,
    pub compressed_name: &'static str,
    pub size_mb: u32,
}

pub const SHAPE_PREDICTOR: ModelInfo = ModelInfo {
    name: "shape_predictor_68_face_landmarks.dat",
    url: "http://dlib.net/files/shape_predictor_68_face_landmarks.dat.bz2",
    compressed_name: "shape_predictor_68_face_landmarks.dat.bz2",
    size_mb: 100,
};

pub const FACE_RECOGNITION: ModelInfo = ModelInfo {
    name: "dlib_face_recognition_resnet_model_v1.dat",
    url: "http://dlib.net/files/dlib_face_recognition_resnet_model_v1.dat.bz2",
    compressed_name: "dlib_face_recognition_resnet_model_v1.dat.bz2",
    size_mb: 22,
};

/// Get the models directory (user-writable)
pub fn get_models_dir() -> PathBuf {
    // Check Flatpak location first (when running as Flatpak)
    let flatpak_dir = PathBuf::from("/app/share/glance/models");
    if models_exist_in(&flatpak_dir) {
        return flatpak_dir;
    }
    
    // Check system location
    let system_dir = PathBuf::from("/usr/share/glance/models");
    if models_exist_in(&system_dir) {
        return system_dir;
    }
    
    // Check user data directory
    if let Some(data_dir) = dirs::data_dir() {
        let user_models = data_dir.join("glance").join("models");
        if models_exist_in(&user_models) {
            return user_models;
        }
        // Return user dir as default download location
        return user_models;
    }
    
    // Fallback to home directory
    if let Some(home) = dirs::home_dir() {
        return home.join(".local").join("share").join("glance").join("models");
    }
    
    // Last resort
    PathBuf::from("./models")
}

/// Check if models exist in a directory
pub fn models_exist_in(dir: &PathBuf) -> bool {
    dir.join(SHAPE_PREDICTOR.name).exists() && dir.join(FACE_RECOGNITION.name).exists()
}

/// Check if models are installed anywhere
pub fn models_installed() -> bool {
    models_exist_in(&get_models_dir())
}

/// Download progress callback type
pub type ProgressCallback = Box<dyn Fn(f64, &str) + Send>;

/// Download and install models
pub fn download_models(progress_callback: Option<ProgressCallback>) -> Result<(), String> {
    let models_dir = get_models_dir();
    
    info!("Downloading models to {:?}", models_dir);
    
    // Create directory
    fs::create_dir_all(&models_dir)
        .map_err(|e| format!("Failed to create models directory: {}", e))?;
    
    // Download shape predictor
    if !models_dir.join(SHAPE_PREDICTOR.name).exists() {
        download_and_extract_model(
            &SHAPE_PREDICTOR,
            &models_dir,
            progress_callback.as_ref().map(|cb| {
                move |p: f64| cb(p * 0.5, &format!("Downloading {} ({} MB)...", SHAPE_PREDICTOR.name, SHAPE_PREDICTOR.size_mb))
            }),
        )?;
    }
    
    // Download face recognition model
    if !models_dir.join(FACE_RECOGNITION.name).exists() {
        download_and_extract_model(
            &FACE_RECOGNITION,
            &models_dir,
            progress_callback.as_ref().map(|cb| {
                move |p: f64| cb(0.5 + p * 0.5, &format!("Downloading {} ({} MB)...", FACE_RECOGNITION.name, FACE_RECOGNITION.size_mb))
            }),
        )?;
    }
    
    if let Some(ref cb) = progress_callback {
        cb(1.0, "Models installed successfully!");
    }
    
    info!("Models downloaded successfully to {:?}", models_dir);
    Ok(())
}

/// Download and extract a single model
fn download_and_extract_model<F>(
    model: &ModelInfo,
    dest_dir: &PathBuf,
    progress: Option<F>,
) -> Result<(), String>
where
    F: Fn(f64),
{
    let final_path = dest_dir.join(model.name);
    
    info!("Downloading {} from {}", model.name, model.url);
    
    // Download the compressed file
    let response = ureq::get(model.url)
        .call()
        .map_err(|e| format!("Failed to download {}: {}", model.name, e))?;
    
    let content_length = response.header("content-length")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(model.size_mb as usize * 1024 * 1024);
    
    let mut reader = response.into_reader();
    let mut compressed_data = Vec::with_capacity(content_length);
    let mut buffer = [0u8; 8192];
    let mut downloaded = 0usize;
    
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                compressed_data.extend_from_slice(&buffer[..n]);
                downloaded += n;
                if let Some(ref p) = progress {
                    p(downloaded as f64 / content_length as f64);
                }
            }
            Err(e) => return Err(format!("Download error: {}", e)),
        }
    }
    
    info!("Downloaded {} bytes, decompressing...", compressed_data.len());
    
    // Decompress bz2
    let mut decompressor = bzip2::read::BzDecoder::new(&compressed_data[..]);
    let mut decompressed_data = Vec::new();
    decompressor.read_to_end(&mut decompressed_data)
        .map_err(|e| format!("Failed to decompress {}: {}", model.name, e))?;
    
    info!("Decompressed to {} bytes", decompressed_data.len());
    
    // Write to final location
    let mut file = File::create(&final_path)
        .map_err(|e| format!("Failed to create {}: {}", model.name, e))?;
    file.write_all(&decompressed_data)
        .map_err(|e| format!("Failed to write {}: {}", model.name, e))?;
    
    info!("Saved model to {:?}", final_path);
    
    Ok(())
}

/// Sync version of download for use in threads (may be used by CLI tools)
#[allow(dead_code)]
pub fn download_models_sync() -> Result<(), String> {
    download_models(None)
}
