// Word wrap that reproduces OpenTUI's `wrapMode: "word"` line breaks, probed
// against its test renderer: fill greedily to the width, then break *after*
// the last boundary character inside the line (whitespace and a set of
// punctuation, so URLs break after "/"). No boundary → hard break. Spaces
// are never trimmed, so a continuation line can start with one.

use crate::canvas::G;

fn is_boundary(s: &str) -> bool {
    matches!(
        s,
        " " | "\t" | "/" | "-" | "." | "," | ":" | ";" | "(" | ")" | "[" | "]" | "{" | "}" | "\\" | "?" | "!"
    )
}

pub fn wrap(gs: &[G], width: usize) -> Vec<Vec<G>> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for para in gs.split(|g| g.s == "\n" || g.s == "\r\n") {
        let mut rest = para;
        loop {
            let total: usize = rest.iter().map(|g| g.w).sum();
            if total <= width {
                lines.push(rest.to_vec());
                break;
            }
            // longest prefix that fits
            let mut n = 0;
            let mut used = 0;
            while n < rest.len() && used + rest[n].w <= width {
                used += rest[n].w;
                n += 1;
            }
            let n = n.max(1);
            let cut = match rest[..n].iter().rposition(|g| is_boundary(&g.s)) {
                Some(i) => i + 1,
                None => n,
            };
            lines.push(rest[..cut].to_vec());
            rest = &rest[cut..];
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{styled, Style};
    use crate::color::Rgba;

    fn w(s: &str) -> Vec<String> {
        wrap(&styled(s, Style::fg(Rgba::WHITE)), 12)
            .into_iter()
            .map(|l| l.into_iter().map(|g| g.s).collect())
            .collect()
    }

    // Every expectation below is OpenTUI's actual output at width 12.
    #[test]
    fn matches_opentui() {
        assert_eq!(w("aaaa bbbb cccc dddd"), ["aaaa bbbb ", "cccc dddd"]);
        assert_eq!(w("aaaaa/bbbbbbbbb"), ["aaaaa/", "bbbbbbbbb"]);
        assert_eq!(w("aaaaa_bbbbbbbbb"), ["aaaaa_bbbbbb", "bbb"]);
        assert_eq!(w("aaaa  bbbb    cccc"), ["aaaa  bbbb  ", "  cccc"]);
        assert_eq!(w("aaaa bbbb cc dd"), ["aaaa bbbb ", "cc dd"]);
        assert_eq!(w("aaaaaaaaaaaa bb"), ["aaaaaaaaaaaa", " bb"]);
        assert_eq!(w("aaaaaaaaaaa bb"), ["aaaaaaaaaaa ", "bb"]);
        assert_eq!(w("aaaaaaaaaaaab bb"), ["aaaaaaaaaaaa", "b bb"]);
        assert_eq!(w("aaaa bbbbbbb  cc"), ["aaaa ", "bbbbbbb  cc"]);
        assert_eq!(w("aa,bbbbbbbbbbbbbbbbbbbbbb"), ["aa,", "bbbbbbbbbbbb", "bbbbbbbbbb"]);
        assert_eq!(w("ab\n\ncd efgh ijklmnop"), ["ab", "", "cd efgh ", "ijklmnop"]);
    }
}
