//! Gemini provider implementation
//!
//! Auth: OAuth via Gemini CLI credentials
//! API: Cloud Code Private API (cloudcode-pa.googleapis.com)
//!
//! ## Data Available
//! - Per-model quota tracking (Pro, Flash, etc.)
//! - Tier detection (Free, Paid, Workspace, Legacy)
//! - Project ID discovery for accurate quota attribution
//! - JWT-based account information (email, hosted domain)

use async_trait::async_trait;
use base64::Engine;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

use crate::providers::traits::*;
use crate::storage::{UsageData, ModelQuota};

// Cloud Code Private API endpoints
const CLOUD_CODE_QUOTA_URL: &str = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota";
const CLOUD_CODE_ASSIST_URL: &str = "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
const GCP_PROJECTS_URL: &str = "https://cloudresourcemanager.googleapis.com/v1/projects";

pub struct GeminiProvider {
    credentials: Arc<RwLock<Option<GeminiCredentials>>>,
    client: reqwest::Client,
    project_id: Arc<RwLock<Option<String>>>,
    tier: Arc<RwLock<Option<GeminiUserTier>>>,
    account_info: Arc<RwLock<Option<AccountInfo>>>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct GeminiCredentials {
    access_token: String,
    refresh_token: String,
    #[serde(default = "default_token_uri")]
    token_uri: String,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    client_secret: Option<String>,
    #[serde(default)]
    expiry_date: Option<i64>,  // Milliseconds since epoch
    #[serde(default)]
    id_token: Option<String>,
}

fn default_token_uri() -> String {
    "https://oauth2.googleapis.com/token".to_string()
}

#[derive(Debug, Clone)]
enum GeminiUserTier {
    Free,
    Standard,
    Legacy,
    Workspace,
}

#[derive(Debug, Clone)]
struct AccountInfo {
    email: String,
    hosted_domain: Option<String>,
}

// Cloud Code API response structures
#[derive(Deserialize, Debug)]
struct QuotaResponse {
    #[serde(default)]
    buckets: Vec<QuotaBucket>,
}

#[derive(Deserialize, Debug)]
struct QuotaBucket {
    #[serde(rename = "modelId")]
    model_id: String,
    #[serde(rename = "remainingFraction")]
    remaining_fraction: f64,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
    #[serde(rename = "tokenType")]
    token_type: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CodeAssistResponse {
    #[serde(rename = "currentTier")]
    current_tier: Option<TierInfo>,
    #[serde(rename = "managedProjectId")]
    managed_project_id: Option<String>,
}

#[derive(Deserialize, Debug)]
struct TierInfo {
    id: String,
}

#[derive(Deserialize, Debug)]
struct ProjectsResponse {
    projects: Option<Vec<Project>>,
}

#[derive(Deserialize, Debug)]
struct Project {
    #[serde(rename = "projectId")]
    project_id: String,
    labels: Option<std::collections::HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
struct JwtClaims {
    email: Option<String>,
    hd: Option<String>,  // Hosted domain
}

impl GeminiProvider {
    pub fn new() -> Self {
        let provider = Self {
            credentials: Arc::new(RwLock::new(None)),
            client: reqwest::Client::new(),
            project_id: Arc::new(RwLock::new(None)),
            tier: Arc::new(RwLock::new(None)),
            account_info: Arc::new(RwLock::new(None)),
        };

        // Load credentials synchronously during initialization
        if let Some(path) = Self::get_credentials_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(creds) = serde_json::from_str::<GeminiCredentials>(&content) {
                    // Extract account info from JWT if present
                    let account_info = creds.id_token.as_ref()
                        .and_then(|token| Self::extract_jwt_claims(token).ok());

                    // Use blocking_write since we're in a sync context
                    if let Some(info) = account_info {
                        *provider.account_info.blocking_write() = Some(info);
                    }
                    *provider.credentials.blocking_write() = Some(creds);
                }
            }
        }

