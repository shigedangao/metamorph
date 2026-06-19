use serde::Deserialize;
use std::collections::HashMap;

// These structs are used after deserialization due to the dynamic endpoint structure after the flatten.
#[derive(Debug, Deserialize)]
pub struct BenchEndpointComponent {
    pub from: Endpoint,
    pub target: Endpoint,
}

#[derive(Debug, Deserialize)]
pub struct Endpoint {
    endpoint: String,
    params: HashMap<String, String>,
    pub check_path: Option<CheckPath>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CheckPath {
    pub path: String,
}

impl BenchEndpointComponent {
    pub fn template(&self) -> (String, String) {
        (
            replace_params(&self.from.endpoint, &self.from.params),
            replace_params(&self.target.endpoint, &self.target.params),
        )
    }
}

fn replace_params(template: &str, params: &HashMap<String, String>) -> String {
    let mut result = template.to_string();

    for (key, value) in params {
        result = result.replace(&format!("{{{key}}}"), value);
    }

    result
}
