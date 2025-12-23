use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlanceConfig {
    pub camera: CameraConfig,
    pub recognition: RecognitionConfig,
    pub ir_emitter: IrEmitterConfig,
    #[serde(default)]
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    #[serde(default = "default_prefer_ir")]
    pub prefer_ir: bool,
    #[serde(default = "default_ir_device")]
    pub ir_device: String,
    #[serde(default = "default_rgb_device")]
    pub rgb_device: String,
    #[serde(default = "default_min_brightness")]
    pub min_brightness: f64,
    #[serde(default = "default_frame_width")]
    pub frame_width: u32,
    #[serde(default = "default_frame_height")]
    pub frame_height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognitionConfig {
    #[serde(default = "default_ir_tolerance")]
    pub ir_tolerance: f64,
    #[serde(default = "default_rgb_tolerance")]
    pub rgb_tolerance: f64,
    #[serde(default = "default_auth_timeout")]
    pub auth_timeout: f64,
    #[serde(default = "default_max_auth_frames")]
    pub max_auth_frames: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrEmitterConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub binary_path: String,
    #[serde(default = "default_ir_config_path")]
    pub config_path: String,
    #[serde(default = "default_ir_device")]
    pub device: String,
}

fn default_prefer_ir() -> bool { true }
fn default_ir_device() -> String { "/dev/video2".to_string() }
fn default_rgb_device() -> String { "/dev/video0".to_string() }
fn default_min_brightness() -> f64 { 70.0 }
fn default_frame_width() -> u32 { 640 }
fn default_frame_height() -> u32 { 480 }
fn default_ir_tolerance() -> f64 { 0.45 }
fn default_rgb_tolerance() -> f64 { 0.50 }
fn default_auth_timeout() -> f64 { 5.0 }
fn default_max_auth_frames() -> u32 { 30 }
fn default_true() -> bool { true }
fn default_ir_config_path() -> String {
    dirs::home_dir()
        .map(|p| p.join(".config/linux-enable-ir-emitter.toml").to_string_lossy().to_string())
        .unwrap_or_default()
}

impl Default for GlanceConfig {
    fn default() -> Self {
        Self {
            camera: CameraConfig::default(),
            recognition: RecognitionConfig::default(),
            ir_emitter: IrEmitterConfig::default(),
            version: 1,
        }
    }
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            prefer_ir: default_prefer_ir(),
            ir_device: default_ir_device(),
            rgb_device: default_rgb_device(),
            min_brightness: default_min_brightness(),
            frame_width: default_frame_width(),
            frame_height: default_frame_height(),
        }
    }
}

impl Default for RecognitionConfig {
    fn default() -> Self {
        Self {
            ir_tolerance: default_ir_tolerance(),
            rgb_tolerance: default_rgb_tolerance(),
            auth_timeout: default_auth_timeout(),
            max_auth_frames: default_max_auth_frames(),
        }
    }
}

impl Default for IrEmitterConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            binary_path: String::new(),
            config_path: default_ir_config_path(),
            device: default_ir_device(),
        }
    }
}

