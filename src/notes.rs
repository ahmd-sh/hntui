// Release notes: CHANGELOG.md, built into the binary. Shown once in the app
// after an update (see main.rs), and any time with `hntui changelog`.

use crate::api::compare_versions;
use crate::canvas::{styled, width_of, Style, G};
use crate::theme::Theme;
use crate::wrap::wrap;

pub const CHANGELOG: &str = include_str!("../CHANGELOG.md");

// "## 0.6.0" sections, newest first, as (version, body)
fn sections() -> Vec<(&'static str, &'static str)> {
    let mut out = vec![];
    let mut rest = CHANGELOG;
    while let Some(i) = rest.find("\n## ") {
        let start = i + 4;
        let line_end = rest[start..].find('\n').map_or(rest.len(), |n| start + n);
        let version = rest[start..line_end].split_whitespace().next().unwrap_or("");
        let body_end = rest[line_end..].find("\n## ").map_or(rest.len(), |n| line_end + n);
        out.push((version, rest[line_end..body_end].trim_matches('\n')));
        rest = &rest[body_end..];
    }
    out
}

/// What to show someone who last saw `seen`'s notes and now runs `current`:
/// every section in between, or just `current`'s when there's no record.
pub fn since(seen: Option<&str>, current: &str) -> Vec<(&'static str, &'static str)> {
    sections()
        .into_iter()
        .filter(|(v, _)| {
            compare_versions(v, current).is_le() && seen.map_or(*v == current, |s| compare_versions(v, s).is_gt())
        })
        .collect()
}

/// The notes as markdown, for `hntui changelog`.
pub fn print(current: &str) {
    match since(None, current).first() {
        Some((v, body)) => println!("hntui {v}\n\n{body}"),
        None => println!("{CHANGELOG}"),
    }
}

/// Lays out the markdown this file uses (headings, paragraphs, bullets,
/// tables, `code`) as styled lines `width` cells wide.
pub fn render(sections: &[(&str, &str)], t: &Theme, width: usize) -> Vec<Vec<G>> {
    let mut out: Vec<Vec<G>> = vec![];
    let blank = |out: &mut Vec<Vec<G>>| {
        if out.last().is_some_and(|l| !l.is_empty()) {
            out.push(vec![]);
        }
    };
    for (si, (version, body)) in sections.iter().enumerate() {
        // the frame title names the version when there's only one
        if sections.len() > 1 {
            if si > 0 {
                blank(&mut out);
            }
            out.push(styled(version, Style::fg(t.accent).bold()));
            blank(&mut out);
        }
        let lines: Vec<&str> = body.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim_end();
            if line.is_empty() {
                blank(&mut out);
            } else if let Some(h) = line.strip_prefix("### ") {
                blank(&mut out);
                out.push(styled(h, Style::fg(t.accent).bold()));
            } else if line.starts_with('|') {
                let start = i;
                while i + 1 < lines.len() && lines[i + 1].starts_with('|') {
                    i += 1;
                }
                out.extend(table(&lines[start..=i], t));
            } else if let Some(item) = line.strip_prefix("- ") {
                // hanging indent under the bullet
                let wrapped = prose(&inline(item, t), width.saturating_sub(2).max(1));
                for (k, l) in wrapped.into_iter().enumerate() {
                    let mut row = styled(if k == 0 { "• " } else { "  " }, Style::fg(t.accent));
                    row.extend(l);
                    out.push(row);
                }
            } else {
                out.extend(prose(&inline(line, t), width.max(1)));
            }
            i += 1;
        }
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out
}

// The app's wrap keeps OpenTUI's habit of breaking after punctuation, which
// can start a line with the space that followed it; fine for comments
// (it's what the original did), untidy in prose.
fn prose(gs: &[G], width: usize) -> Vec<Vec<G>> {
    let mut lines = wrap(gs, width);
    for l in lines.iter_mut().skip(1) {
        let lead = l.iter().take_while(|g| g.s == " ").count();
        l.drain(..lead);
    }
    lines
}

// `code` spans in the accent colour, everything else as body text
fn inline(text: &str, t: &Theme) -> Vec<G> {
    text.replace("**", "")
        .split('`')
        .enumerate()
        .flat_map(|(i, part)| styled(part, Style::fg(if i % 2 == 1 { t.accent } else { t.text_body })))
        .collect()
}

// columns padded to line up; the last column (this release) in the accent
fn table(rows: &[&str], t: &Theme) -> Vec<Vec<G>> {
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|r| r.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect::<Vec<_>>())
        .filter(|r| !r.iter().all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':')))
        .collect();
    let ncol = cells.iter().map(Vec::len).max().unwrap_or(0);
    let colw: Vec<usize> = (0..ncol)
        .map(|c| cells.iter().map(|r| r.get(c).map_or(0, |s| width_of(&styled(s, Style::fg(t.text))))).max().unwrap_or(0))
        .collect();
    cells
        .iter()
        .enumerate()
        .map(|(ri, r)| {
            let mut line = vec![];
            for (c, w) in colw.iter().enumerate() {
                let s = r.get(c).map_or("", String::as_str);
                let fg = if ri == 0 {
                    t.text_muted
                } else if c + 1 == ncol {
                    t.accent
                } else if c == 0 {
                    t.text_body
                } else {
                    t.text_muted
                };
                let mut cell = styled(s, Style::fg(fg).bold_if(ri == 0 && c > 0));
                let pad = w - width_of(&cell) + if c + 1 < ncol { 3 } else { 0 };
                cell.extend(styled(&" ".repeat(pad), Style::fg(fg)));
                line.extend(cell);
            }
            while line.last().is_some_and(|g| g.s == " ") {
                line.pop();
            }
            line
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_sections() {
        let cur = env!("CARGO_PKG_VERSION");
        assert_eq!(since(None, cur).len(), 1, "a changelog section for {cur}");
        assert_eq!(since(None, cur)[0].0, cur);
        assert_eq!(since(Some(cur), cur).len(), 0);
        assert_eq!(since(Some("0.0.1"), cur).first().map(|s| s.0), Some(cur));
    }
}
