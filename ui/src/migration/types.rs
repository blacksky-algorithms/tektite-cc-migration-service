// Core types for Migration Service - no dioxus imports needed here
use serde::{Deserialize, Serialize, Serializer};
use std::collections::VecDeque;

use crate::services::client::ClientPdsProvider;

/// PDS server description response structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PdsContactInfo {
    pub email: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PdsLinks {
    #[serde(rename = "privacyPolicy")]
    pub privacy_policy: Option<String>,
    #[serde(rename = "termsOfService")]
    pub terms_of_service: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PdsDescribeResponse {
    #[serde(rename = "availableUserDomains")]
    pub available_user_domains: Vec<String>,
    pub contact: Option<PdsContactInfo>,
    pub did: String,
    #[serde(rename = "inviteCodeRequired")]
    pub invite_code_required: Option<bool>,
    pub links: Option<PdsLinks>,
    #[serde(rename = "phoneVerificationRequired")]
    pub phone_verification_required: Option<bool>,
}

impl PdsDescribeResponse {
    /// Create a successful describe response
    pub fn success(
        available_user_domains: Vec<String>,
        contact: Option<PdsContactInfo>,
        did: String,
        invite_code_required: Option<bool>,
        links: Option<PdsLinks>,
        phone_verification_required: Option<bool>,
    ) -> Self {
        Self {
            available_user_domains,
            contact,
            did,
            invite_code_required,
            links,
            phone_verification_required,
        }
    }
}

/// Generic PDS login response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PdsLoginResponse {
    pub success: bool,
    pub message: String,
    pub did: Option<String>,
    pub session: Option<SessionCredentials>,
}

impl PdsLoginResponse {
    /// Create a successful login response
    pub fn success(message: &str, did: String, session: SessionCredentials) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            did: Some(did),
            session: Some(session),
        }
    }

    /// Create an error login response
    pub fn error(message: &str) -> Self {
        Self {
            success: false,
            message: message.to_string(),
            did: None,
            session: None,
        }
    }
}

/// Session credentials for ATProto PDS authentication
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionCredentials {
    pub did: String,
    pub handle: String,
    pub pds: String,
    pub access_jwt: String,
    pub refresh_jwt: String,
}

// Form step management
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum FormStep {
    Login,
    SelectPds,
    MigrationDetails,
    PlcVerification,
}

// Validation status enums
#[derive(Clone, PartialEq, Debug)]
pub enum HandleValidation {
    None,
    Checking,
    Available,
    Unavailable,
    Error,
}

#[derive(Clone, PartialEq, Debug)]
pub enum PasswordValidation {
    None,
    Match,
    NoMatch,
}

#[derive(Clone, PartialEq, Debug)]
pub enum EmailValidation {
    None,
    Valid,
    Invalid,
}

// Action enum for state mutations
#[derive(Clone, Debug)]
pub enum MigrationAction {
    // Form 1 actions
    SetHandle(String),
    SetPassword(String),
    SetProvider(ClientPdsProvider),
    SetLoading(bool),
    SetAuthenticating(bool),
    SetLoginResponse(Option<PdsLoginResponse>),
    SetSessionStored(bool),
    SetOriginalHandle(String),

    // Form 2 actions
    SetNewPdsUrl(String),
    SetForm2Submitted(bool),
    SetPdsDescribeResponse(Option<PdsDescribeResponse>),
    SetDescribingPds(bool),

    // Form 3 actions
    SetNewHandle(String),
    SetNewPassword(String),
    SetNewPasswordConfirm(String),
    SetEmailAddress(String),
    SetInviteCode(String),
    SetSelectedDomain(String),

    // Form 4 - PLC Verification actions
    SetPlcVerificationCode(String),
    SetPlcUnsigned(String),
    SetPlcVerifying(bool),

    // Validation actions (only handle validation is still needed)
    SetHandleValidation(HandleValidation),
    SetCheckingHandle(bool),