impl GlanceConfig {
    pub fn load(path: &str) -> Result<Self> {
        if Path::new(path).exists() {
            let content = fs::read_to_string(path)?;
            let config: GlanceConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }
    
    pub fn get_tolerance(&self, is_ir: bool) -> f64 {
        if is_ir {
            self.recognition.ir_tolerance
        } else {
            self.recognition.rgb_tolerance
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceEncodingNested {
    pub encoding: Vec<f64>,
    pub pose: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum EncodingsFormat {
    Nested(Vec<FaceEncodingNested>),
    Flat(Vec<Vec<f64>>),
}

#[derive(Debug, Clone, Deserialize)]
struct FaceDataRaw {
    pub username: String,
    pub encodings: EncodingsFormat,
    #[serde(default)]
    pub pose_labels: Vec<String>,
    #[serde(default)]
    pub ir_captured: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FaceData {
    pub username: String,
    pub encodings: Vec<Vec<f64>>,
    pub pose_labels: Vec<String>,
    #[serde(default)]
    pub ir_captured: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

impl FaceData {
    pub fn from_json(content: &str) -> Result<Self> {
        let raw: FaceDataRaw = serde_json::from_str(content)?;
        
        let (encodings, pose_labels) = match raw.encodings {
            EncodingsFormat::Nested(nested) => {
                let encs: Vec<Vec<f64>> = nested.iter().map(|n| n.encoding.clone()).collect();
                let labels: Vec<String> = nested.iter().map(|n| n.pose.clone()).collect();
                (encs, labels)
            }
            EncodingsFormat::Flat(flat) => {
                let labels = if raw.pose_labels.len() >= flat.len() {
                    raw.pose_labels
                } else {
                    vec!["center".to_string(); flat.len()]
                };
                (flat, labels)
            }
        };
        
        Ok(FaceData {
            username: raw.username,
            encodings,
            pose_labels,
            ir_captured: raw.ir_captured,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureFaceData {
    pub username: String,
    pub encodings: Vec<String>,
    pub pose_labels: Vec<String>,
    #[serde(default)]
    pub ir_captured: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureDatabase {
    pub version: u32,
    pub faces: std::collections::HashMap<String, SecureFaceData>,
}

fn get_machine_key() -> String {
    std::fs::read_to_string("/etc/machine-id")
        .unwrap_or_else(|_| "glance-default-key-12345".to_string())
        .trim()
        .to_string()
}

fn deobfuscate_encoding(obfuscated: &str, key: &str) -> Result<Vec<f64>> {
    use sha2::{Sha256, Digest};
    use base64::{Engine as _, engine::general_purpose};
    
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let key_hash = hasher.finalize();
    
    let data = general_purpose::STANDARD.decode(obfuscated)?;
    
    let deobfuscated: Vec<u8> = data
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key_hash[i % key_hash.len()])
        .collect();
    
    let num_floats = deobfuscated.len() / 8;
    let mut encodings = Vec::with_capacity(num_floats);
    
    for i in 0..num_floats {
        let start = i * 8;
        let bytes: [u8; 8] = deobfuscated[start..start + 8]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid encoding length"))?;
        encodings.push(f64::from_le_bytes(bytes));
    }
    
    Ok(encodings)
}

impl FaceData {
    pub fn load(data_dir: &str, username: &str) -> Result<Option<Self>> {
        let secure_path = "/var/lib/glance/faces.json";
        if Path::new(secure_path).exists() {
            if let Ok(content) = fs::read_to_string(secure_path) {
                if let Ok(db) = serde_json::from_str::<SecureDatabase>(&content) {
                    if let Some(secure_data) = db.faces.get(username) {
                        let key = get_machine_key();
                        let mut encodings = Vec::new();
                        
                        for enc in &secure_data.encodings {
                            match deobfuscate_encoding(enc, &key) {
                                Ok(decoded) => encodings.push(decoded),
                                Err(e) => {
                                    log::error!("Failed to deobfuscate encoding: {}", e);
                                    continue;
                                }
                            }
                        }
                        
                        if !encodings.is_empty() {
                            return Ok(Some(FaceData {
                                username: secure_data.username.clone(),
                                encodings,
                                pose_labels: secure_data.pose_labels.clone(),
                                ir_captured: secure_data.ir_captured,
                                created_at: secure_data.created_at.clone(),
                                updated_at: secure_data.updated_at.clone(),
                            }));
                        }
                    }
                }
            }
        }
        
        let json_path = format!("{}/{}.json", data_dir, username);
        if Path::new(&json_path).exists() {
            let content = fs::read_to_string(&json_path)?;
            let data = FaceData::from_json(&content)?;
            return Ok(Some(data));
        }
        
        let pkl_path = format!("{}/{}.pkl", data_dir, username);
        if Path::new(&pkl_path).exists() {
            log::warn!("Legacy .pkl file found for {}. Please run the GUI to migrate.", username);
            return Ok(None);
        }
        
        Ok(None)
    }
    
    pub fn get_encodings(&self) -> &[Vec<f64>] {
        &self.encodings
    }
}
