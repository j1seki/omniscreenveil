//! Persistent Windows Desktop Duplication session; no GDI screenshots.
use veil_surface::{Frame, PixelFormat};
use windows_capture::{
    dxgi_duplication_api::{DxgiDuplicationApi, DxgiDuplicationFormat, Error},
    monitor::Monitor,
};
use windows_sys::Win32::System::Performance::QueryPerformanceCounter;

pub struct DesktopStream {
    duplication: DxgiDuplicationApi,
    scratch: Vec<u8>,
}

impl DesktopStream {
    pub fn open(display: &str) -> Result<Self, String> {
        let monitor = Monitor::enumerate()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|m| m.device_name().is_ok_and(|name| name == display))
            .ok_or("The selected Windows display is not available.")?;
        let duplication = DxgiDuplicationApi::new_options(monitor, &[DxgiDuplicationFormat::Bgra8])
            .map_err(|e| e.to_string())?;
        if duplication.duplication_desc().Rotation.0 != 1 {
            return Err(
                "Native streaming does not support rotated displays yet. Choose a snapshot mode."
                    .into(),
            );
        }
        Ok(Self {
            duplication,
            scratch: Vec::new(),
        })
    }

    /// Desktop Duplication uses QPC for desktop presents. Never mask a queued
    /// frame with an inventory sampled after that frame's pixels were drawn.
    pub fn clock() -> Result<i64, String> {
        let mut ticks = 0;
        if unsafe { QueryPerformanceCounter(&mut ticks) } == 0 {
            return Err("The Windows capture clock is unavailable.".into());
        }
        Ok(ticks)
    }

    pub fn next(&mut self, since: i64, size: (u32, u32)) -> Result<Option<Frame>, String> {
        let mut native = match self.duplication.acquire_next_frame(100) {
            Ok(frame) => frame,
            Err(Error::Timeout) => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        if native.frame_info().LastPresentTime < since {
            return Ok(None);
        }
        let (w, h) = (native.width(), native.height());
        if (w, h) != size || native.format() != DxgiDuplicationFormat::Bgra8 {
            return Err("Display geometry or pixel format changed; restart the stream.".into());
        }
        let buffer = native.buffer().map_err(|e| e.to_string())?;
        let pixels = buffer.as_nopadding_buffer(&mut self.scratch).to_vec();
        Frame::from_bytes(w, h, PixelFormat::Bgra8, pixels)
            .map(Some)
            .map_err(|e| e.to_string())
    }
}
