use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;
use gtk::gio;

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use crate::app::GlanceApplication;
use crate::camera::{Camera, CameraFrame, CameraInfo};
use crate::face::SharedFaceProcessor;
use crate::models;
use crate::storage::{FaceData, load_face_data, save_face_data};

mod imp {
    use super::*;
    
    #[derive(Debug, Default)]
    pub struct GlanceWindow {
        // State
        pub camera_info: RefCell<Option<CameraInfo>>,
        pub current_user: RefCell<String>,
        pub is_capturing: Cell<bool>,
        
        // Capture state
        pub consecutive_good_frames: Cell<u32>,
        pub required_good_frames: Cell<u32>,
        pub captured_encodings: RefCell<Vec<(Vec<f64>, String)>>,
        
        // Dual-camera capture state
        pub ir_encodings: RefCell<Vec<(Vec<f64>, String)>>,
        pub rgb_encodings: RefCell<Vec<(Vec<f64>, String)>>,
        pub current_camera_type: RefCell<String>,  // "ir" or "rgb"
        pub completed_ir_capture: Cell<bool>,
        pub completed_rgb_capture: Cell<bool>,
        
        // Guidance smoothing
        pub last_guidance: RefCell<String>,
        pub guidance_stable_frames: Cell<u32>,
        
        // Status debouncing to prevent flickering
        pub last_status: RefCell<String>,
        pub status_stable_frames: Cell<u32>,
        pub frame_count: Cell<u32>,
        
        // Face processor (initialized lazily)
        pub face_processor: RefCell<Option<Arc<SharedFaceProcessor>>>,
        
        // UI widgets
        pub toast_overlay: RefCell<Option<adw::ToastOverlay>>,
        pub navigation: RefCell<Option<adw::NavigationView>>,
        pub status_page: RefCell<Option<adw::StatusPage>>,
        pub lbl_camera_info: RefCell<Option<gtk::Label>>,
        pub lbl_registered_status: RefCell<Option<gtk::Label>>,
        pub btn_add_face: RefCell<Option<gtk::Button>>,
        pub btn_delete_face: RefCell<Option<gtk::Button>>,
        pub lbl_capture_title: RefCell<Option<gtk::Label>>,
        pub lbl_pose_instruction: RefCell<Option<gtk::Label>>,
        pub camera_picture: RefCell<Option<gtk::Picture>>,
        pub lbl_guidance: RefCell<Option<gtk::Label>>,
        pub capture_progress: RefCell<Option<gtk::ProgressBar>>,
        pub capture_spinner: RefCell<Option<gtk::Spinner>>,
        pub capture_face_icon: RefCell<Option<gtk::Image>>,
        pub btn_ir_setup: RefCell<Option<gtk::Button>>,
        pub is_verifying: Cell<bool>,
        pub frame_receiver: RefCell<Option<async_channel::Receiver<CameraFrame>>>,
    }
    
    #[glib::object_subclass]
    impl ObjectSubclass for GlanceWindow {
        const NAME: &'static str = "GlanceWindow";
        type Type = super::GlanceWindow;
        type ParentType = adw::ApplicationWindow;
    }
    
    impl ObjectImpl for GlanceWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            
            // Get current user
            let username = users::get_current_username()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "user".to_string());
            *self.current_user.borrow_mut() = username;
            
            // Configure capture
            self.required_good_frames.set(5);
            
            // Build UI
            obj.build_ui();
            
            // Initialize after UI is ready
            glib::idle_add_local_once(glib::clone!(
                #[weak] obj,
                move || { obj.initialize(); }
            ));
        }
    }
    
    impl WidgetImpl for GlanceWindow {}
    impl WindowImpl for GlanceWindow {}
    impl ApplicationWindowImpl for GlanceWindow {}
    impl AdwApplicationWindowImpl for GlanceWindow {}
}

