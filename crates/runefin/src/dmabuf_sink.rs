//! A minimal GStreamer sink that keeps frames in DMA-BUF memory and publishes them to Rune.
//!
//! `appsink` cannot accept the `DMA_DRM` buffer layout `vapostproc` exports: `vapostproc` requires
//! a downstream allocation query that supports `GstVideoMeta`, which appsink does not answer. A
//! `BaseSink` gives us the pad template, the `VideoMeta`, and the buffer lifetime without mapping
//! a byte.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use gstreamer as gst;
use gstreamer::glib;
use gstreamer::glib::subclass::prelude::*;
use gstreamer::subclass::ElementMetadata;
use gstreamer::subclass::prelude::*;
use gstreamer_allocators::DmaBufMemory;
use gstreamer_base::BaseSink;
use gstreamer_base::subclass::prelude::*;
use gstreamer_video::{VideoInfo, VideoMeta};
use rune::NativeSurface;

use crate::playback_trace::Trace;
use crate::player::ColourFacts;
use rune::native_surface::linux::{Dmabuf, DmabufBuffer, DmabufPlane};
use rune::native_surface::{ColorInfo, Matrix, Transfer};

struct DrmCaps {
    width: u32,
    height: u32,
    /// `DRM_FORMAT_ARGB8888`: this Rune imports linear BGRA dmabufs only.
    fourcc: u32,
    modifier: u64,
}

#[derive(Default)]
pub struct Imp {
    surface: Mutex<Option<NativeSurface>>,
    caps: Mutex<Option<DrmCaps>>,
    eos: Mutex<Arc<AtomicBool>>,
    error: Mutex<Arc<Mutex<Option<String>>>>,
    frame_id: Mutex<Arc<AtomicU64>>,
    format_label: Mutex<Arc<Mutex<Option<&'static str>>>>,
    trace: Mutex<Arc<Trace>>,
    facts: Mutex<Arc<std::sync::Mutex<Option<ColourFacts>>>>,
}

#[glib::object_subclass]
impl ObjectSubclass for Imp {
    const NAME: &'static str = "RunefinDmabufSink";
    type Type = DmabufSink;
    type ParentType = BaseSink;
    type Interfaces = ();

    fn new() -> Self {
        Self::default()
    }
}

impl ObjectImpl for Imp {}
impl GstObjectImpl for Imp {}
impl ElementImpl for Imp {
    fn metadata() -> Option<&'static ElementMetadata> {
        static META: std::sync::OnceLock<ElementMetadata> = std::sync::OnceLock::new();
        Some(META.get_or_init(|| {
            ElementMetadata::new(
                "Runefin DMA-BUF sink",
                "Sink/Video",
                "Imports DMA-BUF frames into a Rune NativeSurface",
                "Runefin",
            )
        }))
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static TEMPLATES: std::sync::OnceLock<Vec<gst::PadTemplate>> = std::sync::OnceLock::new();
        TEMPLATES.get_or_init(|| {
            let caps = gst::Caps::builder("video/x-raw")
                .features(["memory:DMABuf"])
                .field("format", "DMA_DRM")
                .build();
            vec![
                gst::PadTemplate::new(
                    "sink",
                    gst::PadDirection::Sink,
                    gst::PadPresence::Always,
                    &caps,
                )
                .expect("DMA-BUF sink pad template"),
            ]
        })
    }
}

