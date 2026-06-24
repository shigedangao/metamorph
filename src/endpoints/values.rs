use serde_json::Value;
use std::collections::HashMap;

/// Diff represents the difference between two endpoints.
#[derive(Debug)]
pub enum Diff {
    Result(String),
    UnableToCompare,
}

/// ValueComparison compares two sets of values from two endpoints.
pub struct ValueComparison<'a> {
    from: &'a [Value],
    target: &'a [Value],
    from_reconcile_node: Option<Vec<Value>>,
    target_reconcile_node: Option<Vec<Value>>,
}

impl<'a> ValueComparison<'a> {
    /// Creates a new [`ValueComparison`] instance.
    ///
    /// # Arguments
    ///
    /// * `from` - A slice of [`Value`]s representing the values from the source endpoint.
    /// * `target` - A slice of [`Value`]s representing the values from the target endpoint.
    /// * `from_reconcile_node` - An optional [`Vec`] of [`Value`]s representing the reconcile node from the source endpoint.
    /// * `target_reconcile_node` - An optional [`Vec`] of [`Value`]s representing the reconcile node from the target endpoint.
    pub fn new(
        from: &'a [Value],
        target: &'a [Value],
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

    /// Compares the values from the source and target endpoints.
    ///
    /// # Returns
    ///
    /// * `Some(Vec<Diff>)` - A vector of [`Diff`]s representing the differences between the source and target endpoints.
    /// * `None` - If the values could not be compared.
    pub fn compare_values(&self) -> Option<Vec<Diff>> {
        // Single value comparison (unary)
        if self.from.len() == 1 && self.target.len() == 1 {
            return match (&self.from[0], &self.target[0]) {
                (Value::String(f), Value::String(t)) => {
                    if f != t {
                        return Some(vec![Diff::Result(format!("Diff from {f} vs {t}"))]);
                    }

                    None
                }
                (Value::Number(f), Value::Number(t)) => {
                    let diff = t.as_f64().unwrap_or_default() - f.as_f64().unwrap_or_default();
                    if diff != 0.0 {
                        return Some(vec![Diff::Result(format!("Diff from {f} vs {t}"))]);
                    }

                    None
                }
                _ => None,
            };
        }

        // Otherwise get the keys from the from & target nodes which we'll use to compare the data
        let from_reconcile_keys = match &self.from_reconcile_node {
            Some(k) => get_stringify_keys_from_values(k),
            None => return Some(vec![Diff::UnableToCompare]),
        };

        let target_from_reconcile_keys = match &self.target_reconcile_node {
            Some(k) => get_stringify_keys_from_values(k),
            None => return Some(vec![Diff::UnableToCompare]),
        };

        let from_map = match_keys_with_nodes(self.from, &from_reconcile_keys);
        let target_map = match_keys_with_nodes(self.target, &target_from_reconcile_keys);
        let mut diffs = Vec::new();

        for (k, v) in &from_map {
            if let Some(target_v) = target_map.get(k) {
                match (v, target_v) {
                    (Value::String(s1), Value::String(s2)) => {
                        if s1 != s2 {
                            diffs.push(Diff::Result(format!(
                                "Diff on key: {k}, origin: {s1} vs target: {s2}"
                            )));
                        }
                    }
                    (Value::Number(f), Value::Number(t)) => {
                        let diff = t.as_f64().unwrap_or_default() - f.as_f64().unwrap_or_default();
                        if diff != 0.0 {
                            diffs.push(Diff::Result(format!(
                                "Diff on key: {k}, origin: {f} vs target: {t}"
                            )));
                        };
                    }
                    _ => {}
                }
            }
        }

        Some(diffs)
    }
}

/// Returns a list of stringify keys from a list of values.
///
/// # Arguments
///
/// * `values` - A slice of [`Value`]s to extract stringify keys from.
fn get_stringify_keys_from_values(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(|v| {
            let Value::String(s) = v else {
                return None;
            };

            Some(s.clone())
        })
        .collect::<Vec<_>>()
}

/// Matches keys with nodes in a one-to-one fashion.
///
/// # Arguments
///
/// * `nodes` - A slice of [`Value`]s representing the nodes.
/// * `keys` - A slice of [`String`]s representing the keys.
fn match_keys_with_nodes(nodes: &[Value], keys: &[String]) -> HashMap<String, Value> {
    keys.iter()
        .enumerate()
        .filter_map(|(k, v)| {
            if let Some(node) = nodes.get(k) {
                return Some((v.clone(), node.clone()));
            }

            None
        })
        .collect::<HashMap<_, _>>()
}