glib::wrapper! {
    pub struct GlanceWindow(ObjectSubclass<imp::GlanceWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget,
                    gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl GlanceWindow {
    pub fn new(app: &GlanceApplication) -> Self {
        glib::Object::builder()
            .property("application", app)
            .build()
    }
    
    fn build_ui(&self) {
        let imp = self.imp();
        
        // Main container
        let toast_overlay = adw::ToastOverlay::new();
        let navigation = adw::NavigationView::new();
        
        // === Main Page ===
        let main_page = adw::NavigationPage::builder()
            .title("Glance")
            .tag("main")
            .build();
        
        let main_toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        
        // Menu
        let menu_btn = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .build();
        let menu = gio::Menu::new();
        menu.append(Some("_Preferences"), Some("app.preferences"));
        menu.append(Some("_About Glance"), Some("app.about"));
        menu_btn.set_menu_model(Some(&menu));
        header.pack_end(&menu_btn);
        main_toolbar.add_top_bar(&header);
        
        // Status page
        let status_page = adw::StatusPage::builder()
            .icon_name("avatar-default-symbolic")
            .title("Glance")
            .description("Set up facial recognition to sign in quickly and securely.")
            .build();
        
        // Content
        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .halign(gtk::Align::Center)
            .build();
        
        let lbl_camera_info = gtk::Label::builder()
            .label("Detecting camera...")
            .css_classes(["dim-label"])
            .build();
        
        let lbl_registered_status = gtk::Label::builder()
            .label("")
            .css_classes(["heading"])
            .build();
        
        // Buttons
        let btn_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .halign(gtk::Align::Center)
            .build();
        
        let btn_add_face = gtk::Button::builder()
            .label("Add Face")
            .css_classes(["suggested-action", "pill"])
            .build();
        btn_add_face.connect_clicked(glib::clone!(
            #[weak(rename_to = window)] self,
            move |_| { window.show_add_face_dialog(); }
        ));
        
        let btn_delete_face = gtk::Button::builder()
            .label("Remove Face")
            .css_classes(["destructive-action", "pill"])
            .sensitive(false)
            .build();
        btn_delete_face.connect_clicked(glib::clone!(
            #[weak(rename_to = window)] self,
            move |_| { window.show_delete_face_dialog(); }
        ));
        
        let btn_ir_setup = gtk::Button::builder()
            .label("Recalibrate IR Camera")
            .css_classes(["pill"])
            .visible(true)  // Always visible — users need this after kernel updates
            .build();
        btn_ir_setup.connect_clicked(glib::clone!(
            #[weak(rename_to = window)] self,
            move |_| { window.show_ir_setup(); }
        ));
        
        btn_box.append(&btn_add_face);
        btn_box.append(&btn_delete_face);
        btn_box.append(&btn_ir_setup);
        
        content_box.append(&lbl_camera_info);
        content_box.append(&lbl_registered_status);
        content_box.append(&btn_box);
        
        status_page.set_child(Some(&content_box));
        main_toolbar.set_content(Some(&status_page));
        main_page.set_child(Some(&main_toolbar));
        
        // === Capture Page ===
        let capture_page = adw::NavigationPage::builder()
            .title("Capture Face")
            .tag("capture")
            .build();
        
        let capture_toolbar = adw::ToolbarView::new();
        let capture_header = adw::HeaderBar::builder()
            .show_back_button(true)
            .build();
        
        let btn_cancel = gtk::Button::builder()
            .label("Cancel")
            .build();
        btn_cancel.connect_clicked(glib::clone!(
            #[weak(rename_to = window)] self,
            move |_| { window.cancel_capture(); }
        ));
        capture_header.pack_end(&btn_cancel);
        capture_toolbar.add_top_bar(&capture_header);
        
        let capture_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .margin_top(48)
            .margin_bottom(48)
            .margin_start(48)
            .margin_end(48)
            .valign(gtk::Align::Center)
            .halign(gtk::Align::Center)
            .build();
        
        // Windows Hello style: icon with spinner below
        let face_container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .halign(gtk::Align::Center)
            .build();
        
        // Face icon on top
        let face_icon = gtk::Image::builder()
            .icon_name("avatar-default-symbolic")
            .pixel_size(80)
            .halign(gtk::Align::Center)
            .build();
        face_icon.add_css_class("capture-face-icon");
        
        // Spinner below the icon
        let capture_spinner = gtk::Spinner::builder()
            .width_request(48)
            .height_request(48)
            .halign(gtk::Align::Center)
            .build();
        capture_spinner.add_css_class("capture-spinner");
        
        face_container.append(&face_icon);
        face_container.append(&capture_spinner);
        
        // Main status text (like "Looking for you...")
        let lbl_capture_title = gtk::Label::builder()
            .label("Looking for you...")
            .css_classes(["title-1"])
            .build();
        
        // Pose/instruction text
        let lbl_pose_instruction = gtk::Label::builder()
            .label("Position your face in front of the camera")
            .css_classes(["dim-label"])
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build();
        
        // Hidden camera picture (we still capture frames, just don't show them)
        let camera_picture = gtk::Picture::builder()
            .visible(false)
            .build();
        
        // Guidance label
        let lbl_guidance = gtk::Label::builder()
            .label("")
            .css_classes(["guidance-neutral"])
            .build();
        
        // Progress bar (subtle, at the bottom)
        let capture_progress = gtk::ProgressBar::builder()
            .show_text(false)
            .margin_top(16)
            .build();
        capture_progress.add_css_class("capture-progress");
        
        capture_box.append(&face_container);
        capture_box.append(&lbl_capture_title);
        capture_box.append(&lbl_pose_instruction);
        capture_box.append(&camera_picture);
        capture_box.append(&lbl_guidance);
        capture_box.append(&capture_progress);
        
        capture_toolbar.set_content(Some(&capture_box));
        capture_page.set_child(Some(&capture_toolbar));
        
        // Add pages
        navigation.add(&main_page);
        navigation.add(&capture_page);
        
        toast_overlay.set_child(Some(&navigation));
        self.set_content(Some(&toast_overlay));
        
        // Store references
        *imp.toast_overlay.borrow_mut() = Some(toast_overlay);
        *imp.navigation.borrow_mut() = Some(navigation);
        *imp.status_page.borrow_mut() = Some(status_page);
        *imp.lbl_camera_info.borrow_mut() = Some(lbl_camera_info);
        *imp.lbl_registered_status.borrow_mut() = Some(lbl_registered_status);
        *imp.btn_add_face.borrow_mut() = Some(btn_add_face);
        *imp.btn_delete_face.borrow_mut() = Some(btn_delete_face);
        *imp.lbl_capture_title.borrow_mut() = Some(lbl_capture_title);
        *imp.lbl_pose_instruction.borrow_mut() = Some(lbl_pose_instruction);
        *imp.camera_picture.borrow_mut() = Some(camera_picture);
        *imp.lbl_guidance.borrow_mut() = Some(lbl_guidance);
        *imp.capture_progress.borrow_mut() = Some(capture_progress);
        *imp.capture_spinner.borrow_mut() = Some(capture_spinner);
        *imp.capture_face_icon.borrow_mut() = Some(face_icon);
        *imp.btn_ir_setup.borrow_mut() = Some(btn_ir_setup);
        
        self.set_title(Some("Glance"));
        self.set_default_size(500, 700);
    }
    
    fn initialize(&self) {
        // Check if models are installed first
        if !models::models_installed() {
            self.show_model_download_dialog();
            return;
        }
        
        self.initialize_face_processor();
        self.detect_camera();
        self.update_registered_status();
    }
    
    fn initialize_face_processor(&self) {
        // Initialize face processor in background
        let (tx, rx) = async_channel::bounded::<Result<Arc<SharedFaceProcessor>, String>>(1);
        std::thread::spawn(move || {
            let result = SharedFaceProcessor::new().map(Arc::new);
            let _ = tx.send_blocking(result);
        });
        
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = window)] self,
            async move {
                if let Ok(result) = rx.recv().await {
                    match result {
                        Ok(processor) => {
                            let imp = window.imp();
                            let can_encode = processor.can_encode();
                            *imp.face_processor.borrow_mut() = Some(processor);
                            
                            if !can_encode {
                                window.show_toast("Face models failed to load");
                            } else {
                                window.show_toast("Ready for facial recognition");
                            }
                        }
                        Err(e) => {
                            window.show_toast(&format!("Face processor error: {}", e));
                        }
                    }
                }
            }
        ));
    }
    
    fn show_model_download_dialog(&self) {
        let dialog = adw::MessageDialog::builder()
            .heading("Face Models Required")
            .body("Glance needs to download face recognition models (~122 MB) to function.\n\nThis is a one-time download from dlib.net.")
            .modal(true)
            .transient_for(self)
            .build();
        
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("download", "Download Models");
        dialog.set_response_appearance("download", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("download"));
        dialog.set_close_response("cancel");
        
        dialog.connect_response(None, glib::clone!(
            #[weak(rename_to = window)] self,
            move |_, response| {
                if response == "download" {
                    window.start_model_download();
                } else {
                    // Show warning and continue without models
                    window.show_toast("Models not installed - face capture won't work");
                    window.detect_camera();
                    window.update_registered_status();
                }
            }
        ));
        
        dialog.present();
    }
    
    fn start_model_download(&self) {
        // Create progress dialog
        let dialog = adw::Window::builder()
            .title("Downloading Models")
            .default_width(400)
            .default_height(200)
            .modal(true)
            .transient_for(self)
            .deletable(false)
            .build();
        
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .margin_top(32)
            .margin_bottom(32)
            .margin_start(32)
            .margin_end(32)
            .valign(gtk::Align::Center)
            .build();
        
        let spinner = gtk::Spinner::builder()
            .width_request(48)
            .height_request(48)
            .halign(gtk::Align::Center)
            .build();
        spinner.start();
        
        let status_label = gtk::Label::builder()
            .label("Downloading face recognition models...")
            .css_classes(["title-3"])
            .halign(gtk::Align::Center)
            .build();
        
        let progress_bar = gtk::ProgressBar::builder()
            .show_text(true)
            .build();
        
        let detail_label = gtk::Label::builder()
            .label("This may take a few minutes depending on your connection.")
            .css_classes(["dim-label"])
            .wrap(true)
            .halign(gtk::Align::Center)
            .build();
        
        content.append(&spinner);
        content.append(&status_label);
        content.append(&progress_bar);
        content.append(&detail_label);
        
        dialog.set_content(Some(&content));
        dialog.present();
        
        // Start download in background
        let (tx, rx) = async_channel::bounded::<Result<(), String>>(1);
        
        std::thread::spawn(move || {
            let result = models::download_models_sync();
            let _ = tx.send_blocking(result);
        });
        
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = window)] self,
            #[weak] dialog,
            async move {
                if let Ok(result) = rx.recv().await {
                    dialog.close();
                    
                    match result {
                        Ok(_) => {
                            window.show_toast("Models downloaded successfully");
                            // Now initialize face processor
                            window.initialize_face_processor();
                            window.detect_camera();
                            window.update_registered_status();
                        }
                        Err(e) => {
                            window.show_download_error_dialog(&e);
                        }
                    }
                }
            }
        ));
    }
    
    fn show_download_error_dialog(&self, error: &str) {
        let dialog = adw::MessageDialog::builder()
            .heading("Download Failed")
            .body(&format!("Failed to download models:\n\n{}\n\nYou can try again later or run install.sh manually.", error))
            .modal(true)
            .transient_for(self)
            .build();
        
        dialog.add_response("retry", "Retry");
        dialog.add_response("close", "Continue Anyway");
        dialog.set_response_appearance("retry", adw::ResponseAppearance::Suggested);
        
        dialog.connect_response(None, glib::clone!(
            #[weak(rename_to = window)] self,
            move |_, response| {
                if response == "retry" {
                    window.start_model_download();
                } else {
                    window.detect_camera();
                    window.update_registered_status();
                }
            }
        ));
        
        dialog.present();
    }
    
    fn detect_camera(&self) {
        let imp = self.imp();
        if let Some(ref lbl) = *imp.lbl_camera_info.borrow() {
            lbl.set_label("Detecting camera...");
        }
        
        let (tx, rx) = async_channel::bounded::<Option<CameraInfo>>(1);
        std::thread::spawn(move || {
            let result = Camera::detect_cameras();
            let _ = tx.send_blocking(result);
        });
        
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = window)] self,
            async move {
                if let Ok(camera_info) = rx.recv().await {
                    window.on_camera_detected(camera_info);
                }
            }
        ));
    }
    
    fn on_camera_detected(&self, camera_info: Option<CameraInfo>) {
        let imp = self.imp();
        
        if let Some(info) = camera_info {
            let label = if info.is_ir {
                format!("IR Camera: /dev/video{}", info.device_id)
            } else {
                format!("RGB Camera: /dev/video{}", info.device_id)
            };
            
            if let Some(ref lbl) = *imp.lbl_camera_info.borrow() {
                lbl.set_label(&label);
            }
            
            // Always show IR setup button - user may need to recalibrate after kernel updates
            if let Some(ref btn) = *imp.btn_ir_setup.borrow() {
                btn.set_visible(true);
                if info.is_ir {
                    btn.set_label("Recalibrate IR Camera");
                } else {
                    btn.set_label("Set Up IR Camera");
                }
            }
            
            *imp.camera_info.borrow_mut() = Some(info);
            
            if let Some(ref btn) = *imp.btn_add_face.borrow() {
                btn.set_sensitive(true);
            }
        } else {
            if let Some(ref lbl) = *imp.lbl_camera_info.borrow() {
                lbl.set_label("No camera detected");
            }
            if let Some(ref btn) = *imp.btn_add_face.borrow() {
                btn.set_sensitive(false);
            }
            // Show IR setup when no camera - might help troubleshooting
            if let Some(ref btn) = *imp.btn_ir_setup.borrow() {
                btn.set_visible(true);
            }
        }
    }
    
    fn update_registered_status(&self) {
        let imp = self.imp();
        let username = imp.current_user.borrow().clone();
        
        if let Some(face_data) = load_face_data(&username) {
            let ir_count = face_data.ir_encodings.len();
            let rgb_count = face_data.rgb_encodings.len();
            let legacy_count = face_data.encodings.len();
            
            let mut parts = Vec::new();
            if ir_count > 0 { parts.push(format!("{} IR", ir_count)); }
            if rgb_count > 0 { parts.push(format!("{} RGB", rgb_count)); }
            if parts.is_empty() && legacy_count > 0 {
                parts.push(format!("{} legacy", legacy_count));
            }
            
            let status_text = if parts.is_empty() {
                "Face registered (no encodings found)".to_string()
            } else {
                format!("Face registered: {} capture(s)", parts.join(", "))
            };
            
            // Warn if RGB is missing — fallback won't work
            let warning = if rgb_count == 0 && ir_count > 0 {
                "\n⚠️ No RGB backup — if IR stops working, face auth will fail."
            } else {
                ""
            };
            
            if let Some(ref lbl) = *imp.lbl_registered_status.borrow() {
                lbl.set_label(&format!("{}{}", status_text, warning));
            }
            if let Some(ref btn) = *imp.btn_add_face.borrow() {
                btn.set_label("Update Face");
            }
            if let Some(ref btn) = *imp.btn_delete_face.borrow() {
                btn.set_sensitive(true);
            }
            if let Some(ref page) = *imp.status_page.borrow() {
                page.set_title("Glance Active");
                page.set_description(Some(&format!(
                    "Your face is registered for {}.\nYou can update your face data anytime.",
                    username
                )));
            }
        } else {
            if let Some(ref lbl) = *imp.lbl_registered_status.borrow() {
                lbl.set_label("No face registered");
            }
            if let Some(ref btn) = *imp.btn_add_face.borrow() {
                btn.set_label("Add Face");
            }
            if let Some(ref btn) = *imp.btn_delete_face.borrow() {
                btn.set_sensitive(false);
            }
            if let Some(ref page) = *imp.status_page.borrow() {
                page.set_title("Glance");
                page.set_description(Some(&format!(
                    "Set up facial recognition for {}\nto sign in quickly and securely.",
                    username
                )));
            }
        }
    }
    
    fn show_add_face_dialog(&self) {
        // For adding a face, we don't require authentication
        // This is safe because:
        // 1. The user already has access to their own session
        // 2. Adding a face only grants access, doesn't remove security
        // 3. Removing a face DOES require authentication (handled separately)
        // 4. Using pkexec would trigger PAM which would try facial recognition = infinite loop!
        
        // Simply start the capture process
        self.start_capture(true);
    }
    
    fn show_delete_face_dialog(&self) {
        let dialog = adw::MessageDialog::builder()
            .heading("Remove Face Data?")
            .body("This will remove your registered face. You'll need to re-register to use facial recognition.")
            .modal(true)
            .transient_for(self)
            .build();
        
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("delete", "Remove");
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        
        dialog.connect_response(None, glib::clone!(
            #[weak(rename_to = window)] self,
            move |_, response| {
                if response == "delete" {
                    window.delete_face_data();
                }
            }
        ));
        
        dialog.present();
    }
    
    fn delete_face_data(&self) {
        let imp = self.imp();
        let username = imp.current_user.borrow().clone();
        
        match crate::storage::delete_face_data(&username) {
            Ok(_) => {
                self.show_toast("Face data removed");
                self.update_registered_status();
            }
            Err(e) => {
                self.show_toast(&format!("Error: {}", e));
            }
        }
    }
    
    fn start_capture(&self, _multi_pose: bool) {
        let imp = self.imp();
        
        // Check if face processor is ready with models
        let can_capture = imp.face_processor.borrow()
            .as_ref()
            .map(|p| p.can_encode())
            .unwrap_or(false);
        
        if !can_capture {
            // Models not loaded - show download dialog instead
            self.show_model_download_dialog();
            return;
        }
        
        // Reset state - dual-camera capture for fallback support
        imp.is_capturing.set(true);
        imp.consecutive_good_frames.set(0);
        imp.captured_encodings.borrow_mut().clear();
        imp.ir_encodings.borrow_mut().clear();
        imp.rgb_encodings.borrow_mut().clear();
        imp.completed_ir_capture.set(false);
        imp.completed_rgb_capture.set(false);
        *imp.last_guidance.borrow_mut() = String::new();
        imp.guidance_stable_frames.set(0);
        *imp.last_status.borrow_mut() = String::new();
        imp.status_stable_frames.set(0);
        imp.frame_count.set(0);
        
        // Detect available cameras and decide capture strategy
        let has_ir = Camera::detect_ir_camera().is_some();
        let has_rgb = Camera::detect_rgb_camera().is_some();
        
        // Start with IR camera if available, otherwise RGB
        if has_ir {
            *imp.current_camera_type.borrow_mut() = "ir".to_string();
            if let Some(ir_cam) = Camera::detect_ir_camera() {
                *imp.camera_info.borrow_mut() = Some(ir_cam);
            }
        } else if has_rgb {
            *imp.current_camera_type.borrow_mut() = "rgb".to_string();
            if let Some(rgb_cam) = Camera::detect_rgb_camera() {
                *imp.camera_info.borrow_mut() = Some(rgb_cam);
            }
        }
        
        // Navigate to capture page
        if let Some(ref nav) = *imp.navigation.borrow() {
            nav.push_by_tag("capture");
        }
        
        self.update_pose_ui();
        self.start_camera_preview();
    }
    
    fn update_pose_ui(&self) {
        let imp = self.imp();
        
        // Start the spinner
        if let Some(ref spinner) = *imp.capture_spinner.borrow() {
            spinner.start();
        }
        
        // Reset face icon state
        if let Some(ref icon) = *imp.capture_face_icon.borrow() {
            icon.remove_css_class("face-found");
        }
        
        // Simple single capture UI - like Windows Hello
        if let Some(ref lbl) = *imp.lbl_pose_instruction.borrow() {
            lbl.set_label("Look directly at the camera");
        }
        if let Some(ref lbl) = *imp.lbl_capture_title.borrow() {
            lbl.set_label("Setting up...");
        }
        
        if let Some(ref bar) = *imp.capture_progress.borrow() {
            bar.set_fraction(0.0);
        }
        
        imp.consecutive_good_frames.set(0);
    }
    
    fn start_camera_preview(&self) {
        let imp = self.imp();
        let camera_info = imp.camera_info.borrow().clone();
        
        if let Some(info) = camera_info {
            let (frame_tx, frame_rx) = async_channel::bounded::<CameraFrame>(2);
            *imp.frame_receiver.borrow_mut() = Some(frame_rx.clone());
            
            let device_id = info.device_id;
            
            // Camera thread - capture at ~20fps
            std::thread::spawn(move || {
                if let Ok(mut camera) = Camera::new(device_id) {
                    loop {
                        match camera.read_frame() {
                            Ok(frame) => {
                                if frame_tx.send_blocking(frame).is_err() {
                                    break;
                                }
                                // ~20fps capture rate
                                std::thread::sleep(std::time::Duration::from_millis(50));
                            }
                            Err(_) => break,
                        }
                    }
                }
            });
            
            // Frame processing on main thread
            glib::spawn_future_local(glib::clone!(
                #[weak(rename_to = window)] self,
                async move {
                    while let Ok(frame) = frame_rx.recv().await {
                        if !window.imp().is_capturing.get() {
                            break;
                        }
                        window.process_frame(&frame);
                    }
                }
            ));
        }
    }
    
    fn process_frame(&self, frame: &CameraFrame) {
        let imp = self.imp();
        
        // Throttle: only process every 2nd frame to reduce CPU usage
        let frame_count = imp.frame_count.get() + 1;
        imp.frame_count.set(frame_count);
        if frame_count % 2 != 0 {
            return;
        }
        
        // Get face processor
        let processor = match imp.face_processor.borrow().as_ref() {
            Some(p) => p.clone(),
            None => {
                self.set_capture_status("Initializing...", false);
                return;
            }
        };
        
        // Detect face and get encoding
        let result = processor.detect_and_encode(&frame.rgb_data, frame.width, frame.height);
        
        if !result.face_found {
            self.set_capture_status("Looking for you...", false);
            self.update_guidance("Position your face in front of the camera", "neutral");
            imp.consecutive_good_frames.set(0);
            return;
        }
        
        // Check if we have encoding capability - required for capture
        if !result.has_encoding() {
            // Face found but no encoding yet - this happens sometimes
            self.set_capture_status("We see you!", true);
            if !processor.can_encode() {
                // Models not loaded - abort capture and show download dialog
                self.update_guidance("Face models not loaded", "error");
                imp.consecutive_good_frames.set(0);
                
                // Stop capture after a moment and show download dialog
                if frame_count > 10 {
                    self.cancel_capture();
                    self.show_model_download_dialog();
                    return;
                }
            } else {
                // Encoding sometimes fails momentarily - decrement but don't fully reset
                let frames = imp.consecutive_good_frames.get();
                if frames > 0 {
                    imp.consecutive_good_frames.set(frames.saturating_sub(1));
                }
                self.update_guidance("Hold still, getting a better look...", "neutral");
            }
            return;
        }
        
        // Good frame with encoding - increment counter
        let good_frames = imp.consecutive_good_frames.get() + 1;
        imp.consecutive_good_frames.set(good_frames);
        
        // Mark face as found
        self.set_capture_status("Hold still...", true);
        
        let required = imp.required_good_frames.get();
        let progress = (good_frames as f64) / (required as f64);
        
        if let Some(ref bar) = *imp.capture_progress.borrow() {
            bar.set_fraction(progress.min(1.0));
        }
        
        // Update title based on progress - directly, no debouncing
        if let Some(ref lbl) = *imp.lbl_capture_title.borrow() {
            if good_frames >= required {
                lbl.set_label("That's you!");
            } else if progress >= 0.6 {
                lbl.set_label("Almost there...");
            } else {
                lbl.set_label("Hold still...");
            }
        }
        
        self.update_guidance("Perfect! Stay still...", "success");
        
        // Check if we've captured enough frames
        if good_frames >= required {
            if let Some(encoding) = result.encoding {
                self.on_pose_captured(encoding);
            }
        }
    }
    
    fn on_pose_captured(&self, encoding: Vec<f64>) {
        let imp = self.imp();
        
        let current_type = imp.current_camera_type.borrow().clone();
        
        // Store encoding based on current camera type
        if current_type == "ir" {
            imp.ir_encodings.borrow_mut().push((encoding.clone(), "center".to_string()));
            imp.completed_ir_capture.set(true);
            eprintln!("[Capture] IR camera capture complete");
            
            // Check if RGB camera is available for fallback capture
            if let Some(rgb_cam) = Camera::detect_rgb_camera() {
                eprintln!("[Capture] Switching to RGB camera for fallback capture...");
                
                // Stop current capture
                imp.is_capturing.set(false);
                *imp.frame_receiver.borrow_mut() = None;
                
                // Switch to RGB camera
                *imp.camera_info.borrow_mut() = Some(rgb_cam);
                *imp.current_camera_type.borrow_mut() = "rgb".to_string();
                
                // Reset capture state for RGB
                imp.consecutive_good_frames.set(0);
                imp.frame_count.set(0);
                imp.is_capturing.set(true);
                
                // Update UI
                self.set_capture_status("Now capturing with regular camera...", false);
                if let Some(ref bar) = *imp.capture_progress.borrow() {
                    bar.set_fraction(0.0);
                }
                if let Some(ref lbl) = *imp.lbl_pose_instruction.borrow() {
                    lbl.set_label("Look at the camera (RGB backup)");
                }
                
                // Start RGB camera preview
                self.start_camera_preview();
                return;
            }
        } else if current_type == "rgb" {
            imp.rgb_encodings.borrow_mut().push((encoding.clone(), "center".to_string()));
            imp.completed_rgb_capture.set(true);
            eprintln!("[Capture] RGB camera capture complete");
        }
        
        // Also store in legacy encodings for backwards compatibility
        imp.captured_encodings.borrow_mut().push((encoding, "center".to_string()));
        
        // If we've captured from all available cameras, save
        let has_ir = Camera::detect_ir_camera().is_some();
        let has_rgb = Camera::detect_rgb_camera().is_some();
        
        let ir_done = !has_ir || imp.completed_ir_capture.get();
        let rgb_done = !has_rgb || imp.completed_rgb_capture.get();
        
        if ir_done && rgb_done {
            self.save_captured_face();
        }
    }
    
    fn save_captured_face(&self) {
        let imp = self.imp();
        
        imp.is_capturing.set(false);
        *imp.frame_receiver.borrow_mut() = None;
        
        // Stop spinner
        if let Some(ref spinner) = *imp.capture_spinner.borrow() {
            spinner.stop();
        }
        
        // Update UI to show success
        self.set_capture_status("All done!", true);
        
        let username = imp.current_user.borrow().clone();
        let ir_encodings = imp.ir_encodings.borrow().clone();
        let rgb_encodings = imp.rgb_encodings.borrow().clone();
        let legacy_encodings = imp.captured_encodings.borrow().clone();
        
        // Create face data with both IR and RGB encodings
        let mut face_data = FaceData::new(&username);
        
        // Add IR encodings
        for (encoding, pose) in &ir_encodings {
            face_data.add_ir_encoding(encoding.clone(), pose);
        }
        face_data.ir_captured = !ir_encodings.is_empty();
        
        // Add RGB encodings
        for (encoding, pose) in &rgb_encodings {
            face_data.add_rgb_encoding(encoding.clone(), pose);
        }
        face_data.rgb_captured = !rgb_encodings.is_empty();
        
        // Also add to legacy encodings for backwards compatibility
        for (encoding, pose) in legacy_encodings {
            face_data.add_encoding(encoding, &pose);
        }
        
        let total_encodings = face_data.ir_encodings.len() + face_data.rgb_encodings.len();
        eprintln!("[Save] IR encodings: {}, RGB encodings: {}", 
                  face_data.ir_encodings.len(), face_data.rgb_encodings.len());
        
        // Save
        let save_result = save_face_data(&face_data);
        
        // Return to main page first
        if let Some(ref nav) = *imp.navigation.borrow() {
            nav.pop();
        }
        
        self.update_registered_status();
        
        // Show success dialog with instructions
        match save_result {
            Ok(_) => {
                self.show_success_dialog_dual(face_data.ir_encodings.len(), face_data.rgb_encodings.len());
            }
            Err(e) => {
                self.show_toast(&format!("Error saving: {}", e));
            }
        }
    }
    
    fn show_success_dialog_dual(&self, ir_count: usize, rgb_count: usize) {
        let mut body = String::new();
        
        if ir_count > 0 && rgb_count > 0 {
            body = format!(
                "Your face has been registered with both cameras:\n\
                • {} IR camera capture(s)\n\
                • {} RGB camera capture(s)\n\n\
                This provides fallback authentication if one camera stops working.\n\n\
                Note: PAM must be configured via install.sh for authentication to work.",
                ir_count, rgb_count
            );
        } else if ir_count > 0 {
            body = format!(
                "Your face has been registered with {} IR camera capture(s).\n\n\
                You can now use facial recognition to log in.\n\n\
                Note: PAM must be configured via install.sh for authentication to work.",
                ir_count
            );
        } else if rgb_count > 0 {
            body = format!(
                "Your face has been registered with {} RGB camera capture(s).\n\n\
                You can now use facial recognition to log in.\n\n\
                Note: PAM must be configured via install.sh for authentication to work.",
                rgb_count
            );
        }
        
        let dialog = adw::MessageDialog::builder()
            .heading("You're All Set")
            .body(&body)
            .modal(true)
            .transient_for(self)
            .build();
        
        dialog.add_response("ok", "OK");
        dialog.set_default_response(Some("ok"));
        dialog.present();
    }
    
    fn show_success_dialog(&self, pose_count: usize) {
        let dialog = adw::MessageDialog::builder()
            .heading("You're All Set")
            .body(&format!(
                "Your face has been registered with {} pose(s).\n\n\
                You can now use facial recognition to log in. If your face \
                is not recognised, you will be prompted for your password or fingerprint.\n\n\
                Note: PAM must be configured via install.sh for authentication to work.",
                pose_count
            ))
            .modal(true)
            .transient_for(self)
            .build();
        
        dialog.add_response("close", "Got it!");
        dialog.set_default_response(Some("close"));
        dialog.present();
    }
    
    /// Update the capture status (title and icon visual state) with debouncing
    fn set_capture_status(&self, title: &str, face_found: bool) {
        let imp = self.imp();
        
        // Debounce status changes to prevent flickering
        let last_status = imp.last_status.borrow().clone();
        if last_status == title {
            // Same status, increment stable counter
            imp.status_stable_frames.set(imp.status_stable_frames.get() + 1);
        } else {
            // Different status - only update if we've been stable for a few frames
            // This prevents rapid flickering between states
            let stable = imp.status_stable_frames.get();
            if stable >= 2 || last_status.is_empty() {
                // Update the display
                if let Some(ref lbl) = *imp.lbl_capture_title.borrow() {
                    lbl.set_label(title);
                }
                
                if let Some(ref icon) = *imp.capture_face_icon.borrow() {
                    if face_found {
                        icon.add_css_class("face-found");
                    } else {
                        icon.remove_css_class("face-found");
                    }
                }
                
                *imp.last_status.borrow_mut() = title.to_string();
            }
            imp.status_stable_frames.set(0);
        }
    }
    
    fn update_guidance(&self, text: &str, style: &str) {
        let imp = self.imp();
        
        // Debounce: only update after seeing the same guidance 2+ times
        let last = imp.last_guidance.borrow().clone();
        if last == text {
            // Same guidance, just increment counter
            let count = imp.guidance_stable_frames.get() + 1;
            imp.guidance_stable_frames.set(count);
            
            // After seeing this guidance twice, apply it
            if count == 2 {
                if let Some(ref lbl) = *imp.lbl_guidance.borrow() {
                    lbl.set_label(text);
                    
                    // Update style
                    lbl.remove_css_class("guidance-neutral");
                    lbl.remove_css_class("guidance-success");
                    lbl.remove_css_class("guidance-warning");
                    lbl.remove_css_class("guidance-error");
                    lbl.add_css_class(&format!("guidance-{}", style));
                }
            }
        } else {
            // Different guidance - start counting from 1
            *imp.last_guidance.borrow_mut() = text.to_string();
            imp.guidance_stable_frames.set(1);
        }
    }
    
    fn cancel_capture(&self) {
        let imp = self.imp();
        
        imp.is_capturing.set(false);
        *imp.frame_receiver.borrow_mut() = None;
        
        // Stop spinner
        if let Some(ref spinner) = *imp.capture_spinner.borrow() {
            spinner.stop();
        }
        
        if let Some(ref nav) = *imp.navigation.borrow() {
            nav.pop();
        }
    }
    
    fn show_ir_setup(&self) {
        // Check current status
        let ir_tool_installed = Self::check_ir_emitter_installed();
        let ir_configured = Self::check_ir_emitter_configured();
        let pam_configured = Self::check_pam_ir_configured();
        
        // Create dialog
        let dialog = adw::Window::builder()
            .title("IR Camera Setup")
            .default_width(550)
            .default_height(500)
            .modal(true)
            .transient_for(self)
            .build();
        
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        toolbar.add_top_bar(&header);
        
        let scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .build();
        
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        
        let title = gtk::Label::builder()
            .label("IR Camera Setup")
            .css_classes(["title-2"])
            .build();
        
        let desc = gtk::Label::builder()
            .label("Configure your Windows Hello compatible IR camera for face authentication.")
            .wrap(true)
            .build();
        
        content.append(&title);
        content.append(&desc);
        
        // Status Section
        let status_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_top(16)
            .build();
        
        let status_title = gtk::Label::builder()
            .label("Status")
            .css_classes(["heading"])
            .halign(gtk::Align::Start)
            .build();
        status_box.append(&status_title);
        
        // Tool installed status
        let tool_row = Self::create_status_row(
            "IR Emitter Tool",
            if ir_tool_installed { "Installed" } else { "Not installed" },
            ir_tool_installed
        );
        status_box.append(&tool_row);
        
        // Configuration status
        let config_row = Self::create_status_row(
            "IR Configuration",
            if ir_configured { "Configured" } else { "Not configured" },
            ir_configured
        );
        status_box.append(&config_row);
        
        // PAM integration status
        let pam_row = Self::create_status_row(
            "PAM Integration",
            if pam_configured { "Enabled" } else { "Not enabled" },
            pam_configured
        );
        status_box.append(&pam_row);
        
        content.append(&status_box);
        
        // Actions Section
        let actions_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(24)
            .build();
        
        let actions_title = gtk::Label::builder()
            .label("Actions")
            .css_classes(["heading"])
            .halign(gtk::Align::Start)
            .build();
        actions_box.append(&actions_title);
        
        // Install button
        if !ir_tool_installed {
            let install_btn = gtk::Button::builder()
                .label("📥 Download & Install IR Emitter Tool")
                .css_classes(["suggested-action"])
                .build();
            let dialog_weak = dialog.downgrade();
            let self_weak = self.downgrade();
            install_btn.connect_clicked(move |btn| {
                btn.set_sensitive(false);
                btn.set_label("Installing...");
                if let Some(window) = self_weak.upgrade() {
                    window.install_ir_emitter_tool();
                }
                if let Some(d) = dialog_weak.upgrade() {
                    d.close();
                }
            });
            actions_box.append(&install_btn);
        }
        
        // Configure / Reconfigure button - always available when tool is installed
        if ir_tool_installed {
            let config_label = if ir_configured {
                "🔄 Recalibrate IR Emitter"
            } else {
                "⚙️ Configure IR Emitter"
            };
            let config_btn = gtk::Button::builder()
                .label(config_label)
                .css_classes(["suggested-action"])
                .build();
            let dialog_weak = dialog.downgrade();
            let self_weak = self.downgrade();
            config_btn.connect_clicked(move |_| {
                if let Some(window) = self_weak.upgrade() {
                    window.run_ir_emitter_configure();
                }
                if let Some(d) = dialog_weak.upgrade() {
                    d.close();
                }
            });
            actions_box.append(&config_btn);
            
            let note = gtk::Label::builder()
                .label("This will open a terminal. Run this after kernel updates if the IR camera stops working.\nAnswer Y/N when the IR LED flashes.")
                .wrap(true)
                .css_classes(["dim-label"])
                .build();
            actions_box.append(&note);
        }
        
        // Setup PAM button
        if ir_tool_installed && ir_configured && !pam_configured {
            let pam_btn = gtk::Button::builder()
                .label("🔐 Enable PAM Integration")
                .css_classes(["suggested-action"])
                .build();
            let dialog_weak = dialog.downgrade();
            let self_weak = self.downgrade();
            pam_btn.connect_clicked(move |_| {
                if let Some(window) = self_weak.upgrade() {
                    window.setup_pam_ir_integration();
                }
                if let Some(d) = dialog_weak.upgrade() {
                    d.close();
                }
            });
            actions_box.append(&pam_btn);
        }
        
        // Test button
        if ir_tool_installed {
            let test_btn = gtk::Button::builder()
                .label("🧪 Test IR Camera")
                .build();
            let self_weak = self.downgrade();
            test_btn.connect_clicked(move |_| {
                if let Some(window) = self_weak.upgrade() {
                    window.test_ir_camera();
                }
            });
            actions_box.append(&test_btn);
        }
        
        // All done message
        if ir_tool_installed && ir_configured && pam_configured {
            let done_label = gtk::Label::builder()
                .label("✅ IR camera is fully configured and ready to use!")
                .css_classes(["success"])
                .wrap(true)
                .build();
            actions_box.append(&done_label);
        }
        
        content.append(&actions_box);
        
        // Close button
        let close_btn = gtk::Button::builder()
            .label("Close")
            .css_classes(["pill"])
            .halign(gtk::Align::Center)
            .margin_top(24)
            .build();
        close_btn.connect_clicked(glib::clone!(
            #[weak] dialog,
            move |_| { dialog.close(); }
        ));
        content.append(&close_btn);
        
        scroll.set_child(Some(&content));
        toolbar.set_content(Some(&scroll));
        dialog.set_content(Some(&toolbar));
        dialog.present();
    }
    
    fn create_status_row(label: &str, status: &str, is_ok: bool) -> gtk::Box {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();
        
        let icon = if is_ok { "emblem-ok-symbolic" } else { "dialog-warning-symbolic" };
        let icon_widget = gtk::Image::from_icon_name(icon);
        if is_ok {
            icon_widget.add_css_class("success");
        } else {
            icon_widget.add_css_class("warning");
        }
        
        let label_widget = gtk::Label::builder()
            .label(label)
            .hexpand(true)
            .halign(gtk::Align::Start)
            .build();
        
        let status_widget = gtk::Label::builder()
            .label(status)
            .css_classes(["dim-label"])
            .build();
        
        row.append(&icon_widget);
        row.append(&label_widget);
        row.append(&status_widget);
        
        row
    }
    
    fn check_ir_emitter_installed() -> bool {
        // Check common locations
        let paths = [
            dirs::home_dir().map(|h| h.join(".local/bin/linux-enable-ir-emitter")),
            Some(std::path::PathBuf::from("/usr/bin/linux-enable-ir-emitter")),
            Some(std::path::PathBuf::from("/usr/local/bin/linux-enable-ir-emitter")),
        ];
        
        for path in paths.iter().flatten() {
            if path.exists() {
                return true;
            }
        }
        
        // Also check via which
        std::process::Command::new("which")
            .arg("linux-enable-ir-emitter")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    
    fn check_ir_emitter_configured() -> bool {
        // v6.x stores config as .ini files in /etc/linux-enable-ir-emitter/
        let config_dir = std::path::Path::new("/etc/linux-enable-ir-emitter");
        if config_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(config_dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().map_or(false, |e| e == "ini") {
                        return true;
                    }
                }
            }
        }
        
        // v7+ stores config as .toml in $HOME/.config/
        if let Some(home) = dirs::home_dir() {
            if home.join(".config/linux-enable-ir-emitter.toml").exists() {
                return true;
            }
        }
        
        false
    }
    
    fn check_pam_ir_configured() -> bool {
        // Check if our PAM module is installed and enabled
        if let Ok(content) = std::fs::read_to_string("/etc/pam.d/common-auth") {
            // Check for pam_glance.so (our module that handles IR + RGB auth)
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.starts_with('#') && trimmed.contains("pam_glance.so") {
                    return true;
                }
            }
        }
        false
    }
    
    fn install_ir_emitter_tool(&self) {
        self.show_toast("Opening terminal to install IR emitter tool...");
        
        let script = r#"#!/bin/bash
echo 'Installing linux-enable-ir-emitter...'
echo ''
cd /tmp
wget -q --show-progress -O ir-emitter.tar.gz \
  'https://github.com/EmixamPP/linux-enable-ir-emitter/releases/download/6.1.2/linux-enable-ir-emitter-6.1.2-release.systemd.x86-64.tar.gz'
if sudo tar -C / --no-same-owner -m -xzf ir-emitter.tar.gz; then
    rm -f ir-emitter.tar.gz
    echo ''
    echo '✅ Installed successfully!'
    linux-enable-ir-emitter --version 2>&1 | head -1
else
    echo ''
    echo '❌ Installation failed.'
fi
echo ''
read -r -p 'Press Enter to close...'
"#;
        match Self::spawn_terminal_script(script) {
            Ok(_) => {},
            Err(e) => self.show_toast(&format!("Could not open terminal: {}", e)),
        }
    }
    
    fn run_ir_emitter_configure(&self) {
        self.show_toast("Opening terminal for IR calibration...");
        
        let tool = Self::find_ir_emitter_tool();
        
        // CRITICAL: Every bare "sudo" triggers PAM auth, which runs pam_glance.so,
        // which tries to open the camera — conflicting with calibration.
        // Fix: cache sudo credentials ONCE upfront with "sudo -v", then use "sudo -n"
        // (non-interactive, uses cached creds, no PAM) for all subsequent commands.
        // A background keepalive prevents credential expiry during long calibrations.
        let script = format!(r#"#!/bin/bash
echo '╔══════════════════════════════════════╗'
echo '║     IR Camera Calibration            ║'
echo '╚══════════════════════════════════════╝'
echo ''

# Cache sudo credentials upfront (this is the ONLY time PAM auth runs).
# At this point the camera is free, so pam_glance won't conflict.
echo 'Authenticating...'
if ! sudo -v; then
    echo '❌ Authentication failed.'
    read -r -p 'Press Enter to close...'
    exit 1
fi

# Keep sudo credentials alive in background (renew every 50s, timeout is usually 60s)
( while true; do sudo -n -v 2>/dev/null; sleep 50; done ) &
KEEPALIVE_PID=$!
trap "kill $KEEPALIVE_PID 2>/dev/null" EXIT

# Ensure config and log directories exist (sudo -n = no PAM, uses cached creds)
sudo -n mkdir -p /etc/linux-enable-ir-emitter
sudo -n mkdir -p /var/local/log/linux-enable-ir-emitter
sudo -n chmod 777 /var/local/log/linux-enable-ir-emitter 2>/dev/null

echo ''
echo 'Stand in front of and close to the camera.'
echo 'Make sure the room is well lit.'
echo ''
echo 'You will be asked if the IR emitter is flashing.'
echo 'Answer "yes" or "no" for each attempt.'
echo ''
echo 'Starting calibration...'
echo ''

sudo -n {tool} configure

echo ''
if ls /etc/linux-enable-ir-emitter/*.ini 1>/dev/null 2>&1; then
    echo '✅ Calibration successful! Config saved.'
    echo ''
    echo 'Testing IR emitter...'
    sudo -n {tool} run
    sleep 2
    echo ''
    echo 'If you see the IR LED glowing, you are all set!'
else
    echo 'ℹ️  Calibration finished. Verifying...'
    sudo -n {tool} test 2>&1
fi

echo ''
read -r -p 'Press Enter to close...'
"#, tool = tool);
        match Self::spawn_terminal_script(&script) {
            Ok(_) => {},
            Err(e) => self.show_toast(&format!("Could not open terminal: {}", e)),
        }
    }
    
    fn setup_pam_ir_integration(&self) {
        self.show_toast("Setting up PAM integration...");
        
        let script = r#"#!/bin/bash
echo 'Checking PAM configuration...'
if grep -q 'pam_glance.so' /etc/pam.d/common-auth; then
    echo '✅ pam_glance.so is already configured.'
else
    echo 'Adding pam_glance.so to /etc/pam.d/common-auth...'
    sudo sed -i '1i auth sufficient pam_glance.so' /etc/pam.d/common-auth
    echo '✅ Done! Face auth is now enabled.'
fi
echo ''
echo 'The Glance PAM module handles the IR emitter'
echo 'automatically — no separate pam_exec line needed.'
echo ''
read -r -p 'Press Enter to close...'
"#;
        match Self::spawn_terminal_script(script) {
            Ok(_) => {},
            Err(e) => self.show_toast(&format!("Could not open terminal: {}", e)),
        }
    }
    
    fn test_ir_camera(&self) {
        self.show_toast("Testing IR camera...");
        
        let tool = Self::find_ir_emitter_tool();
        let ir_device = Camera::detect_ir_camera()
            .map(|c| format!("/dev/video{}", c.device_id))
            .unwrap_or_else(|| "/dev/video2".to_string());
        
        let script = format!(r#"#!/bin/bash
echo 'Testing IR emitter on {device}...'
echo ''

# Cache sudo creds once upfront (avoids PAM/camera conflict)
if ! sudo -v; then
    echo '❌ Authentication failed.'
    read -r -p 'Press Enter to close...'
    exit 1
fi

if sudo -n {tool} -d {device} run; then
    echo '✅ IR emitter enabled!'
else
    echo '❌ No config found — run calibration first.'
fi
echo ''
sudo -n {tool} -d {device} test
echo ''
read -r -p 'Press Enter to close...'
"#, tool = tool, device = ir_device);
        match Self::spawn_terminal_script(&script) {
            Ok(_) => {},
            Err(e) => self.show_toast(&format!("Could not open terminal: {}", e)),
        }
    }
    
    /// Check if a command exists on PATH
    fn command_exists(name: &str) -> bool {
        std::process::Command::new("which")
            .arg(name)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Write a bash script to a temp file and open the user's default terminal to run it.
    /// The temp file approach avoids all quoting/escaping issues when passing complex
    /// scripts through xdg-terminal-exec / ptyxis / ghostty argument chains.
    fn spawn_terminal_script(script: &str) -> std::io::Result<std::process::Child> {
        use std::io::Write;

        // Write script to a temp file
        let script_path = std::env::temp_dir().join("glance-terminal-script.sh");
        {
            let mut f = std::fs::File::create(&script_path)?;
            f.write_all(script.as_bytes())?;
        }

        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;
        }

        let path_str = script_path.to_string_lossy().to_string();

        // Method 1: xdg-terminal-exec (XDG standard — respects user's default terminal)
        if Self::command_exists("xdg-terminal-exec") {
            return std::process::Command::new("xdg-terminal-exec")
                .arg("bash")
                .arg(&path_str)
                .spawn();
        }

        // Method 2: $TERMINAL environment variable
        if let Ok(term) = std::env::var("TERMINAL") {
            if !term.is_empty() && Self::command_exists(&term) {
                return std::process::Command::new(&term)
                    .arg("-e")
                    .arg("bash")
                    .arg(&path_str)
                    .spawn();
            }
        }

        // Method 3: x-terminal-emulator (Debian/Ubuntu alternatives system)
        if Self::command_exists("x-terminal-emulator") {
            return std::process::Command::new("x-terminal-emulator")
                .arg("-e")
                .arg("bash")
                .arg(&path_str)
                .spawn();
        }

        // Method 4: Try common terminals as last resort
        let fallbacks: &[(&str, &[&str])] = &[
            ("gnome-terminal", &["--", "bash"]),
            ("kgx", &["-e", "bash"]),
            ("ptyxis", &["--", "bash"]),
            ("konsole", &["-e", "bash"]),
            ("xfce4-terminal", &["-e", "bash"]),
            ("mate-terminal", &["-e", "bash"]),
            ("tilix", &["-e", "bash"]),
            ("kitty", &["bash"]),
            ("alacritty", &["-e", "bash"]),
            ("ghostty", &["-e", "bash"]),
            ("wezterm", &["start", "--", "bash"]),
            ("foot", &["bash"]),
            ("xterm", &["-e", "bash"]),
        ];

        for (name, args) in fallbacks {
            if Self::command_exists(name) {
                let mut cmd = std::process::Command::new(name);
                for a in *args {
                    cmd.arg(a);
                }
                cmd.arg(&path_str);
                return cmd.spawn();
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No terminal emulator found. Please install one or set $TERMINAL.",
        ))
    }

    /// Find the linux-enable-ir-emitter binary
    fn find_ir_emitter_tool() -> String {
        let paths = [
            "/usr/local/bin/linux-enable-ir-emitter",
            "/usr/bin/linux-enable-ir-emitter",
        ];
        for p in &paths {
            if std::path::Path::new(p).exists() {
                return p.to_string();
            }
        }
        if let Some(home) = dirs::home_dir() {
            let user_path = home.join(".local/bin/linux-enable-ir-emitter");
            if user_path.exists() {
                return user_path.to_string_lossy().to_string();
            }
        }
        "linux-enable-ir-emitter".to_string()
    }
    
    fn show_toast(&self, message: &str) {
        let imp = self.imp();
        if let Some(ref overlay) = *imp.toast_overlay.borrow() {
            overlay.add_toast(adw::Toast::new(message));
        }
    }
}
