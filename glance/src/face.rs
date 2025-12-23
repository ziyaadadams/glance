use dlib_face_recognition::{
    FaceDetector, FaceDetectorTrait,
    LandmarkPredictor, LandmarkPredictorTrait,
    FaceEncoderNetwork, FaceEncoderTrait,
    ImageMatrix,
};
use log::{warn, info};
use std::path::Path;

use crate::models;

const FACE_TOLERANCE: f64 = 0.45;

#[derive(Debug, Clone)]
pub struct FaceDetectionResult {
    pub face_found: bool,
    pub face_rect: Option<(i32, i32, i32, i32)>,
    pub encoding: Option<Vec<f64>>,
    pub landmarks: Option<Vec<(i32, i32)>>,
    pub confidence: f64,
}

pub struct FaceProcessor {
    detector: FaceDetector,
    predictor: Option<LandmarkPredictor>,
    encoder: Option<FaceEncoderNetwork>,
    tolerance: f64,
}

impl std::fmt::Debug for FaceProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaceProcessor")
            .field("has_predictor", &self.predictor.is_some())
            .field("has_encoder", &self.encoder.is_some())
            .field("tolerance", &self.tolerance)
            .finish()
    }
}

impl FaceProcessor {
    pub fn new() -> Result<Self, String> {
        Self::with_models_dir(&models::get_models_dir())
    }
    
    pub fn with_models_dir(models_dir: &Path) -> Result<Self, String> {
        info!("Initializing face processor with models from {:?}", models_dir);
        
        let detector = FaceDetector::new();
        
        let shape_predictor_path = models_dir.join("shape_predictor_68_face_landmarks.dat");
        let predictor = if shape_predictor_path.exists() {
            match LandmarkPredictor::open(&shape_predictor_path) {
                Ok(p) => {
                    info!("Loaded shape predictor from {:?}", shape_predictor_path);
                    Some(p)
                }
                Err(e) => {
                    warn!("Failed to load shape predictor: {}", e);
                    None
                }
            }
        } else {
            warn!("Shape predictor not found at {:?}", shape_predictor_path);
            None
        };
        
        let face_rec_path = models_dir.join("dlib_face_recognition_resnet_model_v1.dat");
        let encoder = if face_rec_path.exists() && predictor.is_some() {
            match FaceEncoderNetwork::open(&face_rec_path) {
                Ok(e) => {
                    info!("Loaded face encoder from {:?}", face_rec_path);
                    Some(e)
                }
                Err(e) => {
                    warn!("Failed to load face encoder: {}", e);
                    None
                }
            }
        } else {
            if !face_rec_path.exists() {
                warn!("Face recognition model not found at {:?}", face_rec_path);
            }
            None
        };
        
        Ok(Self {
            detector,
            predictor,
            encoder,
            tolerance: FACE_TOLERANCE,
        })
    }
    
    pub fn can_encode(&self) -> bool {
        self.predictor.is_some() && self.encoder.is_some()
    }
    
    pub fn detect_and_encode(&self, rgb_data: &[u8], width: u32, height: u32) -> FaceDetectionResult {
        let image = match self.rgb_to_image_matrix(rgb_data, width, height) {
            Some(img) => img,
            None => return FaceDetectionResult::empty(),
        };
        
        let face_rects = self.detector.face_locations(&image);
        
        if face_rects.is_empty() {
            return FaceDetectionResult::empty();
        }
        
        let rect = &face_rects[0];
        let face_rect = Some((
            rect.left as i32,
            rect.top as i32,
            (rect.right - rect.left) as i32,
            (rect.bottom - rect.top) as i32,
        ));
        
        let (landmarks, encoding) = if let (Some(ref predictor), Some(ref encoder)) = (&self.predictor, &self.encoder) {
            let lm = predictor.face_landmarks(&image, rect);
            
            let encodings = encoder.get_face_encodings(&image, &[lm.clone()], 0);
            
            let enc = if !encodings.is_empty() {
                let enc_slice = encodings[0].as_ref();
                let enc_vec: Vec<f64> = enc_slice.iter().map(|&x| x as f64).collect();
                Some(enc_vec)
            } else {
                None
            };
            
            let pts: Vec<(i32, i32)> = Vec::new();
            
            (Some(pts), enc)
        } else {
            (None, None)
        };
        
        FaceDetectionResult {
            face_found: true,
            face_rect,
            encoding,
            landmarks,
            confidence: 1.0,
        }
    }
    
    fn rgb_to_image_matrix(&self, rgb_data: &[u8], width: u32, height: u32) -> Option<ImageMatrix> {
        if rgb_data.len() != (width * height * 3) as usize {
            warn!("Invalid image data size: {} (expected {})", 
                  rgb_data.len(), width * height * 3);
            return None;
        }
        
        let image = unsafe {
            ImageMatrix::new(width as usize, height as usize, rgb_data.as_ptr())
        };
        Some(image)
    }
}

impl FaceDetectionResult {
    pub fn empty() -> Self {
        Self {
            face_found: false,
            face_rect: None,
            encoding: None,
            landmarks: None,
            confidence: 0.0,
        }
    }
    
    pub fn has_encoding(&self) -> bool {
        self.encoding.is_some()
    }
}

impl Default for FaceProcessor {
    fn default() -> Self {
        Self::new().expect("Failed to create face processor")
    }
}

#[derive(Debug)]
pub struct SharedFaceProcessor {
    inner: std::sync::Mutex<FaceProcessor>,
}

impl SharedFaceProcessor {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            inner: std::sync::Mutex::new(FaceProcessor::new()?),
        })
    }
    
    pub fn detect_and_encode(&self, rgb_data: &[u8], width: u32, height: u32) -> FaceDetectionResult {
        let processor = self.inner.lock().unwrap();
        processor.detect_and_encode(rgb_data, width, height)
    }
    
    pub fn can_encode(&self) -> bool {
        let processor = self.inner.lock().unwrap();
        processor.can_encode()
    }
}
