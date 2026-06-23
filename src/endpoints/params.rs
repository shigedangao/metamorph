use serde::Deserialize;
use std::collections::HashMap;

// Constant
const BODY_KEY: &str = "args";

// These structs are used after deserialization due to the dynamic endpoint structure after the flatten.
#[derive(Debug, Deserialize)]
pub struct BenchEndpointComponent {
    pub from: Endpoint,
    pub target: Endpoint,
}

/// A parsed endpoint component containing the source and target endpoints.
#[derive(Debug, Deserialize, Default, Clone)]
pub enum SupportedMethod {
    #[default]
    Get,
    Post,
}

/// A parsed endpoint containing the endpoint URL, method, and parameters.
#[derive(Debug, Deserialize)]
pub struct Endpoint {
    endpoint: String,
    #[serde(default)]
    pub method: SupportedMethod,
    params: HashMap<String, String>,
    pub check_path: Option<String>,
    pub reconcile_path: Option<String>,
}

impl BenchEndpointComponent {
    /// Returns the source and target endpoint templates with parameters replaced.
    pub fn template(&self) -> (String, String) {
        (
            replace_params(&self.from.endpoint, &self.from.params),
            replace_params(&self.target.endpoint, &self.target.params),
        )
    }

    /// Returns the source and target endpoint bodies with parameters replaced.
    pub fn get_body(&self) -> (Option<String>, Option<String>) {
        let from_body = self.from.params.get(BODY_KEY);
        let target_body = self.target.params.get(BODY_KEY);

        match from_body.zip(target_body) {
            Some((from, target)) => (Some(from.to_owned()), Some(target.to_owned())),
            None => (None, None),
        }
    }
}

/// Replaces the parameters in the given template with the values from the given parameters map.
///
/// # Arguments
///
/// * `template` - The template string to replace parameters in.
/// * `params` - A map of parameter names to values to replace in the template.
fn replace_params(template: &str, params: &HashMap<String, String>) -> String {
    let mut result = template.to_string();

    for (key, value) in params {
        result = result.replace(&format!("{{{key}}}"), value);
    }

    result
}
