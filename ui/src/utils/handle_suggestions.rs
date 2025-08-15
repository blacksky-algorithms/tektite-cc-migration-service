use crate::features::migration::MigrationState;

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

    /// Get the domain suffix for the new handle (from PDS availableUserDomains)
    pub fn get_domain_suffix(&self) -> String {
        if let Some(describe_response) = &self.form2.describe_response {
            if let Some(domain) = describe_response.available_user_domains.first() {
                return domain.clone();
            }
        }
        ".newpds.social".to_string() // fallback
    }

    /// Extract the prefix from the current form3 handle (removing the domain suffix)
    pub fn get_handle_prefix(&self) -> String {
        let full_handle = &self.form3.handle;
        let domain_suffix = self.get_domain_suffix();

        if full_handle.ends_with(&domain_suffix) {
            full_handle.trim_end_matches(&domain_suffix).to_string()
        } else {
            full_handle.clone()
        }
    }

    /// Generate a placeholder for the handle prefix input
    pub fn get_handle_prefix_placeholder(&self) -> String {
        if let Some(suggestion) = self.suggest_handle() {
            let domain_suffix = self.get_domain_suffix();
            if suggestion.ends_with(&domain_suffix) {
                suggestion.trim_end_matches(&domain_suffix).to_string()
            } else {
                "your_username".to_string()
            }
        } else {
            "your_username".to_string()
        }
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
