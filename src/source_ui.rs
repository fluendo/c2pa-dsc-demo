use crate::network;
use crate::source::{detect_camera_name, set_facebl0r_enabled, SourceControls};
use gtk::prelude::*;
use std::sync::{Arc, Mutex};

pub fn create_source_window(
    app: &gtk::Application,
    camera_device: &str,
    controls: Arc<Mutex<SourceControls>>,
) -> gtk::ApplicationWindow {
    let window = gtk::ApplicationWindow::new(app);
    window.set_title(Some("DSC Source — Stream Info"));
    window.set_default_size(420, 440);

    let css = gtk::CssProvider::new();
    css.load_from_data(
        ".section-title { font-size: 14px; font-weight: bold; margin-top: 12px; } \
         .data-label   { font-size: 13px; margin: 2px 0; }",
    );
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("No display"),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let container = gtk::Box::new(gtk::Orientation::Vertical, 8);
    container.set_margin_start(16);
    container.set_margin_end(16);
    container.set_margin_top(12);
    container.set_margin_bottom(12);

    let title = gtk::Label::builder()
        .label("Stream Source")
        .css_classes(["section-title"])
        .halign(gtk::Align::Center)
        .build();
    container.append(&title);

    let local_ip = network::local_ip();
    let camera_name = detect_camera_name(camera_device);
    let type_label = gtk::Label::builder()
        .label(&format!("Type: Camera — {}", camera_name))
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();
    let ip_label = gtk::Label::builder()
        .label(&format!("Source IP: {}", local_ip))
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();
    let sign_label = gtk::Label::builder()
        .label("DSC Signing: Active")
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();
    let status_label = gtk::Label::builder()
        .label(&format!("Streaming → WHIP http://{}:8190", local_ip))
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();

    container.append(&type_label);
    container.append(&ip_label);
    container.append(&sign_label);
    container.append(&status_label);

    // Face anonymizer (facebl0r) controls
    let (fb_available, fb_enabled) = {
        let c = controls.lock().unwrap();
        (c.facebl0r_available, c.facebl0r_enabled)
    };

    let sec_fb = gtk::Label::builder()
        .label("Face Anonymizer")
        .css_classes(["section-title"])
        .halign(gtk::Align::Start)
        .margin_top(16)
        .build();
    container.append(&sec_fb);

    let fb_switch = gtk::Switch::builder()
        .active(fb_enabled)
        .valign(gtk::Align::Center)
        .build();
    fb_switch.set_sensitive(fb_available);
    let fb_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let fb_label = gtk::Label::builder()
        .label("Face anonymizer")
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();
    fb_row.append(&fb_label);
    fb_row.append(&fb_switch);
    container.append(&fb_row);

    // Toggle the facebl0r filter live when its switch changes.
    {
        let controls = controls.clone();
        fb_switch.connect_state_set(move |_sw, state| {
            set_facebl0r_enabled(&controls, state);
            glib::Propagation::Proceed
        });
    }

    window.set_child(Some(&container));
    window
}
