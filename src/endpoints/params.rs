use serde::Deserialize;
use std::collections::HashMap;

const BODY_KEY: &'static str = "args";

// These structs are used after deserialization due to the dynamic endpoint structure after the flatten.
#[derive(Debug, Deserialize)]
pub struct BenchEndpointComponent {
    pub from: Endpoint,
    pub target: Endpoint,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub enum SupportedMethod {
    #[default]
    Get,
    Post,
}

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
    pub fn template(&self) -> (String, String) {
        (
            replace_params(&self.from.endpoint, &self.from.params),
            replace_params(&self.target.endpoint, &self.target.params),
        )
    }

    pub fn get_body(&self) -> (Option<String>, Option<String>) {
        let from_body = self.from.params.get(BODY_KEY);
        let target_body = self.target.params.get(BODY_KEY);

        match from_body.zip(target_body) {
            Some((from, target)) => (Some(from.to_owned()), Some(target.to_owned())),
            None => (None, None),
        }
    }
}

fn replace_params(template: &str, params: &HashMap<String, String>) -> String {
    let mut result = template.to_string();

    for (key, value) in params {
        result = result.replace(&format!("{{{key}}}"), value);
    }

    result
}
