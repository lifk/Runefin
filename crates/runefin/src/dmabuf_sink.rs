//! A minimal GStreamer sink that keeps frames in DMA-BUF memory and publishes them to Rune.
//!
//! `appsink` cannot accept the `DMA_DRM` buffer layout `vapostproc` exports: `vapostproc` requires
//! a downstream allocation query that supports `GstVideoMeta`, which appsink does not answer. A
//! `BaseSink` gives us the pad template, the `VideoMeta`, and the buffer lifetime without mapping
//! a byte.

use std::os::fd::OwnedFd;
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
use rune::native_surface::dmabuf::{DmabufFrame, Lease, Plane};
use rune::native_surface::{ColorInfo, Format, Matrix, Transfer};

struct DrmCaps {
    width: u32,
    height: u32,
    format: Format,
    modifier: u64,
    color: ColorInfo,
}

struct BufferLease {
    _buffer: gst::Buffer,
}

impl std::fmt::Debug for BufferLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferLease").finish_non_exhaustive()
    }
}

impl Lease for BufferLease {
    fn presented(&mut self) {}

    fn release(self: Box<Self>, _release_fence: Option<OwnedFd>) {
        // Dropping the buffer returns it to GStreamer's pool. Rune releases the lease only after
        // the GPU work that sampled it has completed.
    }
}

#[derive(Default)]
pub struct Imp {
    surface: Mutex<Option<NativeSurface>>,
    caps: Mutex<Option<DrmCaps>>,
    eos: Mutex<Arc<AtomicBool>>,
    error: Mutex<Arc<Mutex<Option<String>>>>,
    frame_id: Mutex<Arc<AtomicU64>>,
    format_label: Mutex<Arc<Mutex<Option<&'static str>>>>,
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
        let format = DmabufFrame::from_fourcc(fourcc)
            .ok_or_else(|| gst::loggable_error!(gst::CAT_RUST, "unsupported DRM format {drm}"))?;
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
            *label = Some(match (format, color.transfer) {
                (Format::P010Video | Format::P010Full, Transfer::Pq) => "P010 PQ",
                (Format::P010Video | Format::P010Full, Transfer::Hlg) => "P010 HLG",
                (Format::P010Video | Format::P010Full, _) => "P010",
                (Format::Nv12Video | Format::Nv12Full, _) => "NV12",
                (Format::Bgra, _) => "BGRA",
            });
        }
        *self
            .caps
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(DrmCaps {
            width: info.width(),
            height: info.height(),
            format,
            modifier,
            color,
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
                format: caps.format,
                modifier: caps.modifier,
                color: caps.color,
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
        let mut imported = Vec::with_capacity(planes);
        for index in 0..planes {
            let stride = meta.stride()[index];
            if stride <= 0 {
                return Err(gst::FlowError::Error);
            }
            let memory = buffer.peek_memory(index.min(buffer.n_memory() - 1));
            let dmabuf = memory
                .downcast_memory_ref::<DmaBufMemory>()
                .ok_or(gst::FlowError::NotNegotiated)?;
            let plane = Plane::from_raw_fd(dmabuf.fd(), meta.offset()[index] as u64, stride as u64)
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
            imported.push(plane);
        }
        let frame = DmabufFrame::new(
            caps.width,
            caps.height,
            caps.format,
            caps.color,
            caps.modifier,
            imported,
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
        })?
        .with_lease(Box::new(BufferLease {
            _buffer: buffer.clone(),
        }));
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
