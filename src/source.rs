use crate::cert::DscConfig;
use anyhow::Result;
use gst::prelude::*;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const DIGITAL_CAPTURE: &str =
    "http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture";
const TRAINED_ALGORITHMIC_MEDIA: &str =
    "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia";

fn build_manifest_json(ai_filter: bool, camera_name: &str) -> String {
    let mut actions = vec![serde_json::json!({
        "action": "c2pa.created",
        "digitalSourceType": DIGITAL_CAPTURE,
    })];
    if ai_filter {
        actions.push(serde_json::json!({
            "action": "c2pa.edited",
            "digitalSourceType": TRAINED_ALGORITHMIC_MEDIA,
        }));
    }
    let source_title = if camera_name.is_empty() { "Camera" } else { camera_name };
    serde_json::json!({
        "title": "Live Demo",
        "vendor": "fluendo",
        "claim_generator_info": [{"name": "fluendo dscsigner", "version": "1.0"}],
        "ingredients": [{"title": source_title, "format": "video/x-raw", "relationship": "componentOf"}],
        "assertions": [{"label": "c2pa.actions", "data": {"actions": actions}}],
    })
    .to_string()
}

pub fn detect_camera_name(device: &str) -> String {
    if let Ok(out) = Command::new("v4l2-ctl")
        .args(["--device", device, "--info"])
        .output()
    {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut card_type = String::new();
            let mut model = String::new();
            for line in stdout.lines() {
                if let Some(v) = v4l2_field(line, "Card type") {
                    card_type = v.to_string();
                } else if let Some(v) = v4l2_field(line, "Model") {
                    model = v.to_string();
                }
            }
            let raw = if !card_type.is_empty() { &card_type } else { &model };
            if !raw.is_empty() {
                return clean_camera_name(raw);
            }
        }
    }

    if let Some(name) = usb_camera_name(device) {
        return name;
    }

    let devname = device.trim_start_matches('/');
    if let Ok(name) = std::fs::read_to_string(format!("/sys/class/video4linux/{}/name", devname)) {
        let name = clean_camera_name(&name);
        if !name.is_empty() {
            return name;
        }
    }

    device.to_string()
}

fn v4l2_field<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let idx = line.find(field)?;
    let rest = &line[idx + field.len()..];
    Some(rest.trim().trim_start_matches(':').trim())
}

fn clean_camera_name(raw: &str) -> String {
    let name = raw.trim();
    if let Some((left, right)) = name.split_once(':') {
        let left = left.trim();
        let right = right.trim();
        if left.is_empty() {
            return right.to_string();
        }
        if right.is_empty() || left.eq_ignore_ascii_case(right) {
            return left.to_string();
        }
    }
    name.to_string()
}

fn usb_camera_name(device: &str) -> Option<String> {
    let devname = device.trim_start_matches('/');
    let link = format!("/sys/class/video4linux/{}/device", devname);
    let mut path = std::fs::canonicalize(&link).ok()?;
    loop {
        if let Ok(product) = std::fs::read_to_string(path.join("product")) {
            let product = product.trim().to_string();
            if !product.is_empty() {
                let manufacturer = std::fs::read_to_string(path.join("manufacturer"))
                    .ok()
                    .map(|m| m.trim().to_string())
                    .unwrap_or_default();
                if !manufacturer.is_empty()
                    && !product.to_lowercase().contains(&manufacturer.to_lowercase())
                {
                    return Some(format!("{} {}", manufacturer, product));
                }
                return Some(product);
            }
        }
        if let (Ok(vendor), Ok(product_id)) = (
            std::fs::read_to_string(path.join("idVendor")),
            std::fs::read_to_string(path.join("idProduct")),
        ) {
            let vendor = vendor.trim();
            let product_id = product_id.trim();
            if !vendor.is_empty() && !product_id.is_empty() {
                return Some(format!("USB {}:{}", vendor, product_id));
            }
        }
        if !path.pop() {
            return None;
        }
    }
}

