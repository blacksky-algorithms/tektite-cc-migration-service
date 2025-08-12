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
