use crate::endpoints::values::Diff::UnableToCompare;
use serde_json::Value;
use std::collections::HashMap;

/// Diff represents the difference between two endpoints.
#[derive(Debug)]
pub enum Diff {
    Result(String),
    UnableToCompare,
}

pub struct ValueComparison {
    from: Vec<Value>,
    target: Vec<Value>,
    from_reconcile_node: Option<Vec<Value>>,
    target_reconcile_node: Option<Vec<Value>>,
}

impl ValueComparison {
    pub fn new(
        from: Vec<Value>,
        target: Vec<Value>,
        from_reconcile_node: Option<Vec<Value>>,
        target_reconcile_node: Option<Vec<Value>>,
    ) -> Self {
        Self {
            from,
            target,
            from_reconcile_node,
            target_reconcile_node,
        }
    }

    pub fn compare_values(&self) -> Option<Diff> {
        // Single value comparison (unary)
        if self.from.len() == 1 && self.target.len() == 1 {
            return match (&self.from[0], &self.target[0]) {
                (Value::String(f), Value::String(t)) => {
                    if f != t {
                        return Some(Diff::Result(format!("Diff from {} vs {}", f, t)));
                    }

                    None
                }
                (Value::Number(f), Value::Number(t)) => {
                    let diff = t.as_f64().unwrap_or_default() - f.as_f64().unwrap_or_default();
                    if diff != 0.0 {
                        return Some(Diff::Result(format!("Diff from {} vs {}", f, t)));
                    }

                    None
                }
                _ => None,
            };
        }

        // Otherwise get the keys from the from & target nodes which we'll use to compare the data
        let from_reconcile_keys = match &self.from_reconcile_node {
            Some(k) => get_stringify_keys_from_values(&k),
            None => return Some(UnableToCompare),
        };

        let target_from_reconcile_keys = match &self.target_reconcile_node {
            Some(k) => get_stringify_keys_from_values(&k),
            None => return Some(UnableToCompare),
        };

        let from_map = match_keys_with_nodes(&self.from, &from_reconcile_keys);
        let target_map = match_keys_with_nodes(&self.target, &target_from_reconcile_keys);

        for (k, v) in &from_map {
            if let Some(target_v) = target_map.get(k) {
                match (v, target_v) {
                    (Value::String(s1), Value::String(s2)) => {
                        if s1 != s2 {
                            return Some(Diff::Result(format!(
                                "Diff on key: {}, origin value: {} vs target value: {}",
                                k, s1, s2
                            )));
                        }
                    }
                    (Value::Number(f), Value::Number(t)) => {
                        let diff = t.as_f64().unwrap_or_default() - f.as_f64().unwrap_or_default();
                        if diff != 0.0 {
                            return Some(Diff::Result(format!(
                                "Diff on key: {}, origin value: {} vs target value: {}",
                                k, t, f
                            )));
                        };
                    }
                    _ => {}
                }
            }
        }

        None
    }
}

fn get_stringify_keys_from_values(values: &[Value]) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|v| {
            let Value::String(s) = v else {
                return None;
            };

            Some(s.clone())
        })
        .collect::<Vec<_>>()
}

fn match_keys_with_nodes(nodes: &[Value], keys: &[String]) -> HashMap<String, Value> {
    keys.into_iter()
        .enumerate()
        .filter_map(|(k, v)| {
            if let Some(node) = nodes.get(k) {
                return Some((v.clone(), node.clone()));
            }

            None
        })
        .collect::<HashMap<_, _>>()
}