        provider
    }

    fn get_credentials_path() -> Option<PathBuf> {
        // Correct path: ~/.gemini/oauth_creds.json on all platforms
        dirs::home_dir().map(|p| p.join(".gemini").join("oauth_creds.json"))
    }

    async fn load_credentials(&self) {
        if let Some(path) = Self::get_credentials_path() {
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                if let Ok(creds) = serde_json::from_str::<GeminiCredentials>(&content) {
                    // Extract account info from JWT
                    if let Some(id_token) = &creds.id_token {
                        if let Ok(account_info) = Self::extract_jwt_claims(id_token) {
                            *self.account_info.write().await = Some(account_info);
                        }
                    }

                    *self.credentials.write().await = Some(creds);
                }
            }
        }
    }

    fn extract_jwt_claims(id_token: &str) -> ProviderResult<AccountInfo> {
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() != 3 {
            return Err(ProviderError::Parse("Invalid JWT format".into()));
        }

        // Decode the payload (second part)
        let payload = parts[1];

        // Add padding if needed for base64 decoding
        let padded = match payload.len() % 4 {
            2 => format!("{}==", payload),
            3 => format!("{}=", payload),
            _ => payload.to_string(),
        };

        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(padded.as_bytes())
            .map_err(|e| ProviderError::Parse(format!("JWT decode error: {}", e)))?;

        let claims: JwtClaims = serde_json::from_slice(&decoded)
            .map_err(|e| ProviderError::Parse(format!("JWT parse error: {}", e)))?;

        Ok(AccountInfo {
            email: claims.email.unwrap_or_else(|| "unknown@example.com".to_string()),
            hosted_domain: claims.hd,
        })
    }

    async fn ensure_valid_token(&self) -> ProviderResult<String> {
        let creds = {
            let creds_guard = self.credentials.read().await;
            creds_guard.clone().ok_or_else(|| {
                ProviderError::AuthRequired
            })?
        };

        // Check if token is expired (expiry_date is in milliseconds)
        let needs_refresh = if let Some(expiry_ms) = creds.expiry_date {
            let now_ms = Utc::now().timestamp_millis();
            expiry_ms < now_ms
        } else {
            false
        };

        if needs_refresh {
            self.refresh_token().await?;
            let creds_guard = self.credentials.read().await;
            Ok(creds_guard.as_ref().unwrap().access_token.clone())
        } else {
            Ok(creds.access_token.clone())
        }
    }

    async fn refresh_token(&self) -> ProviderResult<()> {
        let creds = {
            let creds_guard = self.credentials.read().await;
            creds_guard.clone().ok_or(ProviderError::AuthRequired)?
        };

        // Extract OAuth credentials from Gemini CLI installation
        let (client_id, client_secret) = if let (Some(id), Some(secret)) = (&creds.client_id, &creds.client_secret) {
            (id.clone(), secret.clone())
        } else {
            Self::extract_oauth_credentials().await?
        };

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            #[serde(default)]
            expires_in: Option<i64>,
            #[serde(default)]
            id_token: Option<String>,
        }

