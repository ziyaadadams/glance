use dlib_face_recognition::{
    FaceDetector, FaceDetectorTrait,
    LandmarkPredictor, LandmarkPredictorTrait,
    FaceEncoderNetwork, FaceEncoderTrait,
    FaceEncoding, ImageMatrix,
};
use opencv::prelude::*;
use opencv::core::Mat;
use anyhow::Result;
use log::{debug, warn};
use std::path::Path;

pub struct FaceRecognizer {
    detector: FaceDetector,
    predictor: LandmarkPredictor,
    encoder: FaceEncoderNetwork,
    tolerance: f64,
}

#[derive(Clone)]
pub struct DetectedFace {
    pub rect: (i64, i64, i64, i64),
    pub encoding: FaceEncoding,
}

impl FaceRecognizer {
    pub fn new(models_dir: &Path, tolerance: f64) -> Result<Self> {
        let shape_predictor_path = models_dir.join("shape_predictor_68_face_landmarks.dat");
        let face_rec_path = models_dir.join("dlib_face_recognition_resnet_model_v1.dat");
        
        let detector = FaceDetector::new();
        
        let predictor = if shape_predictor_path.exists() {
            LandmarkPredictor::open(shape_predictor_path).map_err(|e| anyhow::anyhow!(e))?
        } else {
            warn!("Shape predictor model not found at {:?}", shape_predictor_path);
            anyhow::bail!("Shape predictor model not found");
        };
        
        let encoder = if face_rec_path.exists() {
            FaceEncoderNetwork::open(face_rec_path).map_err(|e| anyhow::anyhow!(e))?
        } else {
            warn!("Face recognition model not found at {:?}", face_rec_path);
            anyhow::bail!("Face recognition model not found");
        };
        
        Ok(Self {
            detector,
            predictor,
            encoder,
            tolerance,
        })
    }
    
    pub fn with_defaults(tolerance: f64) -> Result<Self> {
        let models_dir = Path::new("/usr/share/glance/models");
        Self::new(models_dir, tolerance)
    }
    
    pub fn detect_faces(&self, frame: &Mat) -> Result<Vec<DetectedFace>> {
        let image = opencv_to_dlib(frame)?;
        
        let face_rects = self.detector.face_locations(&image);
        
        if face_rects.is_empty() {
            return Ok(Vec::new());
        }
        
        debug!("Detected {} face(s)", face_rects.len());
        
        let mut faces = Vec::new();
        
        for rect in face_rects.iter() {
            let landmarks = self.predictor.face_landmarks(&image, &rect);
            
            let encodings = self.encoder.get_face_encodings(
                &image, 
                &[landmarks], 
                0,
            );
            
            if !encodings.is_empty() {
                faces.push(DetectedFace {
                    rect: (rect.left, rect.top, rect.right, rect.bottom),
                    encoding: encodings[0].clone(),
                });
            }
        }
        
        Ok(faces)
    }
    
    pub fn compare_face(&self, detected: &FaceEncoding, stored: &[Vec<f64>]) -> Option<f64> {
        if stored.is_empty() {
            return None;
        }
        
        let mut min_distance = f64::MAX;
        
        for stored_vec in stored {
            if let Ok(stored_enc) = FaceEncoding::from_vec(stored_vec) {
                let distance = detected.distance(&stored_enc);
                if distance < min_distance {
                    min_distance = distance;
                }
            }
        }
        
        debug!("Best match distance: {:.4} (tolerance: {:.4})", min_distance, self.tolerance);
        
        if min_distance <= self.tolerance {
            Some(min_distance)
        } else {
            None
        }
    }
    
    pub fn match_face(&self, detected: &FaceEncoding, users_faces: &[(String, Vec<Vec<f64>>)]) -> Option<(String, f64)> {
        let mut best_match: Option<(String, f64)> = None;
        
        for (username, face_encodings) in users_faces {
            if let Some(distance) = self.compare_face(detected, face_encodings) {
                if best_match.is_none() || distance < best_match.as_ref().unwrap().1 {
                    best_match = Some((username.clone(), distance));
                }
            }
        }
        
        best_match
    }
}

