use crate::features::migration::types::*;

/// Validates that all required Form 3 fields are filled and passwords match
pub fn validate_form3_complete(state: &MigrationState) -> bool {
    !state.form3.handle.trim().is_empty()
        && !state.form3.password.trim().is_empty()
        && !state.form3.password_confirm.trim().is_empty()
        && !state.form3.email.trim().is_empty()
        && state.validate_passwords() == PasswordValidation::Match
}

/// Validates that Form 3 handle field has valid availability status
pub fn validate_handle_availability(state: &MigrationState) -> bool {
    match state.validations.handle {
        HandleValidation::Available => true,
        _ => false,
    }
}

/// Validates that the migration can proceed (all required data present)
pub fn validate_migration_ready(state: &MigrationState) -> bool {
    // Form 1: Must have valid session stored
    state.session_stored() &&

    // Form 2: Must have PDS describe response
    state.form2.describe_response.is_some() &&

    // Form 3: All fields valid and handle available
    validate_form3_complete(state) &&
    validate_handle_availability(state)
}

/// Validates that Form 4 PLC verification can proceed
pub fn validate_plc_verification_ready(state: &MigrationState) -> bool {
    !state.form4.verification_code.trim().is_empty() && !state.form4.plc_unsigned.trim().is_empty()
}

/// Gets user-friendly validation message for current form state
pub fn get_form3_validation_message(state: &MigrationState) -> Option<String> {
    if state.form3.handle.trim().is_empty() {
        return Some("Please enter a handle for the new PDS".to_string());
    }

    if state.form3.password.trim().is_empty() {
        return Some("Please enter a new password".to_string());
    }

    if state.form3.password_confirm.trim().is_empty() {
        return Some("Please confirm your password".to_string());
    }

    if state.form3.email.trim().is_empty() {
        return Some("Please enter an email address".to_string());
    }

    match state.validate_passwords() {
        PasswordValidation::NoMatch => Some("Passwords do not match".to_string()),
        PasswordValidation::Match => None,
        _ => Some("Please check your password".to_string()),
    }
}

/// Gets user-friendly validation message for handle availability
pub fn get_handle_validation_message(state: &MigrationState) -> Option<String> {
    match state.validations.handle {
        HandleValidation::Checking => Some("Checking handle availability...".to_string()),
        HandleValidation::Available => Some("✓ Handle is available".to_string()),
        HandleValidation::Unavailable => Some("✗ Handle is already taken".to_string()),
        HandleValidation::Error => Some("✗ Error checking handle availability".to_string()),
        HandleValidation::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_form3_complete() {
        let mut state = MigrationState::default();

        // Should be false with empty fields
        assert!(!validate_form3_complete(&state));

        // Fill in all fields
        state.form3.handle = "testuser".to_string();
        state.form3.password = "testpassword123".to_string();
        state.form3.password_confirm = "testpassword123".to_string();
        state.form3.email = "test@example.com".to_string();

        // Should be true with all fields filled and matching passwords
        assert!(validate_form3_complete(&state));

        // Should be false with mismatched passwords
        state.form3.password_confirm = "different".to_string();
        assert!(!validate_form3_complete(&state));
    }

    #[test]
    fn test_validate_handle_availability() {
        let mut state = MigrationState::default();

        // Should be false with no validation
        assert!(!validate_handle_availability(&state));

        // Should be false with checking status
        state.validations.handle = HandleValidation::Checking;
        assert!(!validate_handle_availability(&state));

        // Should be false with unavailable status
        state.validations.handle = HandleValidation::Unavailable;
        assert!(!validate_handle_availability(&state));

        // Should be true with available status
        state.validations.handle = HandleValidation::Available;
        assert!(validate_handle_availability(&state));
    }

    #[test]
    fn test_validate_plc_verification_ready() {
        let mut state = MigrationState::default();

        // Should be false with empty fields
        assert!(!validate_plc_verification_ready(&state));

        // Should be false with only verification code
        state.form4.verification_code = "ABC12-345DE".to_string();
        assert!(!validate_plc_verification_ready(&state));

        // Should be true with both fields
        state.form4.plc_unsigned = "unsigned_plc_operation".to_string();
        assert!(validate_plc_verification_ready(&state));
    }
}
