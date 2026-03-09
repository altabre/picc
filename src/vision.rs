//! Vision framework bindings via objc2-vision
//!
//! https://developer.apple.com/documentation/vision?language=objc

pub use objc2_vision::{
    VNImageRequestHandler, VNRecognizeTextRequest, VNRecognizedText, VNRecognizedTextObservation,
    VNRequest,
};

/// A single OCR result: recognized text + normalized bounding box.
///
/// The bounding box uses Vision's coordinate system:
/// - origin (0,0) is at the **bottom-left** of the image
/// - values are normalized 0.0–1.0 relative to image dimensions
///
/// To convert to screen coordinates given a window at (win_x, win_y, win_w, win_h):
/// ```
/// let cx = win_x + (bbox.x + bbox.w / 2.0) * win_w;
/// let cy = win_y + (1.0 - bbox.y - bbox.h / 2.0) * win_h;
/// ```
#[derive(Debug, Clone)]
pub struct OcrResult {
    pub text: String,
    /// Normalized x (0–1, left edge)
    pub x: f64,
    /// Normalized y (0–1, bottom edge in Vision coords)
    pub y: f64,
    /// Normalized width
    pub w: f64,
    /// Normalized height
    pub h: f64,
    /// Confidence 0–1
    pub confidence: f64,
}

/// Run OCR on a CGImage and return all recognized text with bounding boxes.
pub fn ocr_with_boxes(image: &CGImage) -> Vec<OcrResult> {
    use objc2_foundation::{NSArray, NSString};

    let text_req = VNRecognizeTextRequest::new();
    let zh = NSString::from_str("zh-Hans");
    let en = NSString::from_str("en-US");
    let lang = NSArray::from_slice(&[&*zh, &*en]);
    text_req.setRecognitionLanguages(&lang);

    let text_req_ref: &VNRequest =
        unsafe { &*((&*text_req) as *const _ as *const VNRequest) };
    let reqs = NSArray::from_slice(&[text_req_ref]);
    let handler = new_handler_with_cgimage(image);

    if perform_requests(&handler, &reqs).is_err() {
        return Vec::new();
    }

    let mut results = Vec::new();
    if let Some(observations) = text_req.results() {
        for obs in observations.iter() {
            let candidates = obs.topCandidates(1);
            if let Some(candidate) = candidates.iter().next() {
                let text = candidate.string().to_string();
                let confidence = candidate.confidence() as f64;
                // boundingBox is inherited from VNDetectedObjectObservation.
                // Use raw ObjC msg_send since the feature isn't exposed separately.
                let bbox: objc2_core_foundation::CGRect =
                    unsafe { objc2::msg_send![&*obs, boundingBox] };
                results.push(OcrResult {
                    text,
                    x: bbox.origin.x,
                    y: bbox.origin.y,
                    w: bbox.size.width,
                    h: bbox.size.height,
                    confidence,
                });
            }
        }
    }
    results
}

use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_core_graphics::CGImage;
use objc2_foundation::{NSArray, NSError, NSURL};
use objc2_vision::VNImageOption;
use objc2::runtime::AnyObject;

pub fn new_handler_with_cgimage(
    image: &CGImage,
) -> Retained<VNImageRequestHandler> {
    let options = objc2_foundation::NSDictionary::<VNImageOption, AnyObject>::new();
    unsafe {
        VNImageRequestHandler::initWithCGImage_options(
            VNImageRequestHandler::alloc(),
            image,
            &options,
        )
    }
}

pub fn new_handler_with_url(url: &NSURL) -> Retained<VNImageRequestHandler> {
    let options = objc2_foundation::NSDictionary::<VNImageOption, AnyObject>::new();
    unsafe {
        VNImageRequestHandler::initWithURL_options(VNImageRequestHandler::alloc(), url, &options)
    }
}

pub fn perform_requests(
    handler: &VNImageRequestHandler,
    requests: &NSArray<VNRequest>,
) -> Result<(), Retained<NSError>> {
    handler.performRequests_error(requests)
}
