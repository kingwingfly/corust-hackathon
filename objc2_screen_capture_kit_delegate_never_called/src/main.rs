#![allow(dead_code)]

use std::sync::{
    Arc,
    atomic::{AtomicPtr, Ordering},
};

use anyhow::Result;
use block2::RcBlock;
use crossbeam_utils::sync::{Parker, Unparker};
use objc2::{AnyThread as _, define_class, msg_send, rc::Retained, runtime::ProtocolObject};
use objc2_core_media::CMSampleBuffer;
use objc2_foundation::{NSArray, NSError, NSObject, NSObjectProtocol};
use objc2_screen_capture_kit::{
    SCContentFilter, SCShareableContent, SCStream, SCStreamConfiguration,
    SCStreamConfigurationPreset, SCStreamDelegate, SCStreamOutput, SCStreamOutputType,
};

fn main() {
    let config = CaptureConfig {};
    let unparker = config.create().expect("Failed to create capture");
    std::thread::sleep(std::time::Duration::from_secs(5));
    // this `unpark` resume screen capture running to the end,
    // so that things get dropped and then screen capture ends.
    unparker.unpark();
    std::thread::sleep(std::time::Duration::from_secs(1));
}

pub type Frame = Vec<u8>;

/// Configuration for the capture process.
#[derive(Debug)]
pub struct CaptureConfig {
    // omit
}

// impl SCStreamOutput Delegate for VideoStreamOutput.
//
// If it receivesd any frame, "stream_didOutputSampleBuffer_ofType" would be printed.
//
// In real implementation, `VideoStreamOutput` has `crossbeam_channel::Sender` as a field in ivar,
// so that data is send back to Rust by OS.
define_class!(
    #[unsafe(super(NSObject))]
    #[derive(Debug)]
    pub(crate) struct VideoStreamOutput;

    unsafe impl NSObjectProtocol for VideoStreamOutput {}

    #[allow(non_snake_case)]
    unsafe impl SCStreamOutput for VideoStreamOutput {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        unsafe fn stream_didOutputSampleBuffer_ofType(
            &self,
            _stream: &SCStream,
            _sample_buffer: &CMSampleBuffer,
            r#type: SCStreamOutputType,
        ) {
            if r#type != SCStreamOutputType::Screen {
                return;
            }
            println!("stream_didOutputSampleBuffer_ofType");
        }
    }
);

impl VideoStreamOutput {
    pub(crate) fn new() -> Retained<Self> {
        unsafe {
            let this = Self::alloc().set_ivars(());
            msg_send![super(this), init]
        }
    }
}

// impl SCStreamDelegate Delegate for StreamDelegate.
//
// It prints something on SCStream lifetime events.
define_class!(
    #[unsafe(super(NSObject))]
    #[derive(Debug)]
    pub(crate) struct StreamDelegate;

    unsafe impl NSObjectProtocol for StreamDelegate {}

    #[allow(non_snake_case)]
    unsafe impl SCStreamDelegate for StreamDelegate {
        #[unsafe(method(stream:didStopWithError:))]
        unsafe fn stream_didStopWithError(&self, stream: &SCStream, error: &NSError) {
            println!("{error}");
        }

        #[unsafe(method(outputVideoEffectDidStartForStream:))]
        unsafe fn outputVideoEffectDidStartForStream(&self, stream: &SCStream) {
            println!("outputVideoEffectDidStartForStream");
        }

        #[unsafe(method(outputVideoEffectDidStopForStream:))]
        unsafe fn outputVideoEffectDidStopForStream(&self, stream: &SCStream) {
            println!("outputVideoEffectDidStopForStream");
        }

        #[unsafe(method(streamDidBecomeActive:))]
        unsafe fn streamDidBecomeActive(&self, stream: &SCStream) {
            println!("streamDidBecomeActive");
        }

        #[unsafe(method(streamDidBecomeInactive:))]
        unsafe fn streamDidBecomeInactive(&self, stream: &SCStream) {
            println!("streamDidBecomeInactive");
        }
    }
);

impl StreamDelegate {
    pub(crate) fn new() -> Retained<Self> {
        unsafe {
            let this = Self::alloc().set_ivars(());
            msg_send![super(this), init]
        }
    }
}

