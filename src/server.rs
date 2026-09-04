use anyhow::Result;
use gst::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Shared control for the live DSC bitstream-tamper demo.
///
/// The tamper is implemented as a pad-probe that reinjects cached VPS/SPS/PPS
/// NAL units into every video access unit. This breaks the DSC hash of every
/// substream (a sustained, viewer-invisible tamper). While the switch is ON the
/// stream stays tampered continuously; switching it OFF restores a clean stream.
pub struct TamperControl {
    /// Whether the tamper is active (switch/toggle ON).
    pub enabled: Arc<AtomicBool>,
    /// Current phase: `true` = reinject (tampered), `false` = clean.
    pub phase: Arc<AtomicBool>,
    /// Cached VPS/SPS/PPS NAL bytes (hvc1 length-prefixed), extracted from the
    /// first IDR access unit.
    pub sps_pps: Arc<Mutex<Option<Vec<u8>>>>,
    /// Whether the control thread has been spawned (spawned once, idles while disabled).
    started: Arc<AtomicBool>,
    /// Whether a WHIP source peer is currently connected.
    pub source_connected: Arc<AtomicBool>,
    /// Whether a WHEP player peer is currently connected.
    pub player_connected: Arc<AtomicBool>,
    /// When the last video buffer flowed through the bridge.
    pub last_video: Arc<Mutex<Option<std::time::Instant>>>,
}

impl TamperControl {
    pub fn new() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            phase: Arc::new(AtomicBool::new(false)),
            sps_pps: Arc::new(Mutex::new(None)),
            started: Arc::new(AtomicBool::new(false)),
            source_connected: Arc::new(AtomicBool::new(false)),
            player_connected: Arc::new(AtomicBool::new(false)),
            last_video: Arc::new(Mutex::new(None)),
        }
    }
}

/// Enable the tamper and ensure the control thread is running. While enabled the
/// stream stays tampered continuously; `stop_tamper` restores a clean stream.
pub fn start_tamper_cycle(control: &Arc<TamperControl>) {
    if !control.started.swap(true, Ordering::SeqCst) {
        let control = control.clone();
        std::thread::spawn(move || loop {
            // Idle until enabled.
            while !control.enabled.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
            }
            // Tamper continuously while enabled.
            control.phase.store(true, Ordering::SeqCst);
            while control.enabled.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
            }
            control.phase.store(false, Ordering::SeqCst);
        });
    }
    control.enabled.store(true, Ordering::SeqCst);
}

/// Disable the tamper and reset to clean.
pub fn stop_tamper(control: &Arc<TamperControl>) {
    control.enabled.store(false, Ordering::SeqCst);
    control.phase.store(false, Ordering::SeqCst);
}

/// Toggle the tamper on/off; returns the new enabled state.
pub fn toggle_tamper(control: &Arc<TamperControl>) -> bool {
    if control.enabled.load(Ordering::SeqCst) {
        stop_tamper(control);
        false
    } else {
        start_tamper_cycle(control);
        true
    }
}

/// True if the NAL is a VPS (32), SPS (33) or PPS (34) — the non-VCL NAL
/// units that the DSC hash covers (unlike AUD/filler/SEI, which are excluded).
fn is_ps_nal(nal: &[u8]) -> bool {
    if nal.is_empty() {
        return false;
    }
    let nal_type = (nal[0] >> 1) & 0x3F;
    nal_type == 32 || nal_type == 33 || nal_type == 34
}

/// Extract VPS/SPS/PPS NAL units from an H.265 access unit, preserving the
/// input framing (byte-stream start codes or hvc1 length prefixes) so the
/// result can be prepended back onto buffers of the same format.
/// Returns `None` if no parameter-set NAL is present.
fn extract_vps_sps_pps(data: &[u8]) -> Option<Vec<u8>> {
    if data.starts_with(&[0, 0, 0, 1]) || data.starts_with(&[0, 0, 1]) {
        extract_vps_sps_pps_byte_stream(data)
    } else {
        extract_vps_sps_pps_length_prefixed(data, 4)
            .or_else(|| extract_vps_sps_pps_length_prefixed(data, 2))
    }
}

