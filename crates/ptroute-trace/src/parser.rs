use anyhow::{anyhow, Result};
use ptroute_model::Hop;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTraceRun {
    pub target: String,
    pub hops: Vec<Hop>,
}

pub fn parse_traceroute_n(text: &str) -> Result<ParsedTraceRun> {
    parse_traceroute_n_inner(text, None)
}

pub fn parse_traceroute_n_with_target(text: &str, fallback_target: &str) -> Result<ParsedTraceRun> {
    parse_traceroute_n_inner(text, Some(fallback_target))
}

fn parse_traceroute_n_inner(text: &str, fallback_target: Option<&str>) -> Result<ParsedTraceRun> {
    let mut target: Option<String> = None;
    let mut hops = Vec::new();
    let mut current_hop: Option<usize> = None;

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

        let mut tokens = line.split_whitespace();
        let first_token = match tokens.next() {
            Some(token) => token,
            None => continue,
        };

        if first_token.chars().all(|c| c.is_ascii_digit()) {
            let hop = parse_hop_line(line)?;
            hops.push(hop);
            current_hop = Some(hops.len() - 1);
            continue;
        }

        if let Some(index) = current_hop {
            if is_probe_start(first_token) {
                let rest: Vec<&str> = std::iter::once(first_token)
                    .chain(tokens)
                    .collect();
                let hop = &mut hops[index];
                append_probe_tokens(&rest, &mut hop.ip, &mut hop.rtt_ms);
            }
        }
    }

    let target = match target {
        Some(value) => value,
        None => fallback_target
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.to_string())
            .ok_or_else(|| anyhow!("missing target in traceroute output"))?,
    };

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

    append_probe_tokens(&tokens[1..], &mut ip, &mut rtt_ms);

    Ok(Hop { ttl, ip, rtt_ms })
}

fn append_probe_tokens(tokens: &[&str], ip: &mut Option<String>, rtt_ms: &mut Vec<Option<f64>>) {
    let mut i = 0;
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
                *ip = Some(tok.to_string());
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
}

fn is_probe_start(token: &str) -> bool {
    token == "*" || is_ip_token(token)
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

    #[test]
    fn parse_uses_fallback_target() {
        let text = "1  192.168.1.1  1.0 ms  1.1 ms  1.2 ms";
        let run = parse_traceroute_n_with_target(text, "9.9.9.9").unwrap();
        assert_eq!(run.target, "9.9.9.9");
        assert_eq!(run.hops.len(), 1);
    }
}
