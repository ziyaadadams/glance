use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::window::GlanceWindow;

mod imp {
    use super::*;
    
    #[derive(Debug, Default)]
    pub struct GlanceApplication;
    
    #[glib::object_subclass]
    impl ObjectSubclass for GlanceApplication {
        const NAME: &'static str = "GlanceApplication";
        type Type = super::GlanceApplication;
        type ParentType = adw::Application;
    }
    
    impl ObjectImpl for GlanceApplication {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_actions();
            obj.load_css();
        }
    }
    
    impl ApplicationImpl for GlanceApplication {
        fn activate(&self) {
            let window = GlanceWindow::new(&self.obj());
            window.present();
        }
    }
    
    impl GtkApplicationImpl for GlanceApplication {}
    impl AdwApplicationImpl for GlanceApplication {}
}

glib::wrapper! {
    pub struct GlanceApplication(ObjectSubclass<imp::GlanceApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl GlanceApplication {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", "io.github.glance.Glance")
            .property("flags", gio::ApplicationFlags::FLAGS_NONE)
            .build()
    }
    
    fn setup_actions(&self) {
        let about_action = gio::ActionEntry::builder("about")
            .activate(|app: &Self, _, _| app.show_about())
            .build();
        
        let prefs_action = gio::ActionEntry::builder("preferences")
            .activate(|app: &Self, _, _| app.show_preferences())
            .build();
        
        let quit_action = gio::ActionEntry::builder("quit")
            .activate(|app: &Self, _, _| app.quit())
            .build();
        
        self.add_action_entries([about_action, prefs_action, quit_action]);
        self.set_accels_for_action("app.quit", &["<Ctrl>q"]);
        self.set_accels_for_action("app.preferences", &["<Ctrl>comma"]);
    }
    
    fn load_css(&self) {
        let css = r#"
            .camera-preview {
                background-color: @card_bg_color;
                border-radius: 12px;
                min-height: 300px;
            }
            .guidance-success { color: @success_color; font-weight: bold; }
            .guidance-warning { color: @warning_color; }
            .guidance-neutral { color: @theme_fg_color; }
            .guidance-error { color: @error_color; font-weight: bold; }
            .camera-info-ir { color: @error_color; font-weight: bold; }
            .camera-info-rgb { color: @success_color; }
            .capture-spinner { opacity: 0.3; }
            .capture-spinner:checked { opacity: 1.0; }
            .capture-face-icon { opacity: 0.8; color: @accent_color; }
            .capture-face-icon.face-found { opacity: 1.0; color: @success_color; }
            .capture-progress { border-radius: 6px; }
            .capture-progress progress { background-color: @accent_color; border-radius: 6px; }
        "#;
        
        let provider = gtk::CssProvider::new();
        provider.load_from_string(css);
        
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }
    
    fn show_about(&self) {
        let window = self.active_window();
        
        let dialog = adw::AboutWindow::builder()
            .application_name("Glance")
            .application_icon("face-smile-symbolic")
            .developer_name("Glance Team")
            .version("1.0.0")
            .copyright("© 2024-2025 Glance Team")
            .license_type(gtk::License::Gpl30)
            .website("https://github.com/glance-linux/glance")
            .comments("Windows Hello-style facial recognition for Linux")
            .modal(true)
            .build();
        
        if let Some(win) = window {
            dialog.set_transient_for(Some(&win));
        }
        dialog.present();
    }
    
    fn show_preferences(&self) {
        let window = self.active_window();
        let dialog = adw::PreferencesWindow::new();
        dialog.set_modal(true);
        
        let general_page = adw::PreferencesPage::builder()
            .title("General")
            .icon_name("preferences-system-symbolic")
            .build();
        
        let camera_group = adw::PreferencesGroup::builder()
            .title("Camera")
            .description("Configure camera settings")
            .build();
        
        let prefer_ir = adw::SwitchRow::builder()
            .title("Prefer IR Camera")
            .subtitle("Use infrared camera when available for better security")
            .active(true)
            .build();
        
        camera_group.add(&prefer_ir);
        general_page.add(&camera_group);
        
        let security_group = adw::PreferencesGroup::builder()
            .title("Security")
            .build();
        
        let threshold_row = adw::SpinRow::builder()
            .title("Match Threshold")
            .subtitle("Lower values are more strict")
            .build();
        threshold_row.set_adjustment(Some(&gtk::Adjustment::new(0.45, 0.3, 0.6, 0.05, 0.1, 0.0)));
        
        security_group.add(&threshold_row);
        general_page.add(&security_group);
        dialog.add(&general_page);
        
        if let Some(win) = window {
            dialog.set_transient_for(Some(&win));
        }
        dialog.present();
    }
}