    // Migration process actions
    SetMigrating(bool),
    SetMigrationError(Option<String>),
    SetMigrationStep(String),
    SetNewPdsSession(Option<SessionCredentials>),
    SetCurrentStep(FormStep),

    // Extended migration progress tracking
    SetMigrationProgress(MigrationProgress),
    SetRepoProgress(RepoProgress),
    SetBlobProgress(BlobProgress),
    SetPreferencesProgress(PreferencesProgress),
    SetPlcProgress(PlcProgress),
    SetMigrationCompleted(bool),

    // PLC recommendation storage
    SetPlcRecommendation(Option<String>),
    // Original PDS describe response cache
    SetOriginalPdsDescribe(Option<PdsDescribeResponse>),
    // Console message logging
    AddConsoleMessage(String),
}

// Form state structs
#[derive(Clone)]
pub struct LoginForm {
    pub handle: String,
    pub password: String,
    pub provider: ClientPdsProvider,
    pub is_loading: bool,
    pub is_authenticating: bool,
    pub login_response: Option<PdsLoginResponse>,
    pub session_stored: bool,
    pub original_handle: String,
}

#[derive(Clone, Default)]
pub struct PdsSelectionForm {
    pub pds_url: String,
    pub submitted: bool,
    pub describe_response: Option<PdsDescribeResponse>,
    pub is_describing: bool,
}

#[derive(Clone, Default)]
pub struct MigrationDetailsForm {
    pub handle: String,
    pub password: String,
    pub password_confirm: String,
    pub email: String,
    pub invite_code: String,
    pub suggested_handle: String,
    pub is_checking_handle: bool,
    pub selected_domain: Option<String>,
}

#[derive(Clone, Default)]
pub struct PlcVerificationForm {
    pub verification_code: String,
    pub plc_unsigned: String,
    pub handle_context: String,
    pub is_verifying: bool,
}

#[derive(Clone)]
pub struct ValidationStates {
    pub handle: HandleValidation,
    pub password: PasswordValidation,
    pub email: EmailValidation,
}

