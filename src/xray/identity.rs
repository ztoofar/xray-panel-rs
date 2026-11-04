// Xray keys user traffic and online state by the client's email. Writing the
// inbound id into that email is the only way to know which inbound a client is
// connected through: the online-stats API exposes only email + source IPs.
// The panel therefore meters each (inbound, client) attachment separately.
pub fn stat_email(inbound_id: i64, email: &str) -> String {
    format!("{inbound_id}:{email}")
}

#[cfg(test)]
pub fn split_stat_email(identity: &str) -> Option<(i64, &str)> {
    let (prefix, email) = identity.split_once(':')?;
    let inbound_id = prefix.parse().ok()?;
    if email.is_empty() {
        return None;
    }
    Some((inbound_id, email))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_identity_round_trips() {
        let identity = stat_email(12, "alice@example.com");
        assert_eq!(identity, "12:alice@example.com");
        assert_eq!(split_stat_email(&identity), Some((12, "alice@example.com")));
        // An email may itself contain a colon; only the first one is the id.
        assert_eq!(split_stat_email("12:team:alice"), Some((12, "team:alice")));
        assert_eq!(split_stat_email("alice@example.com"), None);
        assert_eq!(split_stat_email("x:alice"), None);
        assert_eq!(split_stat_email("12:"), None);
    }
}
