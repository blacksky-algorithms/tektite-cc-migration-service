// Core types for Migration Service - no dioxus imports needed here
use serde::{Deserialize, Serialize, Serializer};

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

// Migration checkpoint management for resumption
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum MigrationCheckpoint {
    AccountCreated,      // Account exists, need repo migration
    RepoMigrated,        // Repo imported, need blob migration
    BlobsMigrated,       // Blobs imported, need preferences migration
    PreferencesMigrated, // Preferences migrated, need PLC operations
    PlcReady,            // Ready for Form 4 transition
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
                self.is_migrating = migrating;
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
            }
            MigrationAction::SetBlobProgress(progress) => {
                self.blob_progress = progress;
            }
            MigrationAction::SetPreferencesProgress(progress) => {
                self.preferences_progress = progress;
            }
            MigrationAction::SetPlcProgress(progress) => {
                self.plc_progress = progress;
            }
            MigrationAction::SetMigrationCompleted(completed) => {
                self.migration_completed = completed;
            }
            MigrationAction::SetPlcRecommendation(recommendation) => {
                self.plc_recommendation = recommendation;
            }
        }
        self
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