        let response = self.client
            .post(&creds.token_uri)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &creds.refresh_token),
                ("client_id", &client_id),
                ("client_secret", &client_secret),
            ])
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ProviderError::AuthFailed(
                "Token refresh failed. Run 'gemini auth' to re-authenticate.".into()
            ));
        }

        let token: TokenResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))?;

        // Calculate new expiry time
        let expiry_date = if let Some(expires_in) = token.expires_in {
            Some(Utc::now().timestamp_millis() + (expires_in * 1000))
        } else {
            None
        };

        // Update credentials in memory
        let mut updated_creds = creds.clone();
        updated_creds.access_token = token.access_token.clone();
        updated_creds.expiry_date = expiry_date;
        if let Some(new_id_token) = token.id_token {
            updated_creds.id_token = Some(new_id_token.clone());

            // Update account info
            if let Ok(account_info) = Self::extract_jwt_claims(&new_id_token) {
                *self.account_info.write().await = Some(account_info);
            }
        }

        *self.credentials.write().await = Some(updated_creds.clone());

        // Persist to disk
        if let Some(path) = Self::get_credentials_path() {
            let json = serde_json::to_string_pretty(&updated_creds)
                .map_err(|e| ProviderError::Provider(format!("Serialize error: {}", e)))?;

            // Atomic write: temp file + rename
            let temp_path = path.with_extension("tmp");
            tokio::fs::write(&temp_path, json).await
                .map_err(|e| ProviderError::Provider(format!("Write error: {}", e)))?;
            tokio::fs::rename(&temp_path, &path).await
                .map_err(|e| ProviderError::Provider(format!("Rename error: {}", e)))?;
        }

        Ok(())
    }

    async fn extract_oauth_credentials() -> ProviderResult<(String, String)> {
        // Locate gemini binary
        let gemini_path = which::which("gemini")
            .map_err(|_| ProviderError::NotConfigured)?;

        // Resolve symlinks to get real installation path
        let real_path = tokio::fs::canonicalize(&gemini_path).await
            .unwrap_or(gemini_path);

        // Search for oauth2.js in various possible locations
        let search_paths = vec![
            // Homebrew nested structure
            real_path.parent()
                .and_then(|p| p.parent())
                .map(|p| p.join("libexec/lib/node_modules/@google/gemini-cli/node_modules/@google/gemini-cli-core/dist/src/code_assist/oauth2.js")),
            // npm sibling structure
            real_path.parent()
                .and_then(|p| p.parent())
                .map(|p| p.join("lib/node_modules/@google/gemini-cli-core/dist/src/code_assist/oauth2.js")),
            // Direct parent
            real_path.parent()
                .map(|p| p.join("node_modules/@google/gemini-cli-core/dist/src/code_assist/oauth2.js")),
        ];

        for maybe_path in search_paths.iter().filter_map(|p| p.as_ref()) {
            if let Ok(content) = tokio::fs::read_to_string(maybe_path).await {
                if let Ok((client_id, client_secret)) = Self::parse_oauth_credentials(&content) {
                    return Ok((client_id, client_secret));
                }
            }
        }

        Err(ProviderError::NotConfigured)
    }

    fn parse_oauth_credentials(content: &str) -> ProviderResult<(String, String)> {
        let client_id_re = Regex::new(r#"OAUTH_CLIENT_ID\s*=\s*['"]([^'"]+)['"]"#).unwrap();
        let client_secret_re = Regex::new(r#"OAUTH_CLIENT_SECRET\s*=\s*['"]([^'"]+)['"]"#).unwrap();

        let client_id = client_id_re.captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| ProviderError::Parse("Client ID not found".into()))?;

        let client_secret = client_secret_re.captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| ProviderError::Parse("Client secret not found".into()))?;

        Ok((client_id, client_secret))
    }

    async fn discover_project_id(&self, token: &str) -> ProviderResult<String> {
        // First try loadCodeAssist for managed project ID
        if let Ok(project_id) = self.load_code_assist_status(token).await {
            return Ok(project_id);
        }

        // Fallback: search for gen-lang-client projects
        let response = self.client
            .get(GCP_PROJECTS_URL)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let projects: ProjectsResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))?;

        if let Some(projects) = projects.projects {
            // Look for projects with gen-lang-client prefix or generative-language label
            for project in projects {
                if project.project_id.starts_with("gen-lang-client") {
                    return Ok(project.project_id);
                }

                if let Some(labels) = project.labels {
                    if labels.contains_key("generative-language") {
                        return Ok(project.project_id);
                    }
                }
            }
        }

        Err(ProviderError::Provider("No Gemini project found".into()))
    }

    async fn load_code_assist_status(&self, token: &str) -> ProviderResult<String> {
        let response = self.client
            .post(CLOUD_CODE_ASSIST_URL)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ProviderError::Provider("loadCodeAssist failed".into()));
        }

        let assist: CodeAssistResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))?;

        // Update tier information
        if let Some(tier_info) = assist.current_tier {
            let tier = match tier_info.id.as_str() {
                "STANDARD" => GeminiUserTier::Standard,
                "FREE" => GeminiUserTier::Free,
                "LEGACY" => GeminiUserTier::Legacy,
                _ => GeminiUserTier::Free,
            };
            *self.tier.write().await = Some(tier);
        }

        assist.managed_project_id
            .ok_or_else(|| ProviderError::Provider("No managed project ID".into()))
    }

    async fn fetch_quota(&self, token: &str, project_id: &str) -> ProviderResult<QuotaResponse> {
        let response = self.client
            .post(CLOUD_CODE_QUOTA_URL)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .header("User-Agent", "LimitsWatcher/1.0")
            .json(&serde_json::json!({
                "project": project_id
            }))
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        match response.status().as_u16() {
            401 => return Err(ProviderError::TokenExpired),
            403 => return Err(ProviderError::AuthFailed("Access denied".into())),
            404 => return Err(ProviderError::Provider(
                "Quota API endpoint not found. Your Gemini CLI version may be outdated.".into()
            )),
            429 => {
                let retry_after = response.headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                return Err(ProviderError::RateLimited(retry_after));
            }
            _ => {}
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))
    }

    fn aggregate_quotas(buckets: Vec<QuotaBucket>) -> (Vec<ModelQuota>, f64, Option<DateTime<Utc>>) {
        use std::collections::HashMap;

        // Group by model_id and keep the lowest remaining_fraction
        let mut model_map: HashMap<String, (f64, Option<DateTime<Utc>>)> = HashMap::new();

        for bucket in buckets {
            let reset_time = bucket.reset_time.as_ref().and_then(|s| {
                DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });

            model_map.entry(bucket.model_id.clone())
                .and_modify(|(existing_fraction, existing_reset)| {
                    if bucket.remaining_fraction < *existing_fraction {
                        *existing_fraction = bucket.remaining_fraction;
                        *existing_reset = reset_time;
                    }
                })
                .or_insert((bucket.remaining_fraction, reset_time));
        }

        // Convert to ModelQuota vec
        let mut model_quotas: Vec<ModelQuota> = model_map.iter().map(|(model_id, (fraction, reset))| {
            ModelQuota {
                model_id: model_id.clone(),
                percent_left: fraction * 100.0,
                reset_time: *reset,
            }
        }).collect();

        // Sort by model_id for consistent display
        model_quotas.sort_by(|a, b| a.model_id.cmp(&b.model_id));

        // Find overall lowest percentage and earliest reset time
        let overall_percent = model_quotas.iter()
            .map(|q| q.percent_left)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(100.0);

        let overall_reset = model_quotas.iter()
            .filter_map(|q| q.reset_time)
            .min();

        (model_quotas, overall_percent, overall_reset)
    }

    fn get_plan_display(&self, tier: &Option<GeminiUserTier>, account_info: &Option<AccountInfo>) -> String {
        match (tier, account_info.as_ref().and_then(|a| a.hosted_domain.as_ref())) {
            (Some(GeminiUserTier::Standard), _) => "Paid".to_string(),
            (Some(GeminiUserTier::Free), Some(_)) => "Workspace".to_string(),
            (Some(GeminiUserTier::Free), None) => "Free".to_string(),
            (Some(GeminiUserTier::Legacy), _) => "Legacy".to_string(),
            (Some(GeminiUserTier::Workspace), _) => "Workspace".to_string(),
            _ => "Unknown".to_string(),
        }
    }
}

