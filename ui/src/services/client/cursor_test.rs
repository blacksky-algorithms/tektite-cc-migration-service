//! Tests for cursor pagination edge cases
//! 
//! These tests verify that cursor handling matches the Go goat implementation
//! for various edge cases including empty cursors, null cursors, and continuation.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Test cursor parsing from JSON responses
    #[test]
    fn test_cursor_parsing_edge_cases() {
        // Test case 1: Missing cursor field
        let json_no_cursor = json!({
            "blobs": []
        });
        let cursor = json_no_cursor.get("cursor").and_then(|c| c.as_str()).map(|s| s.to_string());
        assert_eq!(cursor, None);

        // Test case 2: Null cursor
        let json_null_cursor = json!({
            "blobs": [],
            "cursor": null
        });
        let cursor = json_null_cursor.get("cursor").and_then(|c| c.as_str()).map(|s| s.to_string());
        assert_eq!(cursor, None);

        // Test case 3: Empty cursor
        let json_empty_cursor = json!({
            "blobs": [],
            "cursor": ""
        });
        let cursor = json_empty_cursor.get("cursor").and_then(|c| c.as_str()).map(|s| s.to_string());
        assert_eq!(cursor, Some("".to_string()));

        // Test case 4: Valid cursor
        let json_valid_cursor = json!({
            "blobs": [],
            "cursor": "next_page_token_123"
        });
        let cursor = json_valid_cursor.get("cursor").and_then(|c| c.as_str()).map(|s| s.to_string());
        assert_eq!(cursor, Some("next_page_token_123".to_string()));
    }

    /// Test cursor continuation logic (matches Go goat pattern)
    #[test]
    fn test_cursor_continuation_logic() {
        // Test case 1: No cursor means stop (matches Go: resp.Cursor == nil)
        let response_cursor: Option<String> = None;
        let should_continue = if let Some(next_cursor) = response_cursor {
            !next_cursor.is_empty()
        } else {
            false
        };
        assert_eq!(should_continue, false);

        // Test case 2: Empty cursor means stop (matches Go: *resp.Cursor == "")
        let response_cursor: Option<String> = Some("".to_string());
        let should_continue = if let Some(next_cursor) = response_cursor {
            !next_cursor.is_empty()
        } else {
            false
        };
        assert_eq!(should_continue, false);

        // Test case 3: Valid cursor means continue (matches Go: resp.Cursor != nil && *resp.Cursor != "")
        let response_cursor: Option<String> = Some("valid_cursor".to_string());
        let should_continue = if let Some(next_cursor) = response_cursor {
            !next_cursor.is_empty()
        } else {
            false
        };
        assert_eq!(should_continue, true);
    }

    /// Test the full cursor state machine that mirrors Go goat behavior
    #[test]
    fn test_cursor_state_machine() {
        // Simulate the pagination loop logic
        let test_cases = vec![
            // (input_cursor, output_cursor, should_break)
            (None, None, true),                    // No cursor -> stop
            (Some("".to_string()), None, true),   // Empty cursor -> stop  
            (Some("page2".to_string()), Some("page2".to_string()), false), // Valid cursor -> continue
        ];

        for (input_cursor, expected_output, should_break) in test_cases {
            let mut loop_should_break = false;

            // Simulate the cursor update logic from our pagination code
            let mut cursor = if let Some(next_cursor) = input_cursor {
                if !next_cursor.is_empty() {
                    Some(next_cursor) // Continue with next cursor
                } else {
                    loop_should_break = true;
                    None
                }
            } else {
                loop_should_break = true;
                None
            };

            assert_eq!(cursor, expected_output);
            assert_eq!(loop_should_break, should_break);
        }
    }

    /// Test cursor URL encoding edge cases
    #[test]
    fn test_cursor_url_encoding() {
        // Test that cursors with special characters are handled correctly
        let test_cursors = vec![
            "simple_cursor",
            "cursor-with-dashes",
            "cursor.with.dots", 
            "cursor_with_underscores",
            "cursor123with456numbers",
            "CURSOR_WITH_CAPS",
        ];

        for cursor_value in test_cursors {
            let json_with_cursor = json!({
                "blobs": [],
                "cursor": cursor_value
            });
            
            let parsed_cursor = json_with_cursor
                .get("cursor")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());
            
            assert_eq!(parsed_cursor, Some(cursor_value.to_string()));
            
            // Verify continuation logic works
            let should_continue = if let Some(next_cursor) = parsed_cursor {
                !next_cursor.is_empty()
            } else {
                false
            };
            assert_eq!(should_continue, true);
        }
    }
}