fn opencv_to_dlib(mat: &Mat) -> Result<ImageMatrix> {
    use opencv::imgproc;
    
    let rgb = if mat.channels() == 3 {
        let mut rgb = Mat::default();
        imgproc::cvt_color(mat, &mut rgb, imgproc::COLOR_BGR2RGB, 0)?;
        rgb
    } else if mat.channels() == 1 {
        let mut rgb = Mat::default();
        imgproc::cvt_color(mat, &mut rgb, imgproc::COLOR_GRAY2RGB, 0)?;
        rgb
    } else {
        mat.clone()
    };
    
    let rows = rgb.rows() as usize;
    let cols = rgb.cols() as usize;
    
    let data = rgb.data_bytes()?;
    
    let image = unsafe { ImageMatrix::new(cols, rows, data.as_ptr()) };
    
    Ok(image)
}

pub fn load_user_faces(data_dir: &Path, username: &str) -> Result<Vec<Vec<f64>>> {
    let paths_to_try = [
        data_dir.join(format!("{}_face.json", username)),
        data_dir.join(format!("{}.json", username)),
    ];
    
    let mut face_data_path = None;
    for path in &paths_to_try {
        if path.exists() {
            face_data_path = Some(path.clone());
            break;
        }
    }
    
    if face_data_path.is_none() {
        let config_path = data_dir.join("config.json");
        if config_path.exists() {
            let config: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&config_path)?
            )?;
            
            if let Some(faces) = config.get("registered_faces") {
                if let Some(user_data) = faces.get(username) {
                    if let Some(encodings) = user_data.get("encodings") {
                        if let Some(arr) = encodings.as_array() {
                            let mut result = Vec::new();
                            for enc in arr {
                                if let Some(enc_arr) = enc.as_array() {
                                    let encoding: Vec<f64> = enc_arr.iter()
                                        .filter_map(|v| v.as_f64())
                                        .collect();
                                    if !encoding.is_empty() {
                                        result.push(encoding);
                                    }
                                }
                            }
                            return Ok(result);
                        }
                    }
                }
            }
        }
        return Ok(Vec::new());
    }
    
    let face_file = face_data_path.unwrap();
    let content = std::fs::read_to_string(&face_file)?;
    let data: serde_json::Value = serde_json::from_str(&content)?;
    
    let mut encodings = Vec::new();
    
    if let Some(arr) = data.get("encodings").and_then(|e| e.as_array()) {
        for enc in arr {
            if let Some(nested_enc) = enc.get("encoding").and_then(|e| e.as_array()) {
                let encoding: Vec<f64> = nested_enc.iter()
                    .filter_map(|v| v.as_f64())
                    .collect();
                if !encoding.is_empty() {
                    encodings.push(encoding);
                }
            }
            else if let Some(enc_arr) = enc.as_array() {
                let encoding: Vec<f64> = enc_arr.iter()
                    .filter_map(|v| v.as_f64())
                    .collect();
                if !encoding.is_empty() {
                    encodings.push(encoding);
                }
            }
        }
    }
    
    Ok(encodings)
}

pub fn load_all_faces(data_dir: &Path) -> Result<Vec<(String, Vec<Vec<f64>>)>> {
    let mut all_faces = Vec::new();
    
    let config_path = data_dir.join("config.json");
    if config_path.exists() {
        let config: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&config_path)?
        )?;
        
        if let Some(faces) = config.get("registered_faces") {
            if let Some(obj) = faces.as_object() {
                for (username, user_data) in obj {
                    if let Some(encodings) = user_data.get("encodings") {
                        if let Some(arr) = encodings.as_array() {
                            let mut user_encodings = Vec::new();
                            for enc in arr {
                                if let Some(enc_arr) = enc.as_array() {
                                    let encoding: Vec<f64> = enc_arr.iter()
                                        .filter_map(|v| v.as_f64())
                                        .collect();
                                    if !encoding.is_empty() {
                                        user_encodings.push(encoding);
                                    }
                                }
                            }
                            if !user_encodings.is_empty() {
                                all_faces.push((username.clone(), user_encodings));
                            }
                        }
                    }
                }
            }
        }
    }
    
    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            
            let username = if name.ends_with("_face.json") {
                name.strip_suffix("_face.json").unwrap().to_string()
            } else if name.ends_with(".json") && name != "config.json" {
                name.strip_suffix(".json").unwrap().to_string()
            } else {
                continue;
            };
            
            if all_faces.iter().any(|(u, _)| u == &username) {
                continue;
            }
            
            if let Ok(encodings) = load_user_faces(data_dir, &username) {
                if !encodings.is_empty() {
                    all_faces.push((username, encodings));
                }
            }
        }
    }
    
    Ok(all_faces)
}
