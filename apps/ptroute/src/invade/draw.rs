use super::model::{AppState, HopView};

#[derive(Debug, Clone, Copy)]
pub struct UiOpts {
    pub plain: bool,
    pub ascii_only: bool,
}

pub fn render_map(state: &AppState, opts: &UiOpts, term_w: u16, _term_h: u16) -> String {
    let mut lines = Vec::new();
    let width = term_w as usize;
    let banner = "PATH TRACEROUTE INVADERS";
    lines.push(center_line(banner, width));
    lines.push(center_line(&format!("WAVE {}", state.wave), width));
    lines.push("".to_string());

    let legend = "OK=green WARN=yellow BAD=red UNKNOWN=dim";
    lines.push(center_line(legend, width));
    lines.push("".to_string());

    let max_hops = max_hops(state);
    let ttl_header = (1..=max_hops)
        .map(|n| (n % 10).to_string())
        .collect::<Vec<_>>()
        .join(" ");
    lines.push(format!("TTL: {ttl_header}"));

    let ship = if opts.plain || opts.ascii_only {
        "<^>"
    } else {
        "<^>"
    };
    let inv = if opts.plain || opts.ascii_only {
        "W"
    } else {
        "W"
    };

    for target in &state.targets {
        let row = render_row(inv, &target.hops, max_hops);
        lines.push(format!("{ship} {row}  {}", target.name));
    }

    lines.push("".to_string());
    if let Some(detail) = &state.last_detail {
        lines.push(format!("{detail}"));
    } else {
        lines.push("Last hop: (none)".to_string());
    }

    lines.join("\n")
}

fn max_hops(state: &AppState) -> u32 {
    state
        .targets
        .iter()
        .map(|t| t.hops.len() as u32)
        .max()
        .unwrap_or(0)
        .max(1)
}

fn render_row(inv: &str, hops: &[HopView], max_hops: u32) -> String {
    let mut cells = Vec::new();
    for idx in 0..max_hops {
        if let Some(_) = hops.get(idx as usize) {
            cells.push(inv.to_string());
        } else {
            cells.push(".".to_string());
        }
    }
    cells.join("-")
}

fn center_line(text: &str, width: usize) -> String {
    if text.len() >= width {
        return text.to_string();
    }
    let pad = (width - text.len()) / 2;
    format!("{}{}", " ".repeat(pad), text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invade::model::{AppState, HopView, TargetView};

    #[test]
    fn render_contains_banner_and_rows() {
        let state = AppState {
            wave: 1,
            targets: vec![TargetView {
                name: "1.1.1.1".to_string(),
                hops: vec![HopView {
                    ttl: 1,
                    ip: Some("1.1.1.1".to_string()),
                    loss: 0.0,
                    median_rtt: Some(10.0),
                }],
            }],
            last_detail: Some("Last hop demo".to_string()),
        };
        let opts = UiOpts {
            plain: true,
            ascii_only: true,
        };
        let output = render_map(&state, &opts, 80, 24);
        assert!(output.contains("PATH TRACEROUTE INVADERS"));
        assert!(output.contains("1.1.1.1"));
        assert!(output.contains("TTL:"));
    }

    #[test]
    fn plain_mode_has_no_ansi() {
        let state = AppState {
            wave: 1,
            targets: vec![],
            last_detail: None,
        };
        let opts = UiOpts {
            plain: true,
            ascii_only: true,
        };
        let output = render_map(&state, &opts, 60, 20);
        assert!(!output.contains("\x1b"));
    }
}