/// This is incorrect implementation.
///
/// You would see macOS notifies screen capture started,
/// but nothing gets printed, i.e. neither `SCStreamDelegate` nor `SCStreamOutput` gets called.
#[cfg(feature = "incorrect")]
impl CaptureConfig {
    /// Spawns a thread to capture the screen and returns a `CaptureDesc` that can be used to control the capture.
    pub fn create(self) -> Result<Unparker> {
        let park = Parker::new();
        let unparker = park.unparker().clone();
        unsafe {
            // All jobs are trying to be done in `RcBlock`, it leads things not working as expected.
            SCShareableContent::getShareableContentWithCompletionHandler(&RcBlock::new(
                move |shareable: *mut SCShareableContent, e: *mut NSError| {
                    assert!(e.is_null(), "{}", &*e);
                    let display = (*shareable)
                        .displays()
                        .firstObject()
                        .expect("Primary display should exist");
                    let filter = SCContentFilter::initWithDisplay_excludingWindows(
                        SCContentFilter::alloc(),
                        &display,
                        &NSArray::new(),
                    );
                    let stream_config = SCStreamConfiguration::streamConfigurationWithPreset(
                        SCStreamConfigurationPreset::CaptureHDRStreamCanonicalDisplay,
                    );
                    stream_config.setWidth(display.width() as usize);
                    stream_config.setHeight(display.height() as usize);
                    stream_config.setCapturesAudio(false);
                    stream_config.setCaptureMicrophone(false);
                    stream_config.setPixelFormat(u32::from_be_bytes(*b"BGRA"));

                    // get a stream to capture screen
                    let stream = SCStream::initWithFilter_configuration_delegate(
                        SCStream::alloc(),
                        &filter,
                        &stream_config,
                        Some(&ProtocolObject::from_retained(StreamDelegate::new())),
                    );
                    // add sample handler `VideoStreamOutput` to stream
                    stream
                        .addStreamOutput_type_sampleHandlerQueue_error(
                            &ProtocolObject::from_retained(VideoStreamOutput::new()),
                            SCStreamOutputType::Screen,
                            None,
                        )
                        .unwrap();

                    // start capture with error handler
                    stream.startCaptureWithCompletionHandler(Some(&RcBlock::new(
                        |e: *mut NSError| {
                            if e.is_null() {
                                return;
                            }
                            println!("{}", &*e);
                        },
                    )));
                    println!("started");
                    // park here, and screen capture is drived by OS,
                    park.park();
                    println!("stopping");
                    // stop capturing
                    stream.stopCaptureWithCompletionHandler(None);
                },
            ));
        }
        Ok(unparker)
    }
}

// Things got fixed if just got SCDisplay from the RcBlock:
// i.e. instead of running all logic in RcBlock, after getting SCDisplay from RcBlock,
// run retained logic outside of RcBlock.
//
// With this implementation, messages are printed and samples are handled as expected.
#[cfg(not(feature = "incorrect"))]
impl CaptureConfig {
    /// Spawns a thread to capture the screen and returns a `CaptureDesc` that can be used to control the capture.
    pub fn create(self) -> Result<Unparker> {
        let park = Parker::new();
        let unparker = park.unparker().clone();
        unsafe {
            // AtomicPtr has internal mutability, and RcBlock expects Fn instead of FnMut
            let display = Arc::new(AtomicPtr::new(core::ptr::null_mut()));
            let block = {
                let display = display.clone();
                RcBlock::new(move |shareable: *mut SCShareableContent, e: *mut NSError| {
                    assert!(e.is_null(), "{}", &*e);
                    // store SCDisplay in AtomicPtr
                    display.store(
                        Box::into_raw(Box::new(
                            (*shareable)
                                .displays()
                                .firstObject()
                                .expect("Primary display should exist"),
                        )),
                        Ordering::Relaxed,
                    );
                })
            };
            SCShareableContent::getShareableContentWithCompletionHandler(&block);
            // busy loop to get SCDisplay
            let display = loop {
                let display = display.load(Ordering::Relaxed);
                if !display.is_null() {
                    break *Box::from_raw(display);
                }
            };
            // things next are not changed
            let filter = SCContentFilter::initWithDisplay_excludingWindows(
                SCContentFilter::alloc(),
                &display,
                &NSArray::new(),
            );
            let stream_config = SCStreamConfiguration::streamConfigurationWithPreset(
                SCStreamConfigurationPreset::CaptureHDRStreamCanonicalDisplay,
            );
            stream_config.setWidth(display.width() as usize);
            stream_config.setHeight(display.height() as usize);
            stream_config.setCapturesAudio(false);
            stream_config.setCaptureMicrophone(false);
            stream_config.setPixelFormat(u32::from_be_bytes(*b"BGRA"));

            let stream = SCStream::initWithFilter_configuration_delegate(
                SCStream::alloc(),
                &filter,
                &stream_config,
                Some(&ProtocolObject::from_retained(StreamDelegate::new())),
            );
            stream
                .addStreamOutput_type_sampleHandlerQueue_error(
                    &ProtocolObject::from_retained(VideoStreamOutput::new()),
                    SCStreamOutputType::Screen,
                    None,
                )
                .unwrap();

            stream.startCaptureWithCompletionHandler(Some(&RcBlock::new(|e: *mut NSError| {
                if e.is_null() {
                    return;
                }
                println!("{}", &*e);
            })));
            println!("started");
            park.park();
            println!("stopping");
            stream.stopCaptureWithCompletionHandler(None);
        }
        Ok(unparker)
    }
}
