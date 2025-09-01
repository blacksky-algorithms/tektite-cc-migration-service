use crate::migration::storage::LocalStorageManager;
use crate::migration::types::PdsDescribeResponse;
use crate::migration::{MigrationAction, MigrationState};
use dioxus::prelude::EventHandler;

#[cfg(feature = "web")]
use crate::services::client::compat::describe_server;

impl MigrationState {
    /// Generate a smart handle suggestion based on the original handle and PDS response
    pub fn suggest_handle(&self) -> Option<String> {
        // Only suggest if form2 is submitted and we have a PDS describe response
        if !self.form2_submitted() {
            return None;
        }

        let describe_response = self.form2.describe_response.as_ref()?;
        let available_domains = &describe_response.available_user_domains;
        let suggested_domain = available_domains.first()?;
        let original = &self.form1.original_handle;

        if original.is_empty() {
            return None;
        }

        // Check if the original handle matches any of the available user domains
        let matching_domain = available_domains
            .iter()
            .find(|domain| original.ends_with(domain.as_str()));

        let suggestion = if let Some(matched_domain) = matching_domain {
            // Handle has suffix matching availableUserDomains (e.g., jaz.bsky.social, tektiteb.blacksky.app)
            let prefix = original.trim_end_matches(matched_domain);
            if !prefix.is_empty() && !prefix.starts_with("did:") {
                format!("{}{}", prefix, suggested_domain)
            } else {
                format!("your_username{}", suggested_domain)
            }
        } else if original.contains('.') && !original.starts_with("did:") {
            // Handle is a fully qualified domain name (FQDN) resolved via DNS TXT record
            // Transform torrho.com -> torrho_com.blacksky.app
            let underscore_handle = original.replace('.', "_");
            format!("{}{}", underscore_handle, suggested_domain)
        } else {
            // Fallback for other cases (DID, etc.)
            format!("your_username{}", suggested_domain)
        };

        Some(suggestion)
    }

    /// Check if the original handle is a custom domain requiring DNS setup
    ///
    /// This distinguishes between:
    /// - PDS subdomains (e.g., "tektitef5.bsky.social" on PDS offering ".bsky.social") - returns false
    /// - True custom domains (e.g., "slavecodes.org" DNS-verified) - returns true
    pub fn is_original_handle_fqdn(&self) -> bool {
        let original = &self.form1.original_handle;

        if original.is_empty() || original.starts_with("did:") {
            return false;
        }

        // Must contain a dot to be a domain
        if !original.contains('.') {
            return false;
        }

        // Check if we have a PDS response with available domains
        if let Some(describe_response) = &self.form2.describe_response {
            let available_domains = &describe_response.available_user_domains;

            // Check if the handle is a subdomain of any availableUserDomains
            // E.g., "tektitef5.bsky.social" matches ".bsky.social" domain
            for domain in available_domains {
                let domain_suffix = domain.trim();
                let original_lower = original.trim().to_lowercase();

                // Handle domain suffix with or without leading dot
                let normalized_suffix = if domain_suffix.starts_with('.') {
                    domain_suffix.to_lowercase()
                } else {
                    format!(".{}", domain_suffix.to_lowercase())
                };

                // If original handle ends with this domain suffix, it's a PDS subdomain, not a custom domain
                if original_lower.ends_with(&normalized_suffix) {
                    // Additional check: ensure it's actually a subdomain (has prefix)
                    let prefix = original_lower.trim_end_matches(&normalized_suffix);
                    if !prefix.is_empty() && !prefix.contains('.') {
                        // This is a simple subdomain like "username.bsky.social"
                        return false;
                    }
                }
            }

            // If we get here, the handle doesn't match any availableUserDomains
            // This indicates it's likely a custom domain that requires DNS verification
            return true;
        }

        // Fallback: if no PDS response, assume domains with dots are custom
        true
    }

    /// Get all available domain suffixes from PDS
    pub fn get_available_domains(&self) -> Vec<String> {
        if let Some(describe_response) = &self.form2.describe_response {
            return describe_response.available_user_domains.clone();
        }
        vec![".newpds.social".to_string()] // fallback
    }

    /// Get the currently selected domain suffix for the new handle
    pub fn get_domain_suffix(&self) -> String {
        // Use selected domain if available
        if let Some(selected) = &self.form3.selected_domain {
            return selected.clone();
        }

        // Otherwise use first available domain as default
        if let Some(describe_response) = &self.form2.describe_response {
            if let Some(domain) = describe_response.available_user_domains.first() {
                return domain.clone();
            }
        }
        ".newpds.social".to_string() // fallback
    }

    /// Get the raw prefix without any domain
    pub fn get_handle_prefix_raw(&self) -> String {
        let full_handle = &self.form3.handle;
        let available_domains = self.get_available_domains();

        // Check if the handle ends with any of the available domains
        for domain in &available_domains {
            if full_handle.ends_with(domain) {
                return full_handle.trim_end_matches(domain).to_string();
            }
        }

        // If no match, return the handle as-is (might be just the prefix)
        full_handle.clone()
    }

