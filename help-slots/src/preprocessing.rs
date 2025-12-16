use image::{GrayImage, ImageBuffer, Luma, RgbaImage};
use imageproc::edges::canny;
use imageproc::filter::gaussian_blur_f32;

/// Preprocessing pipeline configuration
#[derive(Debug, Clone)]
pub struct PreprocessConfig {
    /// Sigma for Gaussian blur before edge detection (0 = no blur)
    pub blur_sigma: f32,
    /// Low threshold for Canny edge detection
    pub canny_low: f32,
    /// High threshold for Canny edge detection
    pub canny_high: f32,
}

impl Default for PreprocessConfig {
    fn default() -> Self {
        Self {
            blur_sigma: 1.4,
            canny_low: 50.0,
            canny_high: 150.0,
        }
    }
}

/// Preprocessing pipeline for extracting stable features from game screenshots
pub struct Preprocessor {
    config: PreprocessConfig,
}

impl Preprocessor {
    pub fn new(config: PreprocessConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(PreprocessConfig::default())
    }

    /// Full preprocessing pipeline: RGBA -> Grayscale -> (optional blur) -> Canny edges
    pub fn process(&self, image: &RgbaImage) -> GrayImage {
        let gray = self.to_grayscale(image);
        let blurred = if self.config.blur_sigma > 0.0 {
            self.gaussian_blur(&gray)
        } else {
            gray
        };
        self.edge_detect(&blurred)
    }

    /// Convert RGBA image to grayscale
    pub fn to_grayscale(&self, image: &RgbaImage) -> GrayImage {
        let (width, height) = image.dimensions();
        let mut gray: GrayImage = ImageBuffer::new(width, height);

        for (x, y, pixel) in image.enumerate_pixels() {
            // Standard luminance weights: 0.299R + 0.587G + 0.114B
            let r = pixel[0] as f32;
            let g = pixel[1] as f32;
            let b = pixel[2] as f32;
            let luma = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
            gray.put_pixel(x, y, Luma([luma]));
        }

        gray
    }

    /// Apply Gaussian blur to reduce noise
    pub fn gaussian_blur(&self, image: &GrayImage) -> GrayImage {
        gaussian_blur_f32(image, self.config.blur_sigma)
    }

    /// Apply Canny edge detection
    pub fn edge_detect(&self, image: &GrayImage) -> GrayImage {
        canny(image, self.config.canny_low, self.config.canny_high)
    }

    /// Process and return both grayscale and edge images (for debugging)
    pub fn process_with_intermediates(&self, image: &RgbaImage) -> ProcessingResult {
        let gray = self.to_grayscale(image);
        let blurred = if self.config.blur_sigma > 0.0 {
            self.gaussian_blur(&gray)
        } else {
            gray.clone()
        };
        let edges = self.edge_detect(&blurred);

        ProcessingResult {
            grayscale: gray,
            blurred,
            edges,
        }
    }
}

/// Result containing intermediate processing stages for debugging
pub struct ProcessingResult {
    pub grayscale: GrayImage,
    pub blurred: GrayImage,
    pub edges: GrayImage,
}

impl ProcessingResult {
    /// Save all stages to disk for debugging
    pub fn save_debug(&self, prefix: &str) -> anyhow::Result<()> {
        self.grayscale.save(format!("{}_1_gray.png", prefix))?;
        self.blurred.save(format!("{}_2_blurred.png", prefix))?;
        self.edges.save(format!("{}_3_edges.png", prefix))?;
        Ok(())
    }
}

/// Template matching on edge-detected images
pub fn template_match(
    image: &GrayImage,
    template: &GrayImage,
) -> Option<(u32, u32, f32)> {
    use imageproc::template_matching::{match_template, MatchTemplateMethod};

    let (img_w, img_h) = image.dimensions();
    let (tpl_w, tpl_h) = template.dimensions();

    if tpl_w > img_w || tpl_h > img_h {
        return None;
    }

    let result = match_template(
        image,
        template,
        MatchTemplateMethod::CrossCorrelationNormalized,
    );

    // Find the maximum correlation
    let mut max_val = f32::MIN;
    let mut max_loc = (0u32, 0u32);

    for (x, y, pixel) in result.enumerate_pixels() {
        let val = pixel[0];
        if val > max_val {
            max_val = val;
            max_loc = (x, y);
        }
    }

    Some((max_loc.0, max_loc.1, max_val))
}

/// Check if a template match exceeds a confidence threshold
pub fn is_match(confidence: f32, threshold: f32) -> bool {
    confidence >= threshold
}
