//! OpenRouter image generation and model listing.
//!
//! Generation uses the dedicated Images API (`POST /api/v1/images`), which
//! returns base64 image data plus the actual cost. Multiple copies are made
//! with concurrent requests because many models (including the default
//! Gemini image model) reject the `n` parameter for more than one image.
//! See <https://openrouter.ai/docs/guides/overview/multimodal/image-generation>.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Resolution tiers in ascending order, used to pick the nearest supported
/// tier when a model doesn't offer the requested one
const RESOLUTION_TIERS: [&str; 4] = ["512", "1K", "2K", "4K"];

/// OpenRouter expresses per-image prices in the models list as per-token
/// values, normalized by this nominal token count per image. Multiplying by
/// it recovers the exact per-image price for per-image-billed models and a
/// full-quality estimate for token-billed ones.
const TOKENS_PER_IMAGE_NOMINAL: f64 = 4175.0;

/// Settings for one generation request
#[derive(Debug, Clone)]
pub struct GenerationSettings {
    pub model: String,
    /// "low", "medium", "high", or "auto"
    pub quality: String,
    /// "512", "1K"/"2K"/"4K", a single dimension, or "1024x768" style
    pub size: String,
    pub count: u32,
}

/// One generated image as returned by the API
pub struct GeneratedImage {
    /// Raw base64 (no data URL prefix)
    pub b64_json: String,
    /// e.g. "image/png"; absent when the API couldn't determine it
    pub media_type: Option<String>,
}

/// Result of one generation run (all copies combined)
pub struct GenerationResult {
    pub images: Vec<GeneratedImage>,
    /// Total cost in dollars, when OpenRouter reports it
    pub cost: Option<f64>,
}

/// An image-capable model from the OpenRouter catalog
pub struct ImageModel {
    pub id: String,
    pub name: String,
    /// Estimated full-quality price per generated image in dollars
    pub price_per_image: Option<f64>,
}

impl ImageModel {
    /// Format the estimated per-image price for display
    pub fn price_per_image_display(&self) -> String {
        match self.price_per_image {
            Some(price) => format!("{:.4}", price),
            None => "?".to_string(),
        }
    }
}

/// What tuning parameters a model accepts, from the image models catalog
#[derive(Clone, Debug, Default)]
pub struct ModelCapabilities {
    pub supports_quality: bool,
    /// Supported resolution tiers ("512", "1K", ...); empty when the model
    /// has no size/resolution knob at all
    pub resolution_tiers: Vec<String>,
}

/// Quality/size parameters to actually send, after checking capabilities
#[derive(Debug, PartialEq)]
struct TuningPlan {
    quality: Option<String>,
    size: Option<String>,
    /// Human-readable adjustments to surface to the user
    notes: Vec<String>,
}

/// Minimal OpenRouter API client for image generation
#[derive(Clone)]
pub struct ImageClient {
    api_key: String,
    debug: bool,
    http: Client,
    /// Lazily fetched image-model capability catalog, shared across clones
    capabilities_cache: Arc<Mutex<Option<HashMap<String, ModelCapabilities>>>>,
    /// Models whose tuning-adjustment notes were already shown, so repeated
    /// generations in an interactive session stay quiet
    notes_printed: Arc<Mutex<std::collections::HashSet<String>>>,
}

/// API error carrying the HTTP status, so callers can react to 400s
#[derive(Debug)]
struct ApiError {
    status: u16,
    message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "OpenRouter error ({}): {}", self.status, self.message)
    }
}

impl std::error::Error for ApiError {}

// Request body types

#[derive(Serialize)]
struct ImageGenerationRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<String>,
    provider: ProviderPreferences,
}

#[derive(Serialize)]
struct ProviderPreferences {
    sort: &'static str,
}

// Response body types

#[derive(Deserialize)]
struct ImageGenerationResponse {
    #[serde(default)]
    data: Vec<ImageData>,
    usage: Option<UsageResponse>,
}

#[derive(Deserialize)]
struct ImageData {
    b64_json: String,
    #[serde(default)]
    media_type: Option<String>,
}

#[derive(Deserialize)]
struct UsageResponse {
    #[serde(default)]
    cost: Option<f64>,
    /// True when the request billed a bring-your-own-key provider account;
    /// the provider's charge then sits in cost_details, not cost
    #[serde(default)]
    is_byok: bool,
    #[serde(default)]
    cost_details: Option<CostDetails>,
}