impl BaseSinkImpl for Imp {
    fn set_caps(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let structure = caps
            .structure(0)
            .ok_or_else(|| gst::loggable_error!(gst::CAT_RUST, "caps have no structure"))?;
        let drm = structure
            .get::<&str>("drm-format")
            .map_err(|_| gst::loggable_error!(gst::CAT_RUST, "caps have no drm-format"))?;
        let (fourcc, modifier) = parse_drm_format(drm)
            .map_err(|message| gst::loggable_error!(gst::CAT_RUST, "{}", message))?;
        // Rune's dmabuf path imports one linear BGRA8 plane; a P010/NV12 surface would need the
        // colour pipeline this port has not restored yet (see docs/plans).
        const DRM_FORMAT_ARGB8888: u32 = u32::from_le_bytes(*b"AR24");
        if fourcc != DRM_FORMAT_ARGB8888 {
            return Err(gst::loggable_error!(
                gst::CAT_RUST,
                "unsupported DRM format {drm}: expected BGRA8 (ARGB8888) dmabufs"
            ));
        }
        let info = VideoInfo::from_caps(caps)
            .map_err(|error| gst::loggable_error!(gst::CAT_RUST, "{}", error))?;
        let colorimetry = info.colorimetry();
        let color = ColorInfo {
            matrix: match colorimetry.matrix() {
                gstreamer_video::VideoColorMatrix::Bt601 => Matrix::Bt601,
                gstreamer_video::VideoColorMatrix::Bt2020 => Matrix::Bt2020,
                _ => Matrix::Bt709,
            },
            transfer: match colorimetry.transfer() {
                gstreamer_video::VideoTransferFunction::Srgb => Transfer::Srgb,
                gstreamer_video::VideoTransferFunction::Smpte2084 => Transfer::Pq,
                gstreamer_video::VideoTransferFunction::AribStdB67 => Transfer::Hlg,
                _ => Transfer::Bt709,
            },
        };
        if let Ok(mut label) = self
            .format_label
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .lock()
        {
            *label = Some("BGRA");
        }
        // Static HDR metadata lives in the caps. Keep it for the Statistics panel and for the
        // tone-map work that follows; absent boxes leave zeroes.
        let mastering = gstreamer_video::VideoMasteringDisplayInfo::from_caps(caps).ok();
        let light = gstreamer_video::VideoContentLightLevel::from_caps(caps).ok();
        let facts = ColourFacts {
            transfer: match color.transfer {
                Transfer::Pq => "PQ",
                Transfer::Hlg => "HLG",
                Transfer::Srgb => "sRGB",
                _ => "BT.709",
            },
            matrix: match color.matrix {
                Matrix::Bt2020 => "BT.2020",
                Matrix::Bt601 => "BT.601",
                _ => "BT.709",
            },
            max_luminance: mastering.as_ref().map_or(0.0, |m| {
                m.max_display_mastering_luminance() as f32 / 10_000.0
            }),
            min_luminance: mastering.as_ref().map_or(0.0, |m| {
                m.min_display_mastering_luminance() as f32 / 10_000.0
            }),
            max_cll: light.as_ref().map_or(0, |l| l.max_content_light_level()),
            max_fall: light
                .as_ref()
                .map_or(0, |l| l.max_frame_average_light_level()),
        };
        if let Ok(slot) = self.facts.lock()
            && let Ok(mut slot) = slot.lock()
        {
            *slot = Some(facts);
        }
        *self
            .caps
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(DrmCaps {
            width: info.width(),
            height: info.height(),
            fourcc,
            modifier,
        });
        Ok(())
    }

    fn propose_allocation(
        &self,
        query: &mut gst::query::Allocation,
    ) -> Result<(), gst::LoggableError> {
        query.add_allocation_meta::<VideoMeta>(None);
        self.parent_propose_allocation(query)
    }