#[async_trait]
impl Provider for GeminiProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "gemini".to_string(),
            name: "Gemini".to_string(),
            website: "https://gemini.google.com".to_string(),
            auth_methods: vec![AuthMethod::Cli],
            has_session_limits: true,
            has_weekly_limits: true,
            has_credits: false,
            icon: "gemini".to_string(),
        }
    }

    async fn is_authenticated(&self) -> bool {
        self.credentials.read().await.is_some()
    }

    async fn fetch_usage(&self) -> ProviderResult<UsageData> {
        let token = self.ensure_valid_token().await?;

        // Discover or get cached project ID
        let project_id = {
            let cached_id = self.project_id.read().await;
            if let Some(id) = cached_id.as_ref() {
                id.clone()
            } else {
                drop(cached_id);
                let discovered_id = self.discover_project_id(&token).await?;
                *self.project_id.write().await = Some(discovered_id.clone());
                discovered_id
            }
        };

        // Fetch quota from Cloud Code API
        let quota = self.fetch_quota(&token, &project_id).await?;

        // Aggregate quotas by model
        let (model_quotas, overall_percent, overall_reset) = Self::aggregate_quotas(quota.buckets);

        // Map to UsageData structure
        // session_limit represents overall quota percentage (0-100)
        // We use a fixed scale where 100 = 100%
        let used = ((100.0 - overall_percent) * 100.0) as u64;
        let limit = 10000u64;  // Scale to make percentages work

        Ok(UsageData {
            session_used: used,
            session_limit: limit,
            weekly_used: 0,
            weekly_limit: 0,
            credits_remaining: None,
            reset_time: overall_reset,
            weekly_reset_time: None,
            last_updated: Utc::now(),
            error: None,
            model_quotas: Some(model_quotas),
        })
    }

    async fn start_auth(&mut self) -> ProviderResult<Option<AuthFlow>> {
        Ok(Some(AuthFlow {
            url: "https://ai.google.dev/gemini-api/docs/downloads".to_string(),
            user_code: None,
            instructions: concat!(
                "Gemini uses OAuth via the Gemini CLI.\n\n",
                "1. Install Gemini CLI from ai.google.dev\n",
                "2. Run 'gemini auth' to authenticate\n",
                "3. Click 'Check for credentials' below"
            ).to_string(),
            poll_interval: None,
        }))
    }

    async fn complete_auth(&mut self, _response: AuthResponse) -> ProviderResult<()> {
        self.load_credentials().await;

        let is_authenticated = self.credentials.read().await.is_some();
        if is_authenticated {
            Ok(())
        } else {
            Err(ProviderError::AuthFailed(
                "Gemini CLI not authenticated. Run 'gemini auth' in terminal.".into()
            ))
        }
    }

    async fn logout(&mut self) -> ProviderResult<()> {
        *self.credentials.write().await = None;
        *self.project_id.write().await = None;
        *self.tier.write().await = None;
        *self.account_info.write().await = None;
        Ok(())
    }

    fn auth_status(&self) -> AuthStatus {
        // We need to access RwLock in a sync context - use try_read instead
        let creds = self.credentials.try_read().ok();
        let account_info = self.account_info.try_read().ok().and_then(|g| g.clone());
        let tier = self.tier.try_read().ok().and_then(|g| g.clone());

        if let Some(creds_guard) = creds {
            if creds_guard.is_some() {
                let user_display = if let Some(info) = account_info {
                    let plan = self.get_plan_display(&tier, &Some(info.clone()));
                    format!("{} ({})", info.email, plan)
                } else {
                    "via Gemini CLI".to_string()
                };

                let expires = creds_guard.as_ref()
                    .and_then(|c| c.expiry_date)
                    .map(|ms| {
                        DateTime::from_timestamp_millis(ms)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_default()
                    });

                return AuthStatus::Authenticated {
                    user: Some(user_display),
                    expires,
                };
            }
        }

        AuthStatus::NotAuthenticated
    }
}
