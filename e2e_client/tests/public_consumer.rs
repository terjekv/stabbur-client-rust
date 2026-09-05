//! Compile and behavior checks from an independent downstream crate.

use stabbur_client::{BaseUrl, SecretToken};

#[test]
fn independent_consumer_can_use_public_foundation_types() {
    let base = BaseUrl::parse("https://stabbur.example.invalid").unwrap();
    let token = SecretToken::new("fixture-token").unwrap();
    assert_eq!(base.to_string(), "https://stabbur.example.invalid");
    assert!(!format!("{token:?}").contains("fixture-token"));
}