    fn render(&self, buffer: &gst::Buffer) -> Result<gst::FlowSuccess, gst::FlowError> {
        let Some(surface) = self
            .surface
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
        else {
            return Err(gst::FlowError::Error);
        };
        let Some(caps) = self
            .caps
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .as_ref()
            .map(|caps| DrmCaps {
                width: caps.width,
                height: caps.height,
                fourcc: caps.fourcc,
                modifier: caps.modifier,
            })
        else {
            return Err(gst::FlowError::NotNegotiated);
        };
        let meta = buffer
            .meta::<VideoMeta>()
            .ok_or(gst::FlowError::NotNegotiated)?;
        let planes = meta.n_planes() as usize;
        if planes == 0 {
            return Err(gst::FlowError::Error);
        }
        if planes != 1 {
            return Err(gst::FlowError::NotNegotiated);
        }
        let stride = meta.stride()[0];
        if stride <= 0 {
            return Err(gst::FlowError::Error);
        }
        let memory = buffer.peek_memory(0);
        let dmabuf = memory
            .downcast_memory_ref::<DmaBufMemory>()
            .ok_or(gst::FlowError::NotNegotiated)?;
        let plane = DmabufPlane::from_raw_fd(
            dmabuf.fd(),
            meta.offset()[0] as u32,
            stride as u32,
            caps.modifier,
        )
        .map_err(|message| {
            if let Ok(mut error) = self
                .error
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .lock()
            {
                *error = Some(message);
            }
            gst::FlowError::Error
        })?;
        let image = DmabufBuffer::new(plane, [caps.width, caps.height], caps.fourcc).map_err(
            |message| {
                if let Ok(mut error) = self
                    .error
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .lock()
                {
                    *error = Some(message);
                }
                gst::FlowError::Error
            },
        )?;
        // Holding the GStreamer buffer until Rune has read the frame keeps its pool from reusing
        // the memory; the callback runs on Rune's thread after the GPU copy.
        let hold = buffer.clone();
        let frame = Dmabuf::new(std::sync::Arc::new(image)).on_read(move |_| drop(hold));
        let id = self
            .frame_id
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .fetch_add(1, Ordering::Relaxed);
        surface.push_dmabuf(frame, id).map_err(|message| {
            if let Ok(mut error) = self
                .error
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .lock()
            {
                *error = Some(message);
            }
            gst::FlowError::Error
        })?;
        if let Ok(trace) = self.trace.lock() {
            trace.tick();
        }
        Ok(gst::FlowSuccess::Ok)
    }

    fn event(&self, event: gst::Event) -> bool {
        if matches!(event.view(), gst::EventView::Eos(_)) {
            self.eos
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .store(true, Ordering::Relaxed);
        }
        self.parent_event(event)
    }
}

fn parse_drm_format(value: &str) -> Result<(u32, u64), String> {
    let (fourcc, modifier) = value.split_once(':').unwrap_or((value, "0"));
    let code = match fourcc {
        "AR24" => u32::from_le_bytes(*b"AR24"),
        "XR24" => u32::from_le_bytes(*b"XR24"),
        "NV12" => u32::from_le_bytes(*b"NV12"),
        "P010" => u32::from_le_bytes(*b"P010"),
        _ => return Err(format!("unsupported DRM fourcc {fourcc}")),
    };
    let modifier = match modifier.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => modifier.parse().ok(),
    }
    .ok_or_else(|| format!("bad DRM modifier {modifier}"))?;
    Ok((code, modifier))
}

glib::wrapper! {
    pub struct DmabufSink(ObjectSubclass<Imp>) @extends BaseSink, gst::Element, gst::Object;
}

impl DmabufSink {
    pub fn new(
        surface: NativeSurface,
        eos: Arc<AtomicBool>,
        error: Arc<Mutex<Option<String>>>,
        frame_id: Arc<AtomicU64>,
        format_label: Arc<Mutex<Option<&'static str>>>,
        trace: Arc<Trace>,
        facts: Arc<std::sync::Mutex<Option<ColourFacts>>>,
    ) -> Self {
        let sink: Self = glib::Object::builder().build();
        let imp = sink.imp();
        *imp.surface
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(surface);
        *imp.eos.lock().unwrap_or_else(|poison| poison.into_inner()) = eos;
        *imp.error
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = error;
        *imp.frame_id
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = frame_id;
        *imp.format_label
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = format_label;
        *imp.trace
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = trace;
        *imp.facts
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = facts;
        sink
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drm_format_parses_the_fourcc_and_modifier() {
        let (code, modifier) = parse_drm_format("AR24:0x0200000018601b04").expect("parse");
        assert_eq!(code, u32::from_le_bytes(*b"AR24"));
        assert_eq!(modifier, 0x0200_0000_1860_1b04);
        let (code, modifier) = parse_drm_format("XR24").expect("linear");
        assert_eq!(code, u32::from_le_bytes(*b"XR24"));
        assert_eq!(modifier, 0);
        let (code, _) = parse_drm_format("NV12:0x1").expect("nv12");
        assert_eq!(code, u32::from_le_bytes(*b"NV12"));
        let (code, _) = parse_drm_format("P010:0x1").expect("p010");
        assert_eq!(code, u32::from_le_bytes(*b"P010"));
    }
}
