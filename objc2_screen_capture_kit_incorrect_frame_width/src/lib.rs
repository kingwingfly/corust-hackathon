#![allow(dead_code, unused)]

use std::sync::Arc;

use crossbeam_channel::Sender;
use objc2::{AnyThread as _, DefinedClass as _, define_class, msg_send, rc::Retained};
use objc2_core_media::CMSampleBuffer;
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
    CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVReturnSuccess,
};
use objc2_foundation::{NSObject, NSObjectProtocol};
use objc2_screen_capture_kit::{SCStream, SCStreamOutput, SCStreamOutputType};
use parking_lot::Mutex;

#[derive(Debug, Clone)]
pub enum Frame {
    /// Video frame
    Video {
        /// (Width, Height) of the video frame
        size: (u32, u32),
        // omit others
    },
    /// Audio frame
    Audio {
        // omit others
    },
}

/// A Delegate to handle CMSampleBuffer from SCStream
#[derive(Debug)]
pub(crate) struct StreamOutput {
    tx: Sender<Frame>,
    size: Arc<Mutex<(u32, u32)>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = StreamOutput]
    #[derive(Debug)]
    pub(crate) struct VideoStreamOutput;

    unsafe impl NSObjectProtocol for VideoStreamOutput {}

    #[allow(non_snake_case)]
    unsafe impl SCStreamOutput for VideoStreamOutput {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        unsafe fn stream_didOutputSampleBuffer_ofType(
            &self,
            _stream: &SCStream,
            sample_buffer: &CMSampleBuffer,
            r#type: SCStreamOutputType,
        ) {
            unsafe {
                if !sample_buffer.data_is_ready() {
                    return;
                }
                match r#type {
                    SCStreamOutputType::Screen => {
                        let Some(buffer) = sample_buffer.image_buffer() else {
                            return;
                        };
                        // lock buffer
                        if CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags::ReadOnly)
                            != kCVReturnSuccess
                        {
                            return;
                        }
                        // get bytes per row
                        let bytes_per_row = CVPixelBufferGetBytesPerRow(&buffer);

                        // `CVPixelBufferGetWidth` shouldn't be used, it returns unmatched width on M3 MacBook,
                        // since it's the result without padding.
                        #[cfg(feature = "incorrect")]
                        let width = CVPixelBufferGetWidth(&buffer);
                        #[cfg(not(feature = "incorrect"))]
                        let width = bytes_per_row / 4;

                        let height = CVPixelBufferGetHeight(&buffer);
                        let addr = CVPixelBufferGetBaseAddress(&buffer);
                        let vframe =
                            core::slice::from_raw_parts(addr as *const u8, bytes_per_row * height)
                                .to_vec();
                        CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags::ReadOnly);
                        *self.ivars().size.lock() = (width as u32, height as u32);
                        let _ = self.ivars().tx.try_send(Frame::Video {
                            size: (width as u32, height as u32),
                        });
                    }
                    SCStreamOutputType::Audio => {
                        // omit
                    }
                    SCStreamOutputType::Microphone => {}
                    _ => unreachable!(),
                }
            }
        }
    }
);

impl VideoStreamOutput {
    pub(crate) fn new(tx: Sender<Frame>, size: Arc<Mutex<(u32, u32)>>) -> Retained<Self> {
        unsafe {
            let this = Self::alloc().set_ivars(StreamOutput { tx, size });
            msg_send![super(this), init]
        }
    }
}
