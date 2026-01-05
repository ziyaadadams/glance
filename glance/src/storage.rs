use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn get_storage_dir() -> PathBuf {
    if let Some(data_dir) = dirs::data_dir() {
        let user_dir = data_dir.join("glance");
        eprintln!("[Storage] Using XDG data directory: {:?}", user_dir);
        return user_dir;
    }
    
    if let Some(home) = dirs::home_dir() {
        let home_dir = home.join(".glance");
        eprintln!("[Storage] Using home directory: {:?}", home_dir);
        return home_dir;
    }
    
    eprintln!("[Storage] Using current directory fallback");
    PathBuf::from("./data")
}

/// Get legacy storage directories from the old "facerec" naming
fn get_legacy_storage_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    
    // Old XDG location
    if let Some(data_dir) = dirs::data_dir() {
        dirs.push(data_dir.join("facerec"));
    }
    
    // Old home directory locations
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".facerec"));
        dirs.push(home.join(".local").join("share").join("facerec"));
    }
    
    // Old system locations
    dirs.push(PathBuf::from("/var/lib/facerec"));
    
    dirs
}

fn can_write_to_system() -> bool {
    let system_dir = PathBuf::from("/var/lib/glance");
    if !system_dir.exists() {
        return false;
    }
    let test_file = system_dir.join(".write_test");
    match fs::write(&test_file, "test") {
        Ok(_) => {
            let _ = fs::remove_file(&test_file);
            true
        }
        Err(_) => false,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceData {
    pub username: String,
    pub encodings: Vec<FaceEncoding>,
    #[serde(default)]
    pub ir_encodings: Vec<FaceEncoding>,  // Separate IR camera encodings
    #[serde(default)]
    pub rgb_encodings: Vec<FaceEncoding>, // Separate RGB camera encodings
    pub ir_captured: bool,
    #[serde(default)]
    pub rgb_captured: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceEncoding {
    pub encoding: Vec<f64>,
    pub pose: String,
    #[serde(default)]
    pub camera_type: String,  // "ir" or "rgb"
}

impl FaceData {
    pub fn new(username: &str) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            username: username.to_string(),
            encodings: Vec::new(),
            ir_encodings: Vec::new(),
            rgb_encodings: Vec::new(),
            ir_captured: false,
            rgb_captured: false,
            created_at: now.clone(),
            updated_at: now,
        }
    }
    
    pub fn add_encoding(&mut self, encoding: Vec<f64>, pose: &str) {
        self.encodings.push(FaceEncoding {
            encoding,
            pose: pose.to_string(),
            camera_type: String::new(),
        });
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
    
    pub fn add_ir_encoding(&mut self, encoding: Vec<f64>, pose: &str) {
        self.ir_encodings.push(FaceEncoding {
            encoding,
            pose: pose.to_string(),
            camera_type: "ir".to_string(),
        });
        self.ir_captured = true;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
    
    pub fn add_rgb_encoding(&mut self, encoding: Vec<f64>, pose: &str) {
        self.rgb_encodings.push(FaceEncoding {
            encoding,
            pose: pose.to_string(),
            camera_type: "rgb".to_string(),
        });
        self.rgb_captured = true;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
    
    /// Get all encodings (legacy + ir + rgb) for matching
    pub fn all_encodings(&self) -> Vec<&FaceEncoding> {
        let mut all: Vec<&FaceEncoding> = self.encodings.iter().collect();
        all.extend(self.ir_encodings.iter());
        all.extend(self.rgb_encodings.iter());
        all
    }
}

pub fn get_storage_path(username: &str) -> PathBuf {
    get_storage_dir().join(format!("{}.json", username))
}

pub fn load_face_data(username: &str) -> Option<FaceData> {
    // Check current glance location first
    let path = get_storage_path(username);
    if path.exists() {
        eprintln!("[Storage] Found face data at {:?}", path);
        let content = fs::read_to_string(&path).ok()?;
        return serde_json::from_str(&content).ok();
    }
    
    // Check system glance location
    let system_path = PathBuf::from("/var/lib/glance").join(format!("{}.json", username));
    if system_path.exists() {
        eprintln!("[Storage] Found face data at system location {:?}", system_path);
        let content = fs::read_to_string(&system_path).ok()?;
        return serde_json::from_str(&content).ok();
    }
    
    // Check legacy facerec locations and migrate if found
    for legacy_dir in get_legacy_storage_dirs() {
        let legacy_path = legacy_dir.join(format!("{}.json", username));
        if legacy_path.exists() {
            eprintln!("[Storage] Found legacy face data at {:?}, migrating...", legacy_path);
            if let Ok(content) = fs::read_to_string(&legacy_path) {
                if let Ok(data) = serde_json::from_str::<FaceData>(&content) {
                    // Migrate to new location
                    if save_face_data(&data).is_ok() {
                        eprintln!("[Storage] Successfully migrated face data to new location");
                        // Optionally remove old file
                        let _ = fs::remove_file(&legacy_path);
                    }
                    return Some(data);
                }
            }
        }
    }
    
    eprintln!("[Storage] No face data found for user: {}", username);
    None
}

pub fn save_face_data(data: &FaceData) -> Result<(), String> {
    let storage_dir = get_storage_dir();
    
    eprintln!("[Storage] Saving face data for user: {}", data.username);
    eprintln!("[Storage] Storage directory: {:?}", storage_dir);
    eprintln!("[Storage] Number of encodings: {}", data.encodings.len());
    
    fs::create_dir_all(&storage_dir)
        .map_err(|e| format!("Failed to create storage directory {:?}: {}", storage_dir, e))?;
    
    let path = storage_dir.join(format!("{}.json", data.username));
    eprintln!("[Storage] Writing to: {:?}", path);
    
    let content = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Failed to serialize face data: {}", e))?;
    
    fs::write(&path, &content)
        .map_err(|e| format!("Failed to write face data to {:?}: {}", path, e))?;
    
    eprintln!("[Storage] Saved successfully to {:?}", path);
    
    if can_write_to_system() {
        let system_path = PathBuf::from("/var/lib/glance").join(format!("{}.json", data.username));
        match fs::write(&system_path, &content) {
            Ok(_) => eprintln!("[Storage] Also saved to system location: {:?}", system_path),
            Err(e) => eprintln!("[Storage] Could not save to system location: {}", e),
        }
    }
    
    Ok(())
}

pub fn delete_face_data(username: &str) -> Result<(), String> {
    let path = get_storage_path(username);
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete face data: {}", e))?;
    }
    Ok(())
}