fn extract_vps_sps_pps_byte_stream(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut found = false;
    let mut start = 0usize;
    while start < data.len() {
        let start_code_len = if data[start..].starts_with(&[0, 0, 0, 1]) {
            4
        } else if data[start..].starts_with(&[0, 0, 1]) {
            3
        } else {
            break;
        };
        let nal_start = start + start_code_len;
        let mut end = nal_start;
        while end < data.len() {
            if (end + 4 <= data.len() && &data[end..end + 4] == &[0, 0, 0, 1])
                || (end + 3 <= data.len() && &data[end..end + 3] == &[0, 0, 1])
            {
                break;
            }
            end += 1;
        }
        if is_ps_nal(&data[nal_start..end]) {
            out.extend_from_slice(&data[start..end]);
            found = true;
        }
        start = end;
    }
    if found {
        Some(out)
    } else {
        None
    }
}

fn extract_vps_sps_pps_length_prefixed(data: &[u8], prefix_len: usize) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut found = false;
    let mut off = 0usize;
    while off + prefix_len <= data.len() {
        let len = if prefix_len == 4 {
            u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize
        } else {
            u16::from_be_bytes([data[off], data[off + 1]]) as usize
        };
        let nal_start = off + prefix_len;
        if len == 0 {
            off = nal_start;
            continue;
        }
        if nal_start + len > data.len() {
            break;
        }
        if is_ps_nal(&data[nal_start..nal_start + len]) {
            out.extend_from_slice(&data[off..nal_start + len]);
            found = true;
        }
        off = nal_start + len;
    }
    if found {
        Some(out)
    } else {
        None
    }
}

