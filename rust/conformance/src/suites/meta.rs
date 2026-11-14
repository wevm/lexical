use crate::case::{Case, Outcome, same};
use lexical::json::Value;

/// `meta.json` names the upstream release the vectors come from, which must
/// be the one the port declares.
pub fn check(case: &Value, _file: &Value) -> Result<Outcome, String> {
    same(
        "lexicalVersion",
        lexical::UPSTREAM_VERSION,
        case.str("lexicalVersion")?,
    )?;
    Ok(Outcome::Pass)
}
