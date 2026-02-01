use anyhow::{anyhow, Result};
use ptroute_model::Hop;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTraceRun {
    pub target: String,
    pub hops: Vec<Hop>,
}

pub fn parse_traceroute_n(text: &str) -> Result<ParsedTraceRun> {
    let mut target: Option<String> = None;
    let mut hops = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.to_ascii_lowercase().starts_with("traceroute") {
            if target.is_none() {
                target = parse_target(line);
            }
            continue;
        }

        let first = line.chars().next().unwrap_or(' ');
        if !first.is_ascii_digit() {
            continue;
        }

        let hop = parse_hop_line(line)?;
        hops.push(hop);
    }

    let target = target.ok_or_else(|| anyhow!("missing target in traceroute output"))?;

    Ok(ParsedTraceRun { target, hops })
}

fn parse_target(line: &str) -> Option<String> {
    if let Some(start) = line.find('(') {
        if let Some(end) = line[start + 1..].find(')') {
            let inside = &line[start + 1..start + 1 + end];
            let trimmed = inside.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    let lower = line.to_ascii_lowercase();
    if let Some(idx) = lower.find("traceroute to ") {
        let rest = &line[idx + "traceroute to ".len()..];
        let token = rest
            .split(|c: char| c.is_whitespace() || c == ',')
            .next()?
            .trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }

    None
}

fn parse_hop_line(line: &str) -> Result<Hop> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(anyhow!("empty hop line"));
    }

    let ttl: u32 = tokens[0]
        .parse()
        .map_err(|_| anyhow!("invalid ttl token: {}", tokens[0]))?;

    let mut ip: Option<String> = None;
    let mut rtt_ms: Vec<Option<f64>> = Vec::new();

    let mut i = 1;
    while i < tokens.len() {
        let tok = tokens[i];

        if tok == "*" {
            rtt_ms.push(None);
            i += 1;
            continue;
        }

        if tok.starts_with('!') {
            i += 1;
            continue;
        }

        if is_ip_token(tok) {
            if ip.is_none() {
                ip = Some(tok.to_string());
            }
            i += 1;
            continue;
        }

        let next = tokens.get(i + 1).copied();
        if let Some((val, consumed_next)) = parse_rtt(tok, next) {
            rtt_ms.push(Some(val));
            i += if consumed_next { 2 } else { 1 };
            continue;
        }

        i += 1;
    }

    Ok(Hop { ttl, ip, rtt_ms })
}

fn is_ip_token(token: &str) -> bool {
    if token.ends_with("ms") {
        return false;
    }

    is_ipv4(token) || is_ipv6(token)
}

fn is_ipv4(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 4 {
        return false;
    }

    for part in parts {
        if part.is_empty() || part.len() > 3 {
            return false;
        }
        if !part.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        if part.parse::<u8>().is_err() {
            return false;
        }
    }

    true
}

fn is_ipv6(token: &str) -> bool {
    if !token.contains(':') {
        return false;
    }

    token
        .chars()
        .all(|c| c.is_ascii_hexdigit() || c == ':')
}

fn parse_rtt(token: &str, next: Option<&str>) -> Option<(f64, bool)> {
    if let Some(num) = token.strip_suffix("ms") {
        if let Ok(val) = num.parse::<f64>() {
            return Some((val, false));
        }
    }

    if let Ok(val) = token.parse::<f64>() {
        if matches!(next, Some("ms")) {
            return Some((val, true));
        }
        if matches!(next, Some(next_tok) if next_tok.starts_with("ms")) {
            return Some((val, true));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_target_from_header() {
        let line = "traceroute to 1.1.1.1 (1.1.1.1), 30 hops max";
        assert_eq!(parse_target(line), Some("1.1.1.1".to_string()));
    }
}