#[derive(Deserialize)]
struct CostDetails {
    #[serde(default)]
    upstream_inference_cost: Option<f64>,
}

impl UsageResponse {
    /// The total dollars this generation actually cost, wherever billed
    fn effective_cost(&self) -> Option<f64> {
        let upstream_cost = self
            .cost_details
            .as_ref()
            .and_then(|details| details.upstream_inference_cost);

        match (self.is_byok, upstream_cost) {
            (true, Some(upstream_cost)) => Some(self.cost.unwrap_or(0.0) + upstream_cost),
            _ => self.cost,
        }
    }
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

#[derive(Deserialize)]
struct ModelListResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    architecture: Option<ModelArchitecture>,
    #[serde(default)]
    pricing: Option<ModelPricing>,
}

#[derive(Deserialize)]
struct ModelArchitecture {
    #[serde(default)]
    output_modalities: Vec<String>,
}

#[derive(Deserialize)]
struct CapabilityListResponse {
    data: Vec<CapabilityEntry>,
}

#[derive(Deserialize)]
struct CapabilityEntry {
    id: String,
    #[serde(default)]
    supported_parameters: HashMap<String, ParameterDescriptor>,
}

/// Capability descriptor; only enum `values` matter here, so range/boolean
/// descriptors deserialize with an empty list
#[derive(Deserialize)]
struct ParameterDescriptor {
    #[serde(default)]
    values: Vec<String>,
}

/// Pricing values arrive as decimal strings, e.g. "0.00003", expressed per
/// token (see TOKENS_PER_IMAGE_NOMINAL for the per-image normalization)
#[derive(Deserialize)]
struct ModelPricing {
    #[serde(default)]
    image_output: Option<String>,
    #[serde(default)]
    completion: Option<String>,
}

