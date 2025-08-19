use crate::migration::{
    EmailValidation, HandleValidation, MigrationState, PasswordValidation,
};

impl MigrationState {
    pub fn validate_passwords(&self) -> PasswordValidation {
        if self.form3.password.is_empty() && self.form3.password_confirm.is_empty() {
            PasswordValidation::None
        } else if self.form3.password == self.form3.password_confirm
            && !self.form3.password.is_empty()
        {
            PasswordValidation::Match
        } else {
            PasswordValidation::NoMatch
        }
    }

    pub fn validate_email(&self) -> EmailValidation {
        let email = self.form3.email.trim();
        if email.is_empty() {
            return EmailValidation::None;
        }

        // Basic email validation: must contain exactly one @ and at least one . after @
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 {
            return EmailValidation::Invalid;
        }

        let local_part = parts[0];
        let domain_part = parts[1];

        // Local part should not be empty and domain should contain at least one dot
        if !local_part.is_empty() && domain_part.contains('.') && domain_part.len() > 2 {
            EmailValidation::Valid
        } else {
            EmailValidation::Invalid
        }
    }
}

pub fn validation_class(validation: &HandleValidation) -> &'static str {
    match validation {
        HandleValidation::Available => "input-field input-available",
        HandleValidation::Unavailable => "input-field input-unavailable",
        HandleValidation::Error => "input-field input-error",
        _ => "input-field",
    }
}

pub fn validation_style(validation: &HandleValidation) -> &'static str {
    match validation {
        HandleValidation::Available => "border: 2px solid #10b981; background-color: #f0fdf4;",
        HandleValidation::Unavailable => "border: 2px solid #ef4444; background-color: #fef2f2;",
        HandleValidation::Error => "border: 2px solid #f59e0b; background-color: #fffbeb;",
        _ => "",
    }
}

pub fn password_validation_class(validation: &PasswordValidation) -> &'static str {
    match validation {
        PasswordValidation::Match => "input-field input-valid",
        PasswordValidation::NoMatch => "input-field input-invalid",
        _ => "input-field",
    }
}

pub fn password_validation_style(validation: &PasswordValidation) -> &'static str {
    match validation {
        PasswordValidation::Match => "border: 2px solid #10b981; background-color: #f0fdf4;",
        PasswordValidation::NoMatch => "border: 2px solid #ef4444; background-color: #fef2f2;",
        _ => "",
    }
}

pub fn email_validation_class(validation: &EmailValidation) -> &'static str {
    match validation {
        EmailValidation::Valid => "input-field input-valid",
        EmailValidation::Invalid => "input-field input-invalid",
        _ => "input-field",
    }
}

pub fn email_validation_style(validation: &EmailValidation) -> &'static str {
    match validation {
        EmailValidation::Valid => "border: 2px solid #10b981; background-color: #f0fdf4;",
        EmailValidation::Invalid => "border: 2px solid #ef4444; background-color: #fef2f2;",
        _ => "",
    }
}
