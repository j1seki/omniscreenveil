//! Persistent Windows Desktop Duplication session; no GDI screenshots.
use veil_surface::{Frame, PixelFormat};
use windows_capture::{
    dxgi_duplication_api::{DxgiDuplicationApi, DxgiDuplicationFormat, Error},
    monitor::Monitor,
};
use windows_sys::Win32::System::Performance::QueryPerformanceCounter;

pub struct DesktopStream {
    duplication: DxgiDuplicationApi,
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
        Ok(Self { duplication })
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
        self.next_scaled(since, size, size)
    }

    pub fn next_scaled(
        &mut self,
        since: i64,
        size: (u32, u32),
        output: (u32, u32),
    ) -> Result<Option<Frame>, String> {
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
        let (width, height) = output;
        if width == 0 || height == 0 || width > w || height > h {
            return Err("Invalid native stream output size.".into());
        }
        let mut buffer = native.buffer().map_err(|e| e.to_string())?;
        let pitch = buffer.row_pitch() as usize;
        let pixels = buffer.as_raw_buffer();
        let mut frame = Frame::new(width, height, PixelFormat::Bgra8);
        for (y, row) in frame
            .pixels
            .chunks_exact_mut(width as usize * 4)
            .enumerate()
        {
            let from = y * h as usize / height as usize * pitch;
            if width == w {
                row.copy_from_slice(&pixels[from..from + row.len()]);
            } else {
                for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
                    let from = from + x * w as usize / width as usize * 4;
                    pixel.copy_from_slice(&pixels[from..from + 4]);
                }
            }
        }
        Ok(Some(frame))
    }
}
