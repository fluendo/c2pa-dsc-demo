use crate::cert::DscConfig;
use anyhow::Result;
use gst::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const DIGITAL_CAPTURE: &str =
    "http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture";
const TRAINED_ALGORITHMIC_MEDIA: &str =
    "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia";

fn build_manifest_json(ai_filter: bool) -> String {
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
    serde_json::json!({
        "title": "Live Demo",
        "vendor": "fluendo",
        "claim_generator_info": [{"name": "fluendo dscsigner", "version": "1.0"}],
        "ingredients": [{"title": "Camera", "format": "video/x-raw", "relationship": "componentOf"}],
        "assertions": [{"label": "c2pa.actions", "data": {"actions": actions}}],
    })
    .to_string()
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

pub fn run(whip_endpoint: &str, dsc: &DscConfig, running: Arc<AtomicBool>) -> Result<()> {
    let pipeline = gst::Pipeline::builder().name("source-ingest").build();

    let device = dsc.camera_device.as_deref().unwrap_or("/dev/video0");
    println!("Using camera: {}", device);
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

    let encoder = make_element("vah265enc", "encoder")
        .or_else(|_| make_element("x265enc", "encoder"))?;
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
        &build_manifest_json(dsc.demo_ai_filter),
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
    gst::Element::link_many([
        &audiotestsrc,
        &audiocaps,
        &audioresample,
        &audioconvert,
        &whipsink,
    ])?;

    println!("WHIP source started with DSC signing");

    pipeline.set_state(gst::State::Playing)?;

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
