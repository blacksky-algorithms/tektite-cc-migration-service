//! Input components for form validation and display

use crate::migration::{EmailValidation, HandleValidation, PasswordValidation};
use dioxus::prelude::*;

#[derive(PartialEq, Clone, Debug)]
pub enum InputType {
    Text,
    Password,
    Email,
}

impl InputType {
    pub fn as_str(&self) -> &'static str {
        match self {
            InputType::Text => "text",
            InputType::Password => "password",
            InputType::Email => "email",
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct ValidatedInputProps {
    pub value: String,
    pub placeholder: String,
    pub input_type: InputType,
    pub input_class: String,
    pub input_style: String,
    pub disabled: bool,
    pub on_change: EventHandler<String>,
}

#[component]
pub fn ValidatedInput(props: ValidatedInputProps) -> Element {
    rsx! {
        input {
            class: "{props.input_class}",
            style: "{props.input_style}",
            r#type: "{props.input_type.as_str()}",
            value: "{props.value}",
            placeholder: "{props.placeholder}",
            disabled: props.disabled,
            oninput: move |event| props.on_change.call(event.value())
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct HandleValidationFeedbackProps {
    pub validation: HandleValidation,
    pub is_checking: bool,
}

#[component]
pub fn HandleValidationFeedback(props: HandleValidationFeedbackProps) -> Element {
    if props.is_checking {
        rsx! {
            div {
                class: "validation-feedback checking",
                "⏳ Checking availability..."
            }
        }
    } else {
        match props.validation {
            HandleValidation::Available => rsx! {
                div {
                    class: "validation-feedback available",
                    style: "color: #10b981; background-color: #d1fae5; border: 1px solid #10b981; padding: 8px; border-radius: 4px; margin-top: 4px;",
                    "✓ Handle is available!"
                }
            },
            HandleValidation::Unavailable => rsx! {
                div {
                    class: "validation-feedback unavailable",
                    style: "color: #ef4444; background-color: #fef2f2; border: 1px solid #ef4444; padding: 8px; border-radius: 4px; margin-top: 4px;",
                    "⚠ Handle is not available - please choose a different name"
                }
            },
            HandleValidation::Error => rsx! {
                div {
                    class: "validation-feedback error",
                    style: "color: #f59e0b; background-color: #fffbeb; border: 1px solid #f59e0b; padding: 8px; border-radius: 4px; margin-top: 4px;",
                    "⚠ Error checking availability - please try again"
                }
            },
            _ => rsx! { div {} },
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct PasswordValidationFeedbackProps {
    pub validation: PasswordValidation,
}

#[component]
pub fn PasswordValidationFeedback(props: PasswordValidationFeedbackProps) -> Element {
    match props.validation {
        PasswordValidation::Match => rsx! {
            div {
                class: "validation-feedback match",
                style: "color: #10b981; background-color: #d1fae5; border: 1px solid #10b981; padding: 8px; border-radius: 4px; margin-top: 4px;",
                "✓ Passwords match"
            }
        },
        PasswordValidation::NoMatch => rsx! {
            div {
                class: "validation-feedback no-match",
                style: "color: #ef4444; background-color: #fef2f2; border: 1px solid #ef4444; padding: 8px; border-radius: 4px; margin-top: 4px;",
                "⚠ Passwords do not match"
            }
        },
        _ => rsx! { div {} },
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct EmailValidationFeedbackProps {
    pub validation: EmailValidation,
}

#[component]
pub fn EmailValidationFeedback(props: EmailValidationFeedbackProps) -> Element {
    match props.validation {
        EmailValidation::Valid => rsx! {
            div {
                class: "validation-feedback valid",
                style: "color: #10b981; background-color: #d1fae5; border: 1px solid #10b981; padding: 8px; border-radius: 4px; margin-top: 4px;",
                "✓ Valid email address"
            }
        },
        EmailValidation::Invalid => rsx! {
            div {
                class: "validation-feedback invalid",
                style: "color: #ef4444; background-color: #fef2f2; border: 1px solid #ef4444; padding: 8px; border-radius: 4px; margin-top: 4px;",
                "⚠ Please enter a valid email address"
            }
        },
        _ => rsx! { div {} },
    }
}