    /// Extract the prefix from the current form3 handle (removing the domain suffix)
    pub fn get_handle_prefix(&self) -> String {
        let full_handle = &self.form3.handle;
        let domain_suffix = self.get_domain_suffix();

        if full_handle.ends_with(&domain_suffix) {
            full_handle.trim_end_matches(&domain_suffix).to_string()
        } else {
            // If current handle doesn't match selected domain, return raw prefix
            self.get_handle_prefix_raw()
        }
    }

    /// Generate a placeholder for the handle prefix input
    pub fn get_handle_prefix_placeholder(&self) -> String {
        // Use cached original PDS describe response if available
        let placeholder = self.extract_username_with_original_pds(&self.original_pds_describe);

        // If that didn't work, fall back to the original suggestion logic
        if placeholder == "your_username" {
            if let Some(suggestion) = self.suggest_handle() {
                let domain_suffix = self.get_domain_suffix();
                if suggestion.ends_with(&domain_suffix) {
                    return suggestion.trim_end_matches(&domain_suffix).to_string();
                }
            }
        }

        placeholder
    }

    /// Enhanced async placeholder that checks original PDS domains  
    #[cfg(feature = "web")]
    pub async fn get_handle_prefix_placeholder_async(
        &self,
        dispatch: Option<EventHandler<MigrationAction>>,
    ) -> String {
        // Try to get original PDS information if not cached
        let original_pds_describe = if let Some(describe) = &self.original_pds_describe {
            Some(describe.clone())
        } else {
            self.fetch_and_cache_original_pds_describe(dispatch).await
        };

        // Extract username using original PDS logic
        self.extract_username_with_original_pds(&original_pds_describe)
    }

    /// Fetch original PDS describe response and cache it
    #[cfg(feature = "web")]
    async fn fetch_and_cache_original_pds_describe(
        &self,
        dispatch: Option<EventHandler<MigrationAction>>,
    ) -> Option<PdsDescribeResponse> {
        // Get original PDS URL from session
        let original_pds_url = self.get_original_pds_url()?;

        // Fetch describe response
        match describe_server(original_pds_url).await {
            Ok(server_info) => {
                match serde_json::from_value::<PdsDescribeResponse>(server_info) {
                    Ok(describe_response) => {
                        // Cache the response if dispatch is available
                        if let Some(dispatch) = dispatch {
                            dispatch.call(MigrationAction::SetOriginalPdsDescribe(Some(
                                describe_response.clone(),
                            )));
                        }
                        Some(describe_response)
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }

    /// Get original PDS URL from session
    fn get_original_pds_url(&self) -> Option<String> {
        // Try from login response first
        if let Some(login_response) = &self.form1.login_response {
            if let Some(session) = &login_response.session {
                return Some(session.pds.clone());
            }
        }

        // Fallback to stored session
        if let Ok(old_session) = LocalStorageManager::get_old_session() {
            return Some(old_session.pds);
        }

        None
    }

    /// Extract username using original PDS domain knowledge
    fn extract_username_with_original_pds(
        &self,
        original_pds_describe: &Option<PdsDescribeResponse>,
    ) -> String {
        let original_handle = &self.form1.original_handle;

        // Handle DID cases
        if original_handle.starts_with("did:") {
            return "your_username".to_string();
        }

        // If we have original PDS domains, check if handle uses them
        if let Some(describe) = original_pds_describe {
            for domain in &describe.available_user_domains {
                if original_handle.ends_with(domain) {
                    let prefix = original_handle.trim_end_matches(domain);
                    if !prefix.is_empty() && is_valid_username_prefix(prefix) {
                        return prefix.to_string();
                    }
                }
            }
        }

        // Handle is likely custom DNS or doesn't match original PDS domains
        if original_handle.contains('.') {
            let parts: Vec<&str> = original_handle.split('.').collect();
            if let Some(first_part) = parts.first() {
                if !first_part.is_empty() && is_valid_username_prefix(first_part) {
                    return first_part.to_string();
                }
            }
        }

        // No dots or invalid format - might already be a username
        if !original_handle.is_empty() && is_valid_username_prefix(original_handle) {
            return original_handle.to_string();
        }

        // Fallback to current logic
        self.get_handle_prefix_placeholder()
    }

    /// Generate a fallback placeholder for handle input (legacy - kept for compatibility)
    pub fn handle_placeholder(&self) -> String {
        // Use the smart suggestion if available, otherwise fallback to generic
        if let Some(suggestion) = self.suggest_handle() {
            suggestion
        } else if let Some(describe_response) = &self.form2.describe_response {
            if let Some(domain) = describe_response.available_user_domains.first() {
                format!("your_username{}", domain)
            } else {
                "newhandle.newpds.social".to_string()
            }
        } else {
            "newhandle.newpds.social".to_string()
        }
    }
}

/// Validate if a string looks like a valid username prefix
fn is_valid_username_prefix(prefix: &str) -> bool {
    if prefix.is_empty() || prefix.len() < 2 || prefix.len() > 50 {
        return false;
    }

    // Must start with alphanumeric
    if !prefix.chars().next().unwrap().is_alphanumeric() {
        return false;
    }

    // Allow alphanumeric plus common username characters
    prefix
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}
