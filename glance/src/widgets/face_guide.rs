//! Face guide overlay widget

use gtk4 as gtk;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::glib;
use gtk::graphene;

use std::cell::Cell;

use crate::face::FaceLocation;

mod imp {
    use super::*;
    
    #[derive(Debug, Default)]
    pub struct FaceGuide {
        pub is_optimal: Cell<bool>,
        pub has_face: Cell<bool>,
        pub face_location: Cell<Option<FaceLocation>>,
    }
    
    #[glib::object_subclass]
    impl ObjectSubclass for FaceGuide {
        const NAME: &'static str = "GlanceFaceGuide";
        type Type = super::FaceGuide;
        type ParentType = gtk::DrawingArea;
    }
    
    impl ObjectImpl for FaceGuide {
        fn constructed(&self) {
            self.parent_constructed();
            
            let obj = self.obj();
            
            obj.set_draw_func(|widget, cr, width, height| {
                let guide = widget.downcast_ref::<super::FaceGuide>().unwrap();
                guide.draw(cr, width, height);
            });
        }
    }
    
    impl WidgetImpl for FaceGuide {}
    impl DrawingAreaImpl for FaceGuide {}
}

glib::wrapper! {
    pub struct FaceGuide(ObjectSubclass<imp::FaceGuide>)
        @extends gtk::Widget, gtk::DrawingArea;
}

impl FaceGuide {
    pub fn new() -> Self {
        glib::Object::new()
    }
    
    /// Update the face guide state
    pub fn update(&self, face_location: Option<FaceLocation>, is_optimal: bool) {
        let imp = self.imp();
        
        imp.has_face.set(face_location.is_some());
        imp.is_optimal.set(is_optimal);
        imp.face_location.set(face_location);
        
        self.queue_draw();
    }
    
    /// Draw the face guide overlay
    fn draw(&self, cr: &gtk::cairo::Context, width: i32, height: i32) {
        let imp = self.imp();
        
        let w = width as f64;
        let h = height as f64;
        
        // Determine color based on state
        let (r, g, b) = if imp.is_optimal.get() {
            (0.0, 0.8, 0.0)  // Green
        } else if imp.has_face.get() {
            (1.0, 0.65, 0.0)  // Orange
        } else {
            (0.5, 0.5, 0.5)  // Gray
        };
        
        cr.set_source_rgb(r, g, b);
        cr.set_line_width(3.0);
        
        // Draw oval guide
        let center_x = w / 2.0;
        let center_y = h * 0.45;
        let oval_width = w * 0.22;
        let oval_height = h * 0.32;
        
        // Draw ellipse using arc and scale
        cr.save().unwrap();
        cr.translate(center_x, center_y);
        cr.scale(oval_width, oval_height);
        cr.arc(0.0, 0.0, 1.0, 0.0, 2.0 * std::f64::consts::PI);
        cr.restore().unwrap();
        cr.stroke().unwrap();
        
        // Draw face bounding box if we have a face
        if let Some(face_loc) = imp.face_location.get() {
            cr.set_line_width(2.0);
            cr.rectangle(
                face_loc.left as f64,
                face_loc.top as f64,
                face_loc.width() as f64,
                face_loc.height() as f64,
            );
            cr.stroke().unwrap();
        }
    }
}

impl Default for FaceGuide {
    fn default() -> Self {
        Self::new()
    }
}