fn effect_nick(effect: u32) -> &'static str {
    match effect {
        0 => "pixelate",
        2 => "opaque",
        _ => "blur",
    }
}

/// Live control handles for the source pipeline, shared between the source
/// pipeline thread and the GTK source window.
pub struct SourceControls {
    pub available: bool,
    pub enabled: bool,
    pub effect: u32,
    pub effect_intensity: f32,
    pub camera_name: String,
    selector: Option<gst::Element>,
    selector_raw_pad: Option<gst::Pad>,
    selector_ai_pad: Option<gst::Pad>,
    valve_raw: Option<gst::Element>,
    valve_ai: Option<gst::Element>,
    anonymizer: Option<gst::Element>,
    dscsigner: Option<gst::Element>,
}

impl SourceControls {
    pub fn new() -> Self {
        Self {
            available: false,
            enabled: false,
            effect: 1,
            effect_intensity: 95.0,
            camera_name: String::new(),
            selector: None,
            selector_raw_pad: None,
            selector_ai_pad: None,
            valve_raw: None,
            valve_ai: None,
            anonymizer: None,
            dscsigner: None,
        }
    }
}

/// Toggle the face anonymizer live. Keeps the encoder/signer/WHIP sink (and
/// therefore the WebRTC signaller) running; only the pre-encoder video path
/// is switched via the input-selector. Also re-signs the manifest so the
/// source type assertion flips between digitalCapture and trainedAlgorithmicMedia.
pub fn set_anonymizer_enabled(controls: &Arc<Mutex<SourceControls>>, enabled: bool) {
    let mut c = controls.lock().unwrap();
    if !c.available {
        return;
    }
    c.enabled = enabled;
    if let Some(sel) = &c.selector {
        let pad = if enabled { &c.selector_ai_pad } else { &c.selector_raw_pad };
        if let Some(pad) = pad {
            sel.set_property("active-pad", pad);
        }
    }
    if let Some(v) = &c.valve_raw {
        v.set_property("drop", enabled);
    }
    if let Some(v) = &c.valve_ai {
        v.set_property("drop", !enabled);
    }
    if let Some(d) = &c.dscsigner {
        d.set_property_from_str("c2pa-manifest-json", &build_manifest_json(enabled, &c.camera_name));
    }
    eprintln!(
        "\n>>> Anonymizer {} (live)",
        if enabled { "ON" } else { "OFF" }
    );
}

pub fn set_anonymizer_effect(controls: &Arc<Mutex<SourceControls>>, effect: u32) {
    let mut c = controls.lock().unwrap();
    c.effect = effect;
    if let Some(a) = &c.anonymizer {
        a.set_property_from_str("effect", effect_nick(effect));
    }
}

pub fn set_anonymizer_intensity(controls: &Arc<Mutex<SourceControls>>, intensity: f32) {
    let mut c = controls.lock().unwrap();
    c.effect_intensity = intensity;
    if let Some(a) = &c.anonymizer {
        a.set_property("effect-intensity", intensity);
    }
}

fn make_element(factory: &str, name: &str) -> Result<gst::Element> {
    gst::ElementFactory::make(factory)
        .name(name)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create {factory} element: {e}"))
}

fn make_whip_sink(whip_endpoint: &str) -> Result<gst::Element> {
    let sink = gst::ElementFactory::make("whipclientsink")
        .name("whipsink")
        .build()?;
    sink.dynamic_cast_ref::<gst::ChildProxy>()
        .ok_or_else(|| anyhow::anyhow!("whipclientsink missing ChildProxy"))?
        .set_child_property("signaller::whip-endpoint", whip_endpoint);
    Ok(sink)
}

