use crate::network;
use crate::source::{
    detect_camera_name, set_anonymizer_effect, set_anonymizer_enabled, set_anonymizer_intensity,
    SourceControls,
};
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

    // AI anonymizer controls
    let (available, enabled, effect, intensity) = {
        let c = controls.lock().unwrap();
        (c.available, c.enabled, c.effect, c.effect_intensity)
    };

    let sec_ai = gtk::Label::builder()
        .label("AI Anonymizer")
        .css_classes(["section-title"])
        .halign(gtk::Align::Start)
        .margin_top(16)
        .build();
    container.append(&sec_ai);

    let ai_switch = gtk::Switch::builder()
        .active(enabled)
        .valign(gtk::Align::Center)
        .build();
    ai_switch.set_sensitive(available);
    let ai_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let ai_label = gtk::Label::builder()
        .label("Face anonymizer")
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();
    ai_row.append(&ai_label);
    ai_row.append(&ai_switch);
    container.append(&ai_row);

    let effect_label = gtk::Label::builder()
        .label("Effect")
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();
    let effect_dropdown = gtk::DropDown::from_strings(&["Pixelate", "Blur", "Opaque"]);
    effect_dropdown.set_selected(effect);
    effect_dropdown.set_sensitive(available && enabled);
    let effect_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    effect_row.append(&effect_label);
    effect_row.append(&effect_dropdown);
    container.append(&effect_row);

    let intensity_label = gtk::Label::builder()
        .label("Intensity")
        .css_classes(["data-label"])
        .halign(gtk::Align::Start)
        .build();
    let intensity_scale =
        gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    intensity_scale.set_value(intensity as f64);
    intensity_scale.set_hexpand(true);
    intensity_scale.set_sensitive(available && enabled);
    let intensity_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    intensity_row.append(&intensity_label);
    intensity_row.append(&intensity_scale);
    container.append(&intensity_row);

    // Toggle the anonymizer live when the switch changes.
    {
        let controls = controls.clone();
        let effect_dropdown = effect_dropdown.clone();
        let intensity_scale = intensity_scale.clone();
        ai_switch.connect_state_set(move |_sw, state| {
            set_anonymizer_enabled(&controls, state);
            effect_dropdown.set_sensitive(state);
            intensity_scale.set_sensitive(state);
            glib::Propagation::Proceed
        });
    }

    // Change the effect (pixelate / blur / opaque) live.
    {
        let controls = controls.clone();
        effect_dropdown.connect_selected_notify(move |dd| {
            set_anonymizer_effect(&controls, dd.selected());
        });
    }

    // Change the effect intensity live.
    {
        let controls = controls.clone();
        intensity_scale.connect_value_changed(move |s| {
            set_anonymizer_intensity(&controls, s.value() as f32);
        });
    }

    window.set_child(Some(&container));
    window
}
