use gtk::prelude::*;
use crate::network;

pub fn create_source_window(app: &gtk::Application, camera_device: &str) -> gtk::ApplicationWindow {
    let window = gtk::ApplicationWindow::new(app);
    window.set_title(Some("DSC Source — Stream Info"));
    window.set_default_size(380, 260);

    let container = gtk::Box::new(gtk::Orientation::Vertical, 12);
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
    let type_label = gtk::Label::builder()
        .label(&format!("Type: Camera {}", camera_device))
        .halign(gtk::Align::Start)
        .build();
    let ip_label = gtk::Label::builder()
        .label(&format!("Source IP: {}", local_ip))
        .halign(gtk::Align::Start)
        .build();
    let sign_label = gtk::Label::builder()
        .label("DSC Signing: Active")
        .halign(gtk::Align::Start)
        .build();
    let status_label = gtk::Label::builder()
        .label(&format!("Streaming → WHIP http://{}:8190", local_ip))
        .halign(gtk::Align::Start)
        .build();

    container.append(&type_label);
    container.append(&ip_label);
    container.append(&sign_label);
    container.append(&status_label);

    window.set_child(Some(&container));
    window
}
