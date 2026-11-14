use serde::Deserialize;
use serde_json::Value;

/// The validator's judgement of one proposed tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub safe: bool,
    pub findings: Vec<String>,
}

#[derive(Deserialize)]
struct RawVerdict {
    safe: bool,
    #[serde(default)]
    findings: Vec<Value>,
}

/// Reads a verdict out of the validator's reply. Chatter or code fences around
/// the JSON object are tolerated; a missing `safe` flag is an error.
pub fn parse_verdict(text: &str) -> Result<Verdict, String> {
    let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) else {
        return Err("the validator's reply had no JSON object".to_string());
    };
    if start > end {
        return Err("the validator's reply had no JSON object".to_string());
    }
    let raw: RawVerdict = serde_json::from_str(&text[start..=end])
        .map_err(|e| format!("could not read the validator's verdict: {e}"))?;
    Ok(Verdict {
        safe: raw.safe,
        findings: raw.findings.iter().map(finding_text).collect(),
    })
}

fn finding_text(finding: &Value) -> String {
    finding
        .as_str()
        .map_or_else(|| finding.to_string(), str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_json_parses() {
        let verdict = parse_verdict(r#"{"safe": true, "findings": []}"#).unwrap();
        assert_eq!(
            verdict,
            Verdict {
                safe: true,
                findings: vec![]
            }
        );
    }

    #[test]
    fn fences_and_chatter_are_ignored() {
        let text = "Sure! Here you go:\n```json\n{\"safe\": false, \"findings\": [\"rm -rf /\"]}\n```\nHope that helps.";
        let verdict = parse_verdict(text).unwrap();
        assert!(!verdict.safe);
        assert_eq!(verdict.findings, ["rm -rf /"]);
    }

    #[test]
    fn findings_may_be_objects_or_missing() {
        let verdict = parse_verdict(r#"{"safe": false, "findings": [{"risk": "high"}]}"#).unwrap();
        assert_eq!(verdict.findings, [r#"{"risk":"high"}"#]);
        assert!(
            parse_verdict(r#"{"safe": true}"#)
                .unwrap()
                .findings
                .is_empty()
        );
    }

    #[test]
    fn a_missing_safe_flag_is_an_error() {
        assert!(parse_verdict(r#"{"findings": []}"#).is_err());
    }

    #[test]
    fn replies_without_json_are_errors() {
        assert!(parse_verdict("looks fine to me").is_err());
        assert!(parse_verdict("} then {").is_err());
        assert!(parse_verdict("").is_err());
    }
}