pub fn run(
    whip_addr: String,
    whep_addr: String,
    running: Arc<AtomicBool>,
    control: Arc<TamperControl>,
) -> Result<()> {
    let pipeline = gst::Pipeline::builder().name("server-bridge").build();

    let whipserversrc = gst::ElementFactory::make("whipserversrc")
        .name("whipsrc")
        .build()?;

    let whepserversink = gst::ElementFactory::make("whepserversink")
        .name("whepsink")
        .build()?;

    {
        let whip_signaller = whipserversrc
            .dynamic_cast_ref::<gst::ChildProxy>()
            .ok_or_else(|| anyhow::anyhow!("whipserversrc missing ChildProxy"))?
            .child_by_name("signaller")
            .ok_or_else(|| anyhow::anyhow!("whipserversrc missing 'signaller' child"))?;
        whip_signaller.set_property("host-addr", Some(whip_addr.as_str()));

        let source_connected = control.source_connected.clone();
        let _ = whip_signaller.connect("session-started", false, move |args| {
            let session_id = args.get(1).and_then(|v| v.get::<String>().ok());
            eprintln!("  -> WHIP source peer connected (session {session_id:?})");
            source_connected.store(true, Ordering::SeqCst);
            None
        });
    }
    {
        let whep_signaller = whepserversink
            .dynamic_cast_ref::<gst::ChildProxy>()
            .ok_or_else(|| anyhow::anyhow!("whepserversink missing ChildProxy"))?
            .child_by_name("signaller")
            .ok_or_else(|| anyhow::anyhow!("whepserversink missing 'signaller' child"))?;
        whep_signaller.set_property("host-addr", Some(whep_addr.as_str()));
    }

    // A WHEP player is a "consumer" of the whepserversink element. Track its
    // connection via the element-level signal, which is the same reliable
    // mechanism as the "payloader-setup" handler below.
    {
        let player_connected = control.player_connected.clone();
        let _ = whepserversink.connect("consumer-pipeline-created", false, move |args| {
            let peer_id = args.get(1).and_then(|v| v.get::<String>().ok());
            eprintln!("  -> WHEP player peer connected (peer {peer_id:?})");
            player_connected.store(true, Ordering::SeqCst);
            None
        });
    }

    pipeline.add_many([&whipserversrc, &whepserversink])?;

    let pipeline_weak = pipeline.downgrade();
    let wsink_weak = whepserversink.downgrade();
    let phase = control.phase.clone();
    let sps_pps = control.sps_pps.clone();
    let source_connected = control.source_connected.clone();
    let last_video = control.last_video.clone();
    whipserversrc.connect_pad_added(move |_src, pad| {
        let Some(pipeline) = pipeline_weak.upgrade() else {
            return;
        };
        let Some(wsink) = wsink_weak.upgrade() else {
            return;
        };

        let pad_name = pad.name().to_string();
        println!("  -> whipserversrc pad_added: {}", pad_name);

        let is_video = pad_name.contains("video");
        let is_audio = pad_name.contains("audio");

        if !is_video && !is_audio {
            return;
        }

        source_connected.store(true, Ordering::SeqCst);

        let q = gst::ElementFactory::make("queue")
            .name(format!("queue_{}", pad_name))
            .build()
            .unwrap();

        pipeline.add(&q).unwrap();
        q.sync_state_with_parent().unwrap();

        let qsink = q.static_pad("sink").unwrap();
        pad.link(&qsink).unwrap();

        let qsrc = q.static_pad("src").unwrap();
        let template = if is_video {
            "video_%u"
        } else {
            "audio_%u"
        };
        let wsink_pad = wsink.request_pad_simple(template).unwrap();
        qsrc.link(&wsink_pad).unwrap();

        println!(
            "  -> linked {} pad to whepserversink",
            if is_video { "video" } else { "audio" }
        );

        if is_video {
            // DSC tamper pad-probe: cache VPS/SPS/PPS and reinject them into
            // every access unit while in the tampered phase.
            let phase = phase.clone();
            let sps_pps = sps_pps.clone();
            let last_video = last_video.clone();
            let buffer_count = Arc::new(AtomicU64::new(0));
            qsrc.add_probe(gst::PadProbeType::BUFFER, move |_pad, info| {
                let n = buffer_count.fetch_add(1, Ordering::SeqCst);
                *last_video.lock().unwrap() = Some(std::time::Instant::now());
                // Cache VPS/SPS/PPS from the first access unit that carries them.
                {
                    let mut cache = sps_pps.lock().unwrap();
                    if cache.is_none() {
                        if let Some(buffer) = info.buffer() {
                            if let Ok(map) = buffer.map_readable() {
                                let data = map.as_slice();
                                if n == 0 {
                                    eprintln!(
                                        "[tamper] first video buffer {} bytes, head: {:02x?}",
                                        data.len(),
                                        &data[..data.len().min(16)]
                                    );
                                }
                                if let Some(nals) = extract_vps_sps_pps(data) {
                                    eprintln!(
                                        "[tamper] cached {} bytes of VPS/SPS/PPS",
                                        nals.len()
                                    );
                                    *cache = Some(nals);
                                }
                            }
                        }
                    }
                }

                // Reinject VPS/SPS/PPS while in the tampered phase.
                if phase.load(Ordering::SeqCst) {
                    let cache = sps_pps.lock().unwrap();
                    if let Some(ref nals) = *cache {
                        if let Some(buffer) = info.buffer_mut() {
                            let buf = buffer.make_mut();
                            buf.prepend_memory(gst::Memory::from_slice(nals.clone()));
                            if n % 150 == 0 {
                                eprintln!("[tamper] reinjecting {} bytes into buffer {}", nals.len(), n);
                            }
                        }
                    }
                }

                gst::PadProbeReturn::Ok
            });

            pipeline.debug_to_dot_file(gst::DebugGraphDetails::all(), "server-playing");
        }
    });

    println!("WHIP server listening on {}", whip_addr);
    println!("WHEP server will start once a WHIP stream arrives");

    // Skip config-interval (bitstream modifier that would break DSC by default)
    // and keep the low-latency aggregate mode.
    let _handler = whepserversink.connect("payloader-setup", false, move |args| {
        let payloader = args[3].get::<gst::Element>().unwrap();
        let is_rtph265pay = payloader
            .factory()
            .map(|f| f.name().as_str() == "rtph265pay")
            .unwrap_or(false);
        if is_rtph265pay {
            payloader.set_property_from_str("aggregate-mode", "zero-latency");
            return Some(true.to_value());
        }
        Some(false.to_value())
    });

    pipeline.set_state(gst::State::Playing)?;

    let bus = pipeline.bus().expect("Pipeline should have a bus");
    while running.load(Ordering::SeqCst) {
        if let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(200)) {
            match msg.view() {
                gst::MessageView::Eos(..) => {
                    println!("Server bridge EOS");
                    break;
                }
                gst::MessageView::Error(err) => {
                    eprintln!(
                        "Server bridge error: {} ({})",
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
    println!("Server bridge stopped");
    Ok(())
}