impl ImageClient {
    pub fn new(api_key: String, debug: bool) -> Self {
        Self {
            api_key,
            debug,
            http: Client::new(),
            capabilities_cache: Arc::new(Mutex::new(None)),
            notes_printed: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// Generate `settings.count` images for `prompt` with concurrent requests
    pub async fn generate(
        &self,
        prompt: &str,
        settings: &GenerationSettings,
    ) -> Result<GenerationResult> {
        // Check the capability catalog so we only send quality/size to models
        // that accept them; several models (including the default Gemini one)
        // reject unsupported parameters with an unhelpful 400.
        let capabilities = self.model_capabilities(&settings.model).await;
        let plan = plan_tuning(settings, capabilities.as_ref());
        if !plan.notes.is_empty() && self.notes_printed.lock().unwrap().insert(settings.model.clone())
        {
            for note in &plan.notes {
                eprintln!("Note: {}", note);
            }
        }
        let plan = Arc::new(plan);

        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..settings.count {
            let client = self.clone();
            let prompt = prompt.to_string();
            let model = settings.model.clone();
            let plan = Arc::clone(&plan);
            tasks.spawn(async move { client.generate_one(&prompt, &model, &plan).await });
        }

        let mut images = Vec::new();
        let mut cost_total: Option<f64> = None;
        let mut first_error: Option<anyhow::Error> = None;

        while let Some(joined) = tasks.join_next().await {
            match joined.context("Generation task panicked")? {
                Ok(response) => {
                    images.extend(response.images);
                    if let Some(cost) = response.cost {
                        cost_total = Some(cost_total.unwrap_or(0.0) + cost);
                    }
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }

        // Fail only when nothing succeeded; partial success still saves images
        match first_error {
            Some(error) if images.is_empty() => Err(error),
            Some(error) => {
                eprintln!("Warning: some generations failed: {:#}", error);
                Ok(GenerationResult {
                    images,
                    cost: cost_total,
                })
            }
            None => Ok(GenerationResult {
                images,
                cost: cost_total,
            }),
        }
    }

    /// Fetch and cache the image-model capability catalog, returning the
    /// entry for `model`. Returns None when the catalog is unreachable or
    /// the model isn't listed; callers then send parameters optimistically.
    async fn model_capabilities(&self, model: &str) -> Option<ModelCapabilities> {
        let cached = self.capabilities_cache.lock().unwrap().clone();
        let catalog = match cached {
            Some(catalog) => catalog,
            None => match self.fetch_capability_catalog().await {
                Ok(catalog) => {
                    *self.capabilities_cache.lock().unwrap() = Some(catalog.clone());
                    catalog
                }
                Err(error) => {
                    if self.debug {
                        eprintln!("Could not fetch model capabilities: {:#}", error);
                    }
                    return None;
                }
            },
        };
        catalog.get(model).cloned()
    }

    async fn fetch_capability_catalog(&self) -> Result<HashMap<String, ModelCapabilities>> {
        let response = self
            .http
            .get(format!("{}/images/models", BASE_URL))
            .send()
            .await
            .context("Request to OpenRouter image models API failed")?
            .error_for_status()
            .context("OpenRouter image models API returned an error")?;

        let list: CapabilityListResponse = response
            .json()
            .await
            .context("Failed to parse OpenRouter image models response")?;

        Ok(list
            .data
            .into_iter()
            .map(|entry| {
                let capabilities = ModelCapabilities {
                    supports_quality: entry.supported_parameters.contains_key("quality"),
                    resolution_tiers: entry
                        .supported_parameters
                        .get("resolution")
                        .map(|descriptor| descriptor.values.clone())
                        .unwrap_or_default(),
                };
                (entry.id, capabilities)
            })
            .collect())
    }

    /// Issue a single Images API request.
    ///
    /// As a safety net for models missing from the capability catalog, a 400
    /// rejection of a request that carried tuning parameters is retried once
    /// without them (parameter rejections often come back as unspecific
    /// "invalid argument" errors, so no message matching is attempted).
    async fn generate_one(
        &self,
        prompt: &str,
        model: &str,
        plan: &TuningPlan,
    ) -> Result<GenerationResult> {
        let has_tuning = plan.quality.is_some() || plan.size.is_some();

        match self.request_images(prompt, model, plan.quality.clone(), plan.size.clone()).await {
            Err(error) if has_tuning && is_bad_request(&error) => {
                eprintln!(
                    "Note: {} rejected the request ({}); retrying without quality/size",
                    model,
                    error.root_cause()
                );
                self.request_images(prompt, model, None, None).await
            }
            other => other,
        }
    }

    async fn request_images(
        &self,
        prompt: &str,
        model: &str,
        quality: Option<String>,
        size: Option<String>,
    ) -> Result<GenerationResult> {
        let request = ImageGenerationRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            quality,
            size,
            provider: ProviderPreferences { sort: "price" },
        };

        if self.debug {
            eprintln!(
                "Request: {}",
                serde_json::to_string_pretty(&request).unwrap_or_default()
            );
        }

        let response = self
            .http
            .post(format!("{}/images", BASE_URL))
            .bearer_auth(&self.api_key)
            .header("X-OpenRouter-Title", "get-image")
            .json(&request)
            .send()
            .await
            .context("Request to OpenRouter failed")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read OpenRouter response")?;

        if self.debug {
            eprintln!("Response ({}): {}", status, truncate_for_debug(&body));
        }

        if !status.is_success() {
            let message = serde_json::from_str::<ErrorResponse>(&body)
                .map(|parsed| parsed.error.message)
                .unwrap_or(body);
            return Err(ApiError {
                status: status.as_u16(),
                message,
            }
            .into());
        }

        let parsed: ImageGenerationResponse =
            serde_json::from_str(&body).context("Failed to parse OpenRouter response")?;

        Ok(GenerationResult {
            images: parsed
                .data
                .into_iter()
                .map(|image| GeneratedImage {
                    b64_json: image.b64_json,
                    media_type: image.media_type,
                })
                .collect(),
            cost: parsed.usage.and_then(|usage| usage.effective_cost()),
        })
    }

    /// List models that can output images, sorted cheapest first.
    ///
    /// Uses the public models endpoint (no API key required) with server-side
    /// filtering; without `output_modalities=image` the catalog silently
    /// omits image-only models like flux and gpt-image-1.
    pub async fn list_image_models(&self) -> Result<Vec<ImageModel>> {
        let url = format!(
            "{}/models?output_modalities=image&sort=pricing-low-to-high",
            BASE_URL
        );
        let response = self
            .http
            .get(url)
            .send()
            .await
            .context("Request to OpenRouter models API failed")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenRouter models API error ({}): {}", status, body);
        }

        let list: ModelListResponse = response
            .json()
            .await
            .context("Failed to parse OpenRouter models response")?;

        let mut models = filter_image_models(list.data);
        sort_models_by_price(&mut models);
        Ok(models)
    }
}

/// Translate a validated size setting into the Images API `size` field:
/// resolution tiers ("512", "1K", "2K", "4K") pass through, a bare dimension
/// becomes explicit "NxN" pixels, and "WxH" passes through.
fn size_for_api(size: &str) -> String {
    match size {
        "512" | "1K" | "2K" | "4K" => size.to_string(),
        single if !single.contains('x') => format!("{}x{}", single, single),
        dimensions => dimensions.to_string(),
    }
}

/// Decide which tuning parameters to send, given what the model supports.
///
/// With unknown capabilities (model missing from the catalog, or the catalog
/// unreachable) everything is sent optimistically; generate_one's retry
/// handles a rejection. With known capabilities, unsupported parameters are
/// dropped and an unsupported resolution tier is replaced by the nearest
/// supported one, each with a note for the user.
fn plan_tuning(
    settings: &GenerationSettings,
    capabilities: Option<&ModelCapabilities>,
) -> TuningPlan {
    let Some(capabilities) = capabilities else {
        return TuningPlan {
            quality: Some(settings.quality.clone()),
            size: Some(size_for_api(&settings.size)),
            notes: Vec::new(),
        };
    };

    let mut notes = Vec::new();

    let quality = if capabilities.supports_quality {
        Some(settings.quality.clone())
    } else {
        if settings.quality != "auto" {
            notes.push(format!(
                "{} has no quality setting; \"{}\" is ignored",
                settings.model, settings.quality
            ));
        }
        None
    };

    let size = if capabilities.resolution_tiers.is_empty() {
        notes.push(format!(
            "{} has no size setting; generating at the model's default resolution",
            settings.model
        ));
        None
    } else if let Some(tier) = nearest_supported_tier(&settings.size, &capabilities.resolution_tiers)
    {
        if tier != settings.size {
            notes.push(format!(
                "{} does not support size {}; using {}",
                settings.model, settings.size, tier
            ));
        }
        Some(tier)
    } else {
        // Explicit WxH pixels: the model has a resolution knob, so let
        // OpenRouter normalize the requested dimensions
        Some(size_for_api(&settings.size))
    };

    TuningPlan {
        quality,
        size,
        notes,
    }
}

/// For a tier-form size, find the supported tier closest to the requested
/// one. Returns None for non-tier (explicit pixel) sizes.
fn nearest_supported_tier(size: &str, supported_tiers: &[String]) -> Option<String> {
    let requested_index = RESOLUTION_TIERS.iter().position(|tier| *tier == size)?;

    supported_tiers
        .iter()
        .filter_map(|tier| {
            RESOLUTION_TIERS
                .iter()
                .position(|known| known == tier)
                .map(|index| (index, tier))
        })
        .min_by_key(|(index, _)| index.abs_diff(requested_index))
        .map(|(_, tier)| tier.clone())
}

/// True when the error is an HTTP 400 from the Images API
fn is_bad_request(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<ApiError>()
        .is_some_and(|api_error| api_error.status == 400)
}

/// Keep only models whose output modalities include "image" (defensive:
/// the server-side filter should already guarantee this)
fn filter_image_models(entries: Vec<ModelEntry>) -> Vec<ImageModel> {
    entries
        .into_iter()
        .filter(|entry| {
            entry.architecture.as_ref().is_some_and(|architecture| {
                architecture.output_modalities.iter().any(|m| m == "image")
            })
        })
        .map(|entry| {
            let price_per_image = entry.pricing.as_ref().and_then(estimate_price_per_image);
            ImageModel {
                id: entry.id,
                name: entry.name,
                price_per_image,
            }
        })
        .collect()
}

/// Estimate the full-quality price of one generated image from per-token
/// pricing. Exact for per-image-billed models; an upper-bound estimate for
/// token-billed ones (fewer tokens are used at lower quality/resolution).
fn estimate_price_per_image(pricing: &ModelPricing) -> Option<f64> {
    let per_token = pricing
        .image_output
        .as_deref()
        .or(pricing.completion.as_deref())
        .and_then(|value| value.trim().parse::<f64>().ok())?;

    (per_token > 0.0).then_some(per_token * TOKENS_PER_IMAGE_NOMINAL)
}

/// Sort cheapest first; models with unknown pricing go last
fn sort_models_by_price(models: &mut [ImageModel]) {
    models.sort_by(|a, b| {
        let a_price = a.price_per_image.unwrap_or(f64::INFINITY);
        let b_price = b.price_per_image.unwrap_or(f64::INFINITY);
        a_price
            .partial_cmp(&b_price)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn truncate_for_debug(body: &str) -> String {
    const DEBUG_LENGTH_MAX: usize = 2000;
    if body.len() <= DEBUG_LENGTH_MAX {
        body.to_string()
    } else {
        format!(
            "{}... [{} bytes total]",
            &body[..DEBUG_LENGTH_MAX],
            body.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, output_modalities: &[&str], pricing: Option<ModelPricing>) -> ModelEntry {
        ModelEntry {
            id: id.to_string(),
            name: id.to_string(),
            architecture: Some(ModelArchitecture {
                output_modalities: output_modalities.iter().map(|m| m.to_string()).collect(),
            }),
            pricing,
        }
    }

    #[test]
    fn test_filter_keeps_only_image_output_models() {
        let entries = vec![
            entry("text-only", &["text"], None),
            entry("image-model", &["text", "image"], None),
        ];
        let models = filter_image_models(entries);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "image-model");
    }

    #[test]
    fn test_price_recovers_per_image_cost_from_per_token_value() {
        // bytedance-seed/seedream-4.5 lists 0.04/4175 per token; the estimate
        // must recover the true $0.04 per image
        let pricing = ModelPricing {
            image_output: Some("0.00000958083832335329".to_string()),
            completion: Some("0".to_string()),
        };
        let price = estimate_price_per_image(&pricing).unwrap();
        assert!((price - 0.04).abs() < 1e-6);
    }

    #[test]
    fn test_price_ignores_zero_and_missing_values() {
        let zero = ModelPricing {
            image_output: Some("0".to_string()),
            completion: Some("0".to_string()),
        };
        assert_eq!(estimate_price_per_image(&zero), None);

        let missing = ModelPricing {
            image_output: None,
            completion: None,
        };
        assert_eq!(estimate_price_per_image(&missing), None);
    }

    #[test]
    fn test_models_sort_cheapest_first_with_unknown_last() {
        let mut models = vec![
            ImageModel {
                id: "unknown".into(),
                name: String::new(),
                price_per_image: None,
            },
            ImageModel {
                id: "pricey".into(),
                name: String::new(),
                price_per_image: Some(0.2),
            },
            ImageModel {
                id: "cheap".into(),
                name: String::new(),
                price_per_image: Some(0.001),
            },
        ];
        sort_models_by_price(&mut models);
        let order: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(order, ["cheap", "pricey", "unknown"]);
    }

    #[test]
    fn test_byok_cost_comes_from_upstream_inference_details() {
        // Real shape observed from a BYOK account: cost is 0, the provider's
        // charge is in cost_details
        let body = r#"{
            "prompt_tokens": 3,
            "completion_tokens": 1290,
            "cost": 0,
            "is_byok": true,
            "cost_details": {"upstream_inference_cost": 0.0387009}
        }"#;
        let usage: UsageResponse = serde_json::from_str(body).unwrap();
        assert!((usage.effective_cost().unwrap() - 0.0387009).abs() < 1e-9);

        let non_byok: UsageResponse =
            serde_json::from_str(r#"{"cost": 0.04, "cost_details": {"upstream_inference_cost": 0.03}}"#)
                .unwrap();
        assert_eq!(non_byok.effective_cost(), Some(0.04));
    }

    #[test]
    fn test_generation_response_parses_images_and_cost() {
        // Shape documented for POST /api/v1/images
        let body = r#"{
            "created": 1748372400,
            "data": [
                {"b64_json": "aGk=", "media_type": "image/png"},
                {"b64_json": "aGk="}
            ],
            "usage": {"prompt_tokens": 0, "completion_tokens": 4175, "total_tokens": 4175, "cost": 0.04}
        }"#;
        let parsed: ImageGenerationResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.data.len(), 2);
        assert_eq!(parsed.data[0].b64_json, "aGk=");
        assert_eq!(parsed.data[0].media_type.as_deref(), Some("image/png"));
        assert_eq!(parsed.data[1].media_type, None);
        assert_eq!(parsed.usage.unwrap().cost, Some(0.04));
    }

    #[test]
    fn test_size_for_api_maps_tiers_and_dimensions() {
        assert_eq!(size_for_api("512"), "512");
        assert_eq!(size_for_api("1K"), "1K");
        assert_eq!(size_for_api("1024"), "1024x1024");
        assert_eq!(size_for_api("1024x768"), "1024x768");
    }

    fn settings(quality: &str, size: &str) -> GenerationSettings {
        GenerationSettings {
            model: "test/model".to_string(),
            quality: quality.to_string(),
            size: size.to_string(),
            count: 1,
        }
    }

    fn capabilities(supports_quality: bool, tiers: &[&str]) -> ModelCapabilities {
        ModelCapabilities {
            supports_quality,
            resolution_tiers: tiers.iter().map(|tier| tier.to_string()).collect(),
        }
    }

    #[test]
    fn test_plan_sends_everything_when_capabilities_unknown() {
        let plan = plan_tuning(&settings("low", "512"), None);
        assert_eq!(plan.quality.as_deref(), Some("low"));
        assert_eq!(plan.size.as_deref(), Some("512"));
        assert!(plan.notes.is_empty());
    }

    #[test]
    fn test_plan_drops_unsupported_parameters_with_notes() {
        // Shape of google/gemini-2.5-flash-image: no quality, no resolution
        let plan = plan_tuning(&settings("low", "512"), Some(&capabilities(false, &[])));
        assert_eq!(plan.quality, None);
        assert_eq!(plan.size, None);
        assert_eq!(plan.notes.len(), 2);
    }

    #[test]
    fn test_plan_upgrades_to_nearest_supported_tier() {
        // Shape of google/gemini-3-pro-image: tiers 1K/2K/4K only
        let plan = plan_tuning(
            &settings("low", "512"),
            Some(&capabilities(false, &["1K", "2K", "4K"])),
        );
        assert_eq!(plan.size.as_deref(), Some("1K"));
        assert!(plan.notes.iter().any(|note| note.contains("using 1K")));
    }

    #[test]
    fn test_plan_keeps_supported_settings_without_notes() {
        // Shape of openai/gpt-image-1 plus a resolution knob
        let plan = plan_tuning(
            &settings("high", "2K"),
            Some(&capabilities(true, &["1K", "2K", "4K"])),
        );
        assert_eq!(plan.quality.as_deref(), Some("high"));
        assert_eq!(plan.size.as_deref(), Some("2K"));
        assert!(plan.notes.is_empty());
    }

    #[test]
    fn test_plan_passes_explicit_pixels_when_model_has_resolution_knob() {
        let plan = plan_tuning(
            &settings("low", "1024x768"),
            Some(&capabilities(false, &["1K", "2K"])),
        );
        assert_eq!(plan.size.as_deref(), Some("1024x768"));
    }

    #[test]
    fn test_bad_request_detection_uses_status_not_message() {
        let bad_request: anyhow::Error = ApiError {
            status: 400,
            message: "Request contains an invalid argument.".to_string(),
        }
        .into();
        assert!(is_bad_request(&bad_request));

        let server_error: anyhow::Error = ApiError {
            status: 502,
            message: "provider error".to_string(),
        }
        .into();
        assert!(!is_bad_request(&server_error));

        let other = anyhow::anyhow!("Request to OpenRouter failed");
        assert!(!is_bad_request(&other));
    }

    #[test]
    fn test_capability_catalog_parses_enum_and_range_descriptors() {
        let body = r#"{
            "data": [{
                "id": "google/gemini-3.1-flash-image",
                "supported_parameters": {
                    "resolution": {"type": "enum", "values": ["512", "1K", "2K", "4K"]},
                    "n": {"type": "range", "min": 1, "max": 1},
                    "seed": {"type": "boolean"}
                }
            }]
        }"#;
        let parsed: CapabilityListResponse = serde_json::from_str(body).unwrap();
        let entry = &parsed.data[0];
        assert!(entry.supported_parameters.contains_key("resolution"));
        assert_eq!(
            entry.supported_parameters["resolution"].values,
            ["512", "1K", "2K", "4K"]
        );
        assert!(entry.supported_parameters["n"].values.is_empty());
    }
}