// Migration progress tracking structures

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MigrationProgress {
    // Repository migration
    pub repo_exported: bool,
    pub repo_imported: bool,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub repo_car_size: u64,

    // OPFS Blob migration
    pub missing_blobs_checked: bool,
    pub blobs_exported: bool,
    pub blobs_imported: bool,
    pub total_blob_count: u32,
    pub imported_blob_count: u32,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub total_blob_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub downloaded_blob_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub opfs_storage_used: u64,

    // Preferences migration
    pub preferences_exported: bool,
    pub preferences_imported: bool,

    // PLC operations
    pub plc_recommended: bool,
    pub plc_token_requested: bool,
    pub plc_signed: bool,
    pub plc_submitted: bool,

    // Final activation
    pub new_account_activated: bool,
    pub old_account_deactivated: bool,

    // Resume capability
    pub migration_resumable: bool,
    pub last_checkpoint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RepoProgress {
    pub export_complete: bool,
    pub import_complete: bool,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub car_size: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct BlobProgress {
    pub total_blobs: u32,
    pub processed_blobs: u32,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub total_bytes: u64,
    #[serde(serialize_with = "serialize_u64_as_string")]
    pub processed_bytes: u64,
    pub current_blob_cid: Option<String>,
    pub current_blob_progress: Option<f64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct PreferencesProgress {
    pub export_complete: bool,
    pub import_complete: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct PlcProgress {
    pub recommendation_complete: bool,
    pub token_requested: bool,
    pub operation_signed: bool,
    pub operation_submitted: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct MigrationState {
    pub current_step: FormStep,
    pub form1: LoginForm,
    pub form2: PdsSelectionForm,
    pub form3: MigrationDetailsForm,
    pub form4: PlcVerificationForm,
    pub validations: ValidationStates,
    // Migration process state
    pub is_migrating: bool,
    pub migration_error: Option<String>,
    pub migration_step: String,
    pub new_pds_session: Option<SessionCredentials>,
    // Extended progress tracking
    pub migration_progress: MigrationProgress,
    pub repo_progress: RepoProgress,
    pub blob_progress: BlobProgress,
    pub preferences_progress: PreferencesProgress,
    pub plc_progress: PlcProgress,
    pub migration_completed: bool,
    // PLC recommendation storage
    pub plc_recommendation: Option<String>,
    // Original PDS describe response cache
    pub original_pds_describe: Option<PdsDescribeResponse>,
    // Console messages for blob progress display (max 10 recent messages)
    pub console_messages: VecDeque<String>,
    // Performance optimization: cache for unified_blob_progress
    pub cached_unified_blob_progress: Option<BlobProgress>,
    pub blob_progress_cache_key: u64,
}

impl MigrationState {
    /// Reduces the state based on an action
    pub fn reduce(mut self, action: MigrationAction) -> Self {
        match action {
            // Form 1 actions
            MigrationAction::SetHandle(handle) => {
                self.form1.handle = handle;
            }
            MigrationAction::SetPassword(password) => {
                self.form1.password = password;
            }
            MigrationAction::SetProvider(provider) => {
                self.form1.provider = provider;
            }
            MigrationAction::SetLoading(loading) => {
                self.form1.is_loading = loading;
            }
            MigrationAction::SetAuthenticating(auth) => {
                self.form1.is_authenticating = auth;
            }
            MigrationAction::SetLoginResponse(response) => {
                self.form1.login_response = response;
            }
            MigrationAction::SetSessionStored(stored) => {
                self.form1.session_stored = stored;
            }
            MigrationAction::SetOriginalHandle(handle) => {
                self.form1.original_handle = handle;
            }

            // Form 2 actions
            MigrationAction::SetNewPdsUrl(url) => {
                self.form2.pds_url = url;
            }
            MigrationAction::SetForm2Submitted(submitted) => {
                self.form2.submitted = submitted;
            }
            MigrationAction::SetPdsDescribeResponse(response) => {
                self.form2.describe_response = response;
            }
            MigrationAction::SetDescribingPds(describing) => {
                self.form2.is_describing = describing;
            }

            // Form 3 actions
            MigrationAction::SetNewHandle(handle) => {
                self.form3.handle = handle;
            }
            MigrationAction::SetNewPassword(password) => {
                self.form3.password = password;
            }
            MigrationAction::SetNewPasswordConfirm(password) => {
                self.form3.password_confirm = password;
            }
            MigrationAction::SetEmailAddress(email) => {
                self.form3.email = email;
            }
            MigrationAction::SetInviteCode(code) => {
                self.form3.invite_code = code;
            }
            MigrationAction::SetCheckingHandle(checking) => {
                self.form3.is_checking_handle = checking;
            }
            MigrationAction::SetSelectedDomain(domain) => {
                self.form3.selected_domain = Some(domain);
            }

            // Form 4 - PLC Verification actions
            MigrationAction::SetPlcVerificationCode(code) => {
                self.form4.verification_code = code;
            }
            MigrationAction::SetPlcUnsigned(plc_unsigned) => {
                self.form4.plc_unsigned = plc_unsigned;
            }
            MigrationAction::SetPlcVerifying(verifying) => {
                self.form4.is_verifying = verifying;
            }

            // Validation actions
            MigrationAction::SetHandleValidation(validation) => {
                self.validations.handle = validation;
            }

            // Migration process actions
            MigrationAction::SetMigrating(migrating) => {
                crate::console_info!(
                    "[REDUCER] SetMigrating reducer entered with value: {} - timestamp: {}",
                    migrating,
                    js_sys::Date::now()
                );

                let old_value = self.is_migrating;
                self.is_migrating = migrating;

                crate::console_info!(
                    "[STATE] Migration state changing: is_migrating={} -> {} - timestamp: {}",
                    old_value,
                    migrating,
                    js_sys::Date::now()
                );

                crate::console_info!("[REDUCER] SetMigrating reducer completed successfully - final is_migrating: {}", 
                    self.is_migrating);
            }
            MigrationAction::SetMigrationError(error) => {
                self.migration_error = error;
            }
            MigrationAction::SetMigrationStep(step) => {
                self.migration_step = step;
            }
            MigrationAction::SetNewPdsSession(session) => {
                self.new_pds_session = session;
            }
            MigrationAction::SetCurrentStep(step) => {
                let old_step = &self.current_step;

                // Initialize domain selection when entering MigrationDetails form
                if step == FormStep::MigrationDetails && self.form3.selected_domain.is_none() {
                    let domains = self.get_available_domains();
                    if let Some(first_domain) = domains.first() {
                        self.form3.selected_domain = Some(first_domain.clone());
                    }
                }

                crate::console_info!("[FORM] Transitioning from {:?} to {:?} - migration_status: is_migrating={}, completed={} - timestamp: {}", 
                    old_step, step, self.is_migrating, self.migration_completed, js_sys::Date::now());

                self.current_step = step;
            }

            // Extended migration progress tracking
            MigrationAction::SetMigrationProgress(progress) => {
                self.migration_progress = progress;
            }
            MigrationAction::SetRepoProgress(progress) => {
                self.repo_progress = progress;
                self.update_unified_blob_progress_cache();
            }
            MigrationAction::SetBlobProgress(progress) => {
                crate::console_debug!("[BLOB] Progress state updated: total={}, processed={}, total_bytes={}, processed_bytes={}", 
                    progress.total_blobs, progress.processed_blobs, progress.total_bytes, progress.processed_bytes);
                self.blob_progress = progress;
                self.update_unified_blob_progress_cache();
            }
            MigrationAction::SetPreferencesProgress(progress) => {
                self.preferences_progress = progress;
            }
            MigrationAction::SetPlcProgress(progress) => {
                self.plc_progress = progress;
            }
            MigrationAction::SetMigrationCompleted(completed) => {
                let old_value = self.migration_completed;
                self.migration_completed = completed;
                crate::console_info!("[STATE] Migration completion changing: migration_completed={} -> {} - timestamp: {}", 
                    old_value, completed, js_sys::Date::now());
            }
            MigrationAction::SetPlcRecommendation(recommendation) => {
                self.plc_recommendation = recommendation;
            }
            MigrationAction::SetOriginalPdsDescribe(describe) => {
                self.original_pds_describe = describe;
            }
            MigrationAction::AddConsoleMessage(message) => {
                self.console_messages.push_back(message);
                // Keep only the most recent 10 messages
                while self.console_messages.len() > 10 {
                    self.console_messages.pop_front();
                }
            }
        }
        self
    }

    /// Reduces the state based on an action in-place (preserves Dioxus Signal reactivity)
    pub fn reduce_in_place(&mut self, action: MigrationAction) {
        match action {
            // Form 1 actions
            MigrationAction::SetHandle(handle) => {
                self.form1.handle = handle;
            }
            MigrationAction::SetPassword(password) => {
                self.form1.password = password;
            }
            MigrationAction::SetProvider(provider) => {
                self.form1.provider = provider;
            }
            MigrationAction::SetLoading(loading) => {
                self.form1.is_loading = loading;
            }
            MigrationAction::SetAuthenticating(auth) => {
                self.form1.is_authenticating = auth;
            }
            MigrationAction::SetLoginResponse(response) => {
                self.form1.login_response = response;
            }
            MigrationAction::SetSessionStored(stored) => {
                self.form1.session_stored = stored;
            }
            MigrationAction::SetOriginalHandle(handle) => {
                self.form1.original_handle = handle;
            }

            // Form 2 actions
            MigrationAction::SetNewPdsUrl(url) => {
                self.form2.pds_url = url;
            }
            MigrationAction::SetForm2Submitted(submitted) => {
                self.form2.submitted = submitted;
            }
            MigrationAction::SetPdsDescribeResponse(response) => {
                self.form2.describe_response = response;
            }
            MigrationAction::SetDescribingPds(describing) => {
                self.form2.is_describing = describing;
            }

            // Form 3 actions
            MigrationAction::SetNewHandle(handle) => {
                self.form3.handle = handle;
            }
            MigrationAction::SetNewPassword(password) => {
                self.form3.password = password;
            }
            MigrationAction::SetNewPasswordConfirm(password) => {
                self.form3.password_confirm = password;
            }
            MigrationAction::SetEmailAddress(email) => {
                self.form3.email = email;
            }
            MigrationAction::SetInviteCode(code) => {
                self.form3.invite_code = code;
            }
            MigrationAction::SetSelectedDomain(domain) => {
                self.form3.selected_domain = Some(domain);
            }

            // Form 4 - PLC Verification actions
            MigrationAction::SetPlcVerificationCode(code) => {
                self.form4.verification_code = code;
            }
            MigrationAction::SetPlcUnsigned(unsigned) => {
                self.form4.plc_unsigned = unsigned;
            }
            MigrationAction::SetPlcVerifying(verifying) => {
                self.form4.is_verifying = verifying;
            }

            // Validation actions
            MigrationAction::SetHandleValidation(validation) => {
                self.validations.handle = validation;
            }
            MigrationAction::SetCheckingHandle(checking) => {
                // This should likely update the form3.is_checking_handle field instead
                self.form3.is_checking_handle = checking;
            }

            // Migration process actions
            MigrationAction::SetMigrating(migrating) => {
                crate::console_info!(
                    "[REDUCER] SetMigrating reducer entered with value: {} - timestamp: {}",
                    migrating,
                    js_sys::Date::now()
                );

                let old_value = self.is_migrating;
                self.is_migrating = migrating;

                crate::console_info!(
                    "[STATE] Migration state changing: is_migrating={} -> {} - timestamp: {}",
                    old_value,
                    migrating,
                    js_sys::Date::now()
                );

                crate::console_info!("[REDUCER] SetMigrating reducer completed successfully - final is_migrating: {}", 
                    self.is_migrating);
            }
            MigrationAction::SetMigrationError(error) => {
                self.migration_error = error;
            }
            MigrationAction::SetMigrationStep(step) => {
                self.migration_step = step;
            }
            MigrationAction::SetNewPdsSession(session) => {
                self.new_pds_session = session;
            }
            MigrationAction::SetCurrentStep(step) => {
                self.current_step = step;
            }

            // Extended migration progress tracking
            MigrationAction::SetMigrationProgress(progress) => {
                self.migration_progress = progress;
            }
            MigrationAction::SetRepoProgress(progress) => {
                self.repo_progress = progress;
                self.update_unified_blob_progress_cache();
            }
            MigrationAction::SetBlobProgress(progress) => {
                crate::console_debug!("[BLOB] Progress state updated: total={}, processed={}, total_bytes={}, processed_bytes={}", 
                    progress.total_blobs, progress.processed_blobs, progress.total_bytes, progress.processed_bytes);
                self.blob_progress = progress;
                self.update_unified_blob_progress_cache();
            }
            MigrationAction::SetPreferencesProgress(progress) => {
                self.preferences_progress = progress;
            }
            MigrationAction::SetPlcProgress(progress) => {
                self.plc_progress = progress;
            }
            MigrationAction::SetMigrationCompleted(completed) => {
                let old_value = self.migration_completed;
                self.migration_completed = completed;
                crate::console_info!("[STATE] Migration completion changing: migration_completed={} -> {} - timestamp: {}", 
                    old_value, completed, js_sys::Date::now());
            }

            // PLC recommendation storage
            MigrationAction::SetPlcRecommendation(recommendation) => {
                self.plc_recommendation = recommendation;
            }
            // Original PDS describe response cache
            MigrationAction::SetOriginalPdsDescribe(describe) => {
                self.original_pds_describe = describe;
            }
            MigrationAction::AddConsoleMessage(message) => {
                self.console_messages.push_back(message);
                // Keep only the most recent 10 messages
                while self.console_messages.len() > 10 {
                    self.console_messages.pop_front();
                }
            }
        }
    }

    /// Helper methods for common state queries
    pub fn session_stored(&self) -> bool {
        self.form1.session_stored
    }

    pub fn form2_submitted(&self) -> bool {
        self.form2.submitted
    }

    pub fn should_show_form2(&self) -> bool {
        self.session_stored()
    }

    pub fn should_show_form3(&self) -> bool {
        self.session_stored() && self.form2_submitted()
    }

    pub fn should_show_form4(&self) -> bool {
        self.current_step == FormStep::PlcVerification
    }

    pub fn migration_percentage(&self) -> f64 {
        let completed_steps = [
            self.migration_progress.repo_exported,
            self.migration_progress.repo_imported,
            self.migration_progress.missing_blobs_checked,
            self.migration_progress.blobs_exported,
            self.migration_progress.blobs_imported,
            self.migration_progress.preferences_exported,
            self.migration_progress.preferences_imported,
            self.migration_progress.plc_recommended,
            self.migration_progress.plc_token_requested,
            self.migration_progress.plc_signed,
            self.migration_progress.plc_submitted,
            self.migration_progress.new_account_activated,
            self.migration_progress.old_account_deactivated,
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        (completed_steps as f64 / 13.0) * 100.0
    }

    pub fn blob_progress_percentage(&self) -> f64 {
        if self.blob_progress.total_blobs == 0 {
            0.0
        } else {
            (self.blob_progress.processed_blobs as f64 / self.blob_progress.total_blobs as f64)
                * 100.0
        }
    }

    /// Calculate a cache key for unified_blob_progress based on relevant fields
    fn calculate_blob_progress_cache_key(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash the fields that affect unified_blob_progress calculation
        self.blob_progress.total_blobs.hash(&mut hasher);
        self.blob_progress.processed_blobs.hash(&mut hasher);
        self.blob_progress.total_bytes.hash(&mut hasher);
        self.blob_progress.processed_bytes.hash(&mut hasher);
        self.repo_progress.car_size.hash(&mut hasher);

        hasher.finish()
    }

    /// Get unified blob progress that accounts for both repository and dedicated blob migration
    /// Repository migration processes embedded blobs, while blob migration handles missing ones
    /// Uses memoization to avoid expensive calculations on every call
    pub fn unified_blob_progress(&self) -> BlobProgress {
        // DEBUG: Log blob progress state for troubleshooting UI freeze
        crate::console_log!(
            "[DEBUG unified_blob_progress] blob_progress: {}/{} blobs, {}/{} bytes",
            self.blob_progress.processed_blobs,
            self.blob_progress.total_blobs,
            self.blob_progress.processed_bytes,
            self.blob_progress.total_bytes
        );

        // Check if we can use cached result
        let current_cache_key = self.calculate_blob_progress_cache_key();
        if let Some(ref cached) = self.cached_unified_blob_progress {
            if self.blob_progress_cache_key == current_cache_key {
                crate::console_log!("[DEBUG unified_blob_progress] Using cached result");
                return cached.clone();
            }
        }

        crate::console_log!("[DEBUG unified_blob_progress] Cache miss, calculating...");

        // Cache miss - perform the calculation
        // Use the existing blob_progress which gets updated during both phases
        // Repository phase: estimates blobs from streamed data
        // Blob phase: uses actual blob counts and continues from where repository left off
        let mut unified = self.blob_progress.clone();

        // If we're in repository migration phase and have repo progress, ensure we show something
        if unified.total_blobs == 0
            && unified.processed_blobs == 0
            && self.repo_progress.car_size > 0
        {
            // Estimate blobs from repository size as fallback
            let estimated_blobs = std::cmp::max(1, (self.repo_progress.car_size / 10_000) as u32);
            unified.total_blobs = estimated_blobs;
            unified.processed_blobs = estimated_blobs;
            unified.total_bytes = self.repo_progress.car_size;
            unified.processed_bytes = self.repo_progress.car_size;
        }

        // Note: We can't update the cache here because self is immutable
        // The cache will be updated in the reducer when state changes
        unified
    }

    /// Update the unified blob progress cache - called from the reducer when relevant state changes
    pub fn update_unified_blob_progress_cache(&mut self) {
        let current_cache_key = self.calculate_blob_progress_cache_key();
        if self.blob_progress_cache_key != current_cache_key {
            // Recalculate and cache the result
            let mut unified = self.blob_progress.clone();

            if unified.total_blobs == 0
                && unified.processed_blobs == 0
                && self.repo_progress.car_size > 0
            {
                let estimated_blobs =
                    std::cmp::max(1, (self.repo_progress.car_size / 10_000) as u32);
                unified.total_blobs = estimated_blobs;
                unified.processed_blobs = estimated_blobs;
                unified.total_bytes = self.repo_progress.car_size;
                unified.processed_bytes = self.repo_progress.car_size;
            }

            self.cached_unified_blob_progress = Some(unified);
            self.blob_progress_cache_key = current_cache_key;
        }
    }

    /// Check if we should display blob progress based on migration state
    pub fn should_show_blob_progress(&self) -> bool {
        crate::console_info!("[BLOB] should_show_blob_progress() called - evaluating conditions");

        let unified = self.unified_blob_progress();

        let has_blobs = unified.total_blobs > 0;
        let has_blob_step = self.migration_step.contains("blob");
        let has_repo_step = self.migration_step.contains("repository");
        let has_streaming_step = self.migration_step.contains("streaming");
        let is_migrating = self.is_migrating;
        let migration_completed = self.migration_completed;

        let should_show =
            has_blobs || has_blob_step || has_repo_step || has_streaming_step || is_migrating;

        crate::console_info!("[BLOB] should_show_blob_progress decision: show={}, has_blobs={}, has_blob_step={}, has_repo_step={}, has_streaming_step={}, is_migrating={}, migration_completed={}, step='{}'", 
            should_show, has_blobs, has_blob_step, has_repo_step, has_streaming_step, is_migrating, migration_completed, self.migration_step);

        should_show
    }
}

impl Default for LoginForm {
    fn default() -> Self {
        Self {
            handle: String::new(),
            password: String::new(),
            provider: ClientPdsProvider::None,
            is_loading: false,
            is_authenticating: false,
            login_response: None,
            session_stored: false,
            original_handle: String::new(),
        }
    }
}

impl Default for ValidationStates {
    fn default() -> Self {
        Self {
            handle: HandleValidation::None,
            password: PasswordValidation::None,
            email: EmailValidation::None,
        }
    }
}

impl Default for MigrationState {
    fn default() -> Self {
        Self {
            current_step: FormStep::Login,
            form1: LoginForm::default(),
            form2: PdsSelectionForm::default(),
            form3: MigrationDetailsForm::default(),
            form4: PlcVerificationForm::default(),
            validations: ValidationStates::default(),
            is_migrating: false,
            migration_error: None,
            migration_step: String::new(),
            new_pds_session: None,
            migration_progress: MigrationProgress::default(),
            repo_progress: RepoProgress::default(),
            blob_progress: BlobProgress::default(),
            preferences_progress: PreferencesProgress::default(),
            plc_progress: PlcProgress::default(),
            migration_completed: false,
            plc_recommendation: None,
            original_pds_describe: None,
            console_messages: VecDeque::new(),
            cached_unified_blob_progress: None,
            blob_progress_cache_key: 0,
        }
    }
}

// Type alias for dispatch function
pub type DispatchFn = Box<dyn Fn(MigrationAction) + 'static>;

/// Helper function to serialize u64 as string to avoid BigInt serialization issues in WASM
fn serialize_u64_as_string<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}