pub fn run(
    whip_endpoint: &str,
    dsc: &DscConfig,
    running: Arc<AtomicBool>,
    controls: Arc<Mutex<SourceControls>>,
) -> Result<()> {
    // Try hardware VA-API first, then software x265, so the source can start
    // even when vah265enc is registered but stalls/fails at state change
    // (e.g. inside containers without a working GPU render node).
    let encoders: &[&str] = if dsc.software_encoder {
        &["x265enc"]
    } else {
        &["vah265enc", "x265enc"]
    };
    let mut last_err = None;
    for &encoder_factory in encoders {
        match run_with_encoder(whip_endpoint, dsc, &running, &controls, encoder_factory) {
            Ok(()) => return Ok(()),
            Err(e) => {
                eprintln!("Source with {encoder_factory} failed to start: {e}");
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("No H.265 encoder could start the source pipeline")))
}

fn run_with_encoder(
    whip_endpoint: &str,
    dsc: &DscConfig,
    running: &AtomicBool,
    controls: &Arc<Mutex<SourceControls>>,
    encoder_factory: &str,
) -> Result<()> {
    let pipeline = gst::Pipeline::builder().name("source-ingest").build();

    let device = dsc.camera_device.as_deref().unwrap_or("/dev/video0");
    let camera_name = detect_camera_name(device);
    println!("Using camera: {} ({})", device, camera_name);
    let videosrc = gst::ElementFactory::make("v4l2src")
        .name("videosrc")
        .property("device", device)
        .build()?;

    let videoconvert = make_element("videoconvert", "videoconvert")?;

    let audiotestsrc = gst::ElementFactory::make("audiotestsrc")
        .name("audiosrc")
        .property("is-live", true)
        .property_from_str("wave", "ticks")
        .build()?;

    let audiocaps = gst::ElementFactory::make("capsfilter")
        .name("audio-caps")
        .property(
            "caps",
            gst::Caps::builder("audio/x-raw")
                .field("channels", 2i32)
                .field("rate", 48000i32)
                .build(),
        )
        .build()?;

    let audioresample = make_element("audioresample", "audioresample")?;
    let audioconvert = make_element("audioconvert", "audioconvert")?;

    let whipsink = make_whip_sink(whip_endpoint)?;

    let _handler = whipsink.connect("payloader-setup", false, |args| {
        let payloader = args[3].get::<gst::Element>().unwrap();
        if payloader
            .factory()
            .map(|f| f.name().as_str() == "rtph265pay")
            .unwrap_or(false)
        {
            payloader.set_property_from_str("aggregate-mode", "zero-latency");
            return Some(true.to_value());
        }
        Some(false.to_value())
    });

    let encoder = make_element(encoder_factory, "encoder")?;
    // Low-latency tuning for live streaming.
    match encoder_factory {
        "x265enc" => {
            encoder.set_property_from_str("speed-preset", "ultrafast");
            encoder.set_property_from_str("tune", "zerolatency");
        }
        "vah265enc" => {
            encoder.set_property("target-usage", 1u32);
        }
        _ => {}
    }
    let h265parse = make_element("h265parse", "h265parse")?;
    let enc_queue = make_element("queue", "enc-queue")?;

    let dsc_caps = gst::ElementFactory::make("capsfilter")
        .name("dsc-caps")
        .property(
            "caps",
            gst::Caps::builder("video/x-h265")
                .field("stream-format", "hvc1")
                .field("alignment", "au")
                .build(),
        )
        .build()?;

    let dscsigner = make_element("dscsigner", "dscsigner")?;
    dscsigner.set_property("enable-c2pa", true);
    dscsigner.set_property_from_str(
        "c2pa-manifest-json",
        &build_manifest_json(dsc.demo_ai_filter, &camera_name),
    );
    dscsigner.set_property_from_str(
        "private-key-path",
        dsc.private_key_path.to_string_lossy().as_ref(),
    );
    dscsigner.set_property_from_str(
        "public-key-uri",
        &format!("file://{}", dsc.cert_path.display()),
    );
    dscsigner.set_property("substream-length", dsc.substream_length);
    dscsigner.set_property_from_str("hash-method", &dsc.hash_method);
    if let Some(ref template) = dsc.manifest_uri_template {
        dscsigner.set_property_from_str("c2pa-manifest-uri-template", template);
    }
    if let Some(ref uuid) = dsc.content_uuid {
        dscsigner.set_property_from_str("content-uuid", uuid);
    }

    let seiinserter = make_element("h265seiinserter", "seiinserter")?;

    // Optional AI face anonymization (flufaceanonymizer element from the
    // Fluendo anonymizer package). When the element is available we build a
    // tee + input-selector pipeline so the anonymizer can be toggled live
    // without dropping the encoder/WHIP sink (and thus the WebRTC signaller).
    let ai_available = gst::ElementFactory::find("flufaceanonymizer").is_some();

    if ai_available {
        let tee = make_element("tee", "tee")?;
        let selector = make_element("input-selector", "selector")?;
        let queue_raw = make_element("queue", "raw-queue")?;
        let valve_raw = make_element("valve", "raw-valve")?;
        let queue_ai = make_element("queue", "ai-queue")?;
        let valve_ai = make_element("valve", "ai-valve")?;
        let anonymizer = make_element("flufaceanonymizer", "anonymizer")?;
        anonymizer.set_property_from_str("effect", effect_nick(dsc.ai_effect));
        anonymizer.set_property("effect-intensity", dsc.ai_effect_intensity);
        anonymizer.set_property_from_str("model-path", &dsc.ai_model_path);
        let videoconvert2 = make_element("videoconvert", "videoconvert2")?;
        // Pin the anonymizer's output back to 4:2:0 so the encoder always emits
        // the same H.265 profile (main) whether or not the anonymizer is active.
        // Otherwise the anonymizer can output 4:4:4, making x265enc switch to
        // main-444, which would require a WebRTC renegotiation the sink does not
        // support.
        let ai_caps = gst::ElementFactory::make("capsfilter")
            .name("ai-caps")
            .property(
                "caps",
                gst::Caps::builder("video/x-raw").field("format", "I420").build(),
            )
            .build()?;
        let queue_ai_out = make_element("queue", "ai-out-queue")?;

        pipeline.add_many([
            &videosrc,
            &videoconvert,
            &tee,
            &queue_raw,
            &valve_raw,
            &queue_ai,
            &valve_ai,
            &anonymizer,
            &videoconvert2,
            &ai_caps,
            &queue_ai_out,
            &selector,
            &encoder,
            &enc_queue,
            &h265parse,
            &dsc_caps,
            &dscsigner,
            &seiinserter,
            &audiotestsrc,
            &audiocaps,
            &audioresample,
            &audioconvert,
            &whipsink,
        ])?;

        videosrc.link(&videoconvert)?;
        videoconvert.link(&tee)?;

        // Raw branch (bypasses the anonymizer).
        let tee_pad0 = tee
            .request_pad_simple("src_%u")
            .ok_or_else(|| anyhow::anyhow!("tee: failed to request src pad"))?;
        tee_pad0.link(&queue_raw.static_pad("sink").unwrap())?;
        queue_raw.link(&valve_raw)?;
        let selector_raw_pad = selector
            .request_pad_simple("sink_%u")
            .ok_or_else(|| anyhow::anyhow!("input-selector: failed to request sink pad"))?;
        valve_raw
            .static_pad("src")
            .unwrap()
            .link(&selector_raw_pad)?;

        // AI branch (through the anonymizer).
        let tee_pad1 = tee
            .request_pad_simple("src_%u")
            .ok_or_else(|| anyhow::anyhow!("tee: failed to request src pad"))?;
        tee_pad1.link(&queue_ai.static_pad("sink").unwrap())?;
        gst::Element::link_many([
            &queue_ai,
            &valve_ai,
            &anonymizer,
            &videoconvert2,
            &ai_caps,
            &queue_ai_out,
        ])?;
        let selector_ai_pad = selector
            .request_pad_simple("sink_%u")
            .ok_or_else(|| anyhow::anyhow!("input-selector: failed to request sink pad"))?;
        queue_ai_out
            .static_pad("src")
            .unwrap()
            .link(&selector_ai_pad)?;

        gst::Element::link_many([
            &selector,
            &encoder,
            &enc_queue,
            &h265parse,
            &dsc_caps,
            &dscsigner,
            &seiinserter,
            &whipsink,
        ])?;

        {
            let mut c = controls.lock().unwrap();
            c.available = true;
            c.enabled = dsc.demo_ai_filter;
            c.camera_name = camera_name.clone();
            c.selector = Some(selector);
            c.selector_raw_pad = Some(selector_raw_pad);
            c.selector_ai_pad = Some(selector_ai_pad);
            c.valve_raw = Some(valve_raw);
            c.valve_ai = Some(valve_ai);
            c.anonymizer = Some(anonymizer);
            c.dscsigner = Some(dscsigner.clone());
        }
        set_anonymizer_enabled(controls, dsc.demo_ai_filter);

        println!("WHIP source started with DSC signing (AI anonymizer available)");
    } else {
        pipeline.add_many([
            &videosrc,
            &videoconvert,
            &encoder,
            &enc_queue,
            &h265parse,
            &dsc_caps,
            &dscsigner,
            &seiinserter,
            &audiotestsrc,
            &audiocaps,
            &audioresample,
            &audioconvert,
            &whipsink,
        ])?;

        videosrc.link(&videoconvert)?;
        gst::Element::link_many([
            &videoconvert,
            &encoder,
            &enc_queue,
            &h265parse,
            &dsc_caps,
            &dscsigner,
            &seiinserter,
            &whipsink,
        ])?;

        {
            let mut c = controls.lock().unwrap();
            c.available = false;
            c.enabled = false;
            c.camera_name = camera_name.clone();
        }

        println!("WHIP source started with DSC signing");
    }

    gst::Element::link_many([
        &audiotestsrc,
        &audiocaps,
        &audioresample,
        &audioconvert,
        &whipsink,
    ])?;

    pipeline.set_state(gst::State::Playing)?;

    // Wait for the async state change to complete (or fail/stall).
    let (state_result, _, _) = pipeline.state(gst::ClockTime::from_seconds(8));
    match state_result {
        Ok(gst::StateChangeSuccess::Async) => {
            // Stall: tear down and wait so the camera/GPU are released before
            // the caller retries with a different encoder.
            let _ = pipeline.set_state(gst::State::Null);
            let _ = pipeline.state(gst::ClockTime::from_seconds(5));
            return Err(anyhow::anyhow!(
                "{encoder_factory} state change to Playing timed out"
            ));
        }
        Err(_) => {
            let _ = pipeline.set_state(gst::State::Null);
            let _ = pipeline.state(gst::ClockTime::from_seconds(5));
            return Err(anyhow::anyhow!(
                "{encoder_factory} failed to change state to Playing"
            ));
        }
        Ok(_) => {}
    }

    let bus = pipeline.bus().expect("Pipeline should have a bus");
    pipeline.debug_to_dot_file(gst::DebugGraphDetails::all(), "source-playing");
    while running.load(Ordering::SeqCst) {
        if let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(500)) {
            match msg.view() {
                gst::MessageView::Eos(..) => break,
                gst::MessageView::Error(err) => {
                    eprintln!(
                        "Source error: {} ({})",
                        err.error(),
                        err.debug().unwrap_or_default()
                    );
                    break;
                }
                _ => (),
            }
        }
    }

    pipeline.set_state(gst::State::Null)?;
    println!("Source stopped");
    Ok(())
}
