use anyhow::{Result, bail};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};

fn var_regex() -> Regex {
    Regex::new(r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\}\}").expect("valid regex")
}

fn valid_name_regex() -> Regex {
    Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").expect("valid regex")
}

pub fn is_valid_var_name(name: &str) -> bool {
    valid_name_regex().is_match(name)
}

pub fn invalid_var_names(declared: &BTreeMap<String, String>) -> BTreeSet<String> {
    declared
        .keys()
        .filter(|name| !is_valid_var_name(name))
        .cloned()
        .collect()
}

pub fn references_in(value: &str) -> BTreeSet<String> {
    var_regex()
        .captures_iter(value)
        .map(|caps| caps[1].to_string())
        .collect()
}

pub fn missing_vars(values: &[String], declared: &BTreeMap<String, String>) -> BTreeSet<String> {
    let mut missing = BTreeSet::new();
    for value in values {
        for name in references_in(value) {
            if !declared.contains_key(&name) {
                missing.insert(name);
            }
        }
    }
    missing
}

pub fn required_empty_vars(declared: &BTreeMap<String, String>) -> BTreeSet<String> {
    declared
        .iter()
        .filter(|(_, value)| value.trim().is_empty())
        .map(|(name, _)| name.clone())
        .collect()
}

pub fn substitute(value: &str, vars: &BTreeMap<String, String>) -> Result<String> {
    let regex = var_regex();
    let mut output = String::with_capacity(value.len());
    let mut last = 0;
    for caps in regex.captures_iter(value) {
        let mat = caps.get(0).expect("match");
        let name = caps.get(1).expect("name").as_str();
        let replacement = vars
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("missing variable `{name}`"))?;
        if replacement.is_empty() {
            bail!("required variable `{name}` is empty");
        }
        output.push_str(&value[last..mat.start()]);
        output.push_str(replacement);
        last = mat.end();
    }
    output.push_str(&value[last..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{
        invalid_var_names, is_valid_var_name, missing_vars, required_empty_vars, substitute,
    };
    use std::collections::BTreeMap;

    #[test]
    fn substitutes_declared_vars() {
        let mut vars = BTreeMap::new();
        vars.insert("log".into(), "/tmp/app.log".into());
        vars.insert("pattern".into(), "ERROR".into());
        let rendered = substitute("tail {{log}} | grep {{pattern}}", &vars).unwrap();
        assert_eq!(rendered, "tail /tmp/app.log | grep ERROR");
    }

    #[test]
    fn detects_missing_vars() {
        let mut vars = BTreeMap::new();
        vars.insert("pattern".into(), "ERROR".into());
        let missing = missing_vars(&["tail {{log}} | grep {{pattern}}".into()], &vars);
        assert!(missing.contains("log"));
        assert!(!missing.contains("pattern"));
    }

    #[test]
    fn finds_required_empty_vars() {
        let mut vars = BTreeMap::new();
        vars.insert("log".into(), "".into());
        vars.insert("pattern".into(), "ERROR".into());
        let required = required_empty_vars(&vars);
        assert!(required.contains("log"));
        assert!(!required.contains("pattern"));
    }

    #[test]
    fn validates_var_names() {
        assert!(is_valid_var_name("log_file"));
        assert!(is_valid_var_name("_gpu_id"));
        assert!(!is_valid_var_name("log-file"));
        assert!(!is_valid_var_name("1run"));

        let vars = BTreeMap::from([
            ("good_name".into(), "".into()),
            ("bad-name".into(), "".into()),
        ]);
        let invalid = invalid_var_names(&vars);
        assert!(invalid.contains("bad-name"));
        assert!(!invalid.contains("good_name"));
    }
}
