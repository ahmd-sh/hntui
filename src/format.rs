use regex::Regex;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn relative_time(unix_sec: Option<i64>) -> String {
    relative_time_at(unix_sec, now_secs())
}

fn relative_time_at(unix_sec: Option<i64>, now: i64) -> String {
    let Some(t) = unix_sec.filter(|&t| t != 0) else { return String::new() };
    let diff = now - t;
    match diff {
        d if d < 60 => format!("{d}s ago"),
        d if d < 3600 => format!("{}m ago", d / 60),
        d if d < 86400 => format!("{}h ago", d / 3600),
        d if d < 86400 * 30 => format!("{}d ago", d / 86400),
        d if d < 86400 * 365 => format!("{}mo ago", d / 2_592_000),
        d => format!("{}y ago", d / 31_536_000),
    }
}

pub fn hostname(url: Option<&str>) -> String {
    let Some(u) = url else { return String::new() };
    match url::Url::parse(u) {
        Ok(p) => {
            let host = p.host_str().unwrap_or("");
            host.strip_prefix("www.").unwrap_or(host).to_string()
        }
        Err(_) => String::new(),
    }
}

static NAMED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&(amp|lt|gt|quot|nbsp|#x27|#39|#x2F|#47);").unwrap());
static DEC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&#(\d+);").unwrap());
static HEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)&#x([0-9a-f]+);").unwrap());

fn from_code(n: Option<u32>) -> String {
    // JS String.fromCharCode truncates to 16 bits; match it
    n.and_then(|n| char::from_u32(n & 0xffff)).map(String::from).unwrap_or_default()
}

/// Old HN posts carry raw Windows-1252 bytes (`don\x92t`), which arrive as
/// C1 control characters. Written to a terminal those are control codes,
/// not text, and can garble the display. Map them to what they meant, and
/// drop any other control characters (newlines and tabs stay).
pub fn clean_text(s: &str) -> String {
    const CP1252: [char; 32] = [
        '€', '\0', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\0', 'Ž', '\0', //
        '\0', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\0', 'ž', 'Ÿ',
    ];
    s.chars()
        .filter_map(|c| match c {
            '\u{80}'..='\u{9f}' => Some(CP1252[c as usize - 0x80]).filter(|&m| m != '\0'),
            '\n' | '\t' => Some(c),
            c if c.is_control() => None,
            c => Some(c),
        })
        .collect()
}

pub fn decode_entities(s: &str) -> String {
    let s = NAMED.replace_all(s, |c: &regex::Captures| {
        match &c[0] {
            "&amp;" => "&",
            "&lt;" => "<",
            "&gt;" => ">",
            "&quot;" => "\"",
            "&#x27;" | "&#39;" => "'",
            "&#x2F;" | "&#47;" => "/",
            "&nbsp;" => " ",
            _ => unreachable!(),
        }
        .to_string()
    });
    let s = DEC.replace_all(&s, |c: &regex::Captures| from_code(c[1].parse().ok()));
    HEX.replace_all(&s, |c: &regex::Captures| from_code(u32::from_str_radix(&c[1], 16).ok())).into_owned()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub text: String,
    pub url: String,
}

static A_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?is)<a\s+href="([^"]+)"[^>]*>(.*?)</a>"#).unwrap());
static ANY_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

pub fn extract_links(html: Option<&str>) -> Vec<Link> {
    let Some(html) = html else { return vec![] };
    let mut out: Vec<Link> = Vec::new();
    for m in A_TAG.captures_iter(html) {
        let url = decode_entities(&m[1]);
        if out.iter().any(|l| l.url == url) {
            continue;
        }
        let text = decode_entities(&ANY_TAG.replace_all(&m[2], "")).trim().to_string();
        out.push(Link { text: if text.is_empty() { url.clone() } else { text }, url });
    }
    out
}

/// A link into HN itself: /item?id=POSTID or /item?id=POSTID#COMMENTID.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HnItemRef {
    pub id: u64,
    pub anchor_id: Option<u64>,
}

pub fn parse_hn_item_link(url: &str) -> Option<HnItemRef> {
    let u = url::Url::parse(url).ok()?;
    let host = u.host_str()?;
    if host.strip_prefix("www.").unwrap_or(host) != "news.ycombinator.com" || u.path() != "/item" {
        return None;
    }
    let id: u64 = u.query_pairs().find(|(k, _)| k == "id")?.1.parse().ok().filter(|&n| n > 0)?;
    let anchor_id = u.fragment().and_then(|f| f.parse().ok()).filter(|&n: &u64| n > 0);
    Some(HnItemRef { id, anchor_id })
}

static P_OPEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<p>").unwrap());
static P_CLOSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</p>").unwrap());
static BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").unwrap());
static ITAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<i>(.*?)</i>").unwrap());
static A_INLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)<a\s+href="([^"]+)"[^>]*>(.*?)</a>"#).unwrap());
static PRE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<pre><code>(.*?)</code></pre>").unwrap());
static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<code>(.*?)</code>").unwrap());

pub fn html_to_text(html: Option<&str>) -> String {
    let Some(html) = html.filter(|h| !h.is_empty()) else { return String::new() };
    let s = P_OPEN.replace_all(html, "\n\n");
    let s = P_CLOSE.replace_all(&s, "");
    let s = BR.replace_all(&s, "\n");
    let s = ITAL.replace_all(&s, "$1");
    let s = A_INLINE.replace_all(&s, "$2 ($1)");
    let s = PRE.replace_all(&s, "\n$1\n");
    let s = CODE.replace_all(&s, "`$1`");
    let s = ANY_TAG.replace_all(&s, "");
    decode_entities(&s).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_cp1252_and_controls() {
        assert_eq!(clean_text("don\u{92}t \u{93}hi\u{94}\u{85}"), "don’t “hi”…");
        assert_eq!(clean_text("a\u{1b}[31mb\u{8d}c\nd\te"), "a[31mbc\nd\te");
    }

    #[test]
    fn relative() {
        let now = 1_000_000_000;
        assert_eq!(relative_time_at(None, now), "");
        assert_eq!(relative_time_at(Some(now - 5), now), "5s ago");
        assert_eq!(relative_time_at(Some(now - 120), now), "2m ago");
        assert_eq!(relative_time_at(Some(now - 7200), now), "2h ago");
        assert_eq!(relative_time_at(Some(now - 86400 * 3), now), "3d ago");
        assert_eq!(relative_time_at(Some(now - 86400 * 60), now), "2mo ago");
        assert_eq!(relative_time_at(Some(now - 86400 * 800), now), "2y ago");
    }

    #[test]
    fn host() {
        assert_eq!(hostname(Some("https://www.example.com/a")), "example.com");
        assert_eq!(hostname(Some("not a url")), "");
        assert_eq!(hostname(None), "");
    }

    #[test]
    fn html() {
        let t = html_to_text(Some(
            "Hi <i>there</i><p>See <a href=\"https://x.com/&#x2F;y\" rel=\"nofollow\">x</a> &amp; <code>c</code>",
        ));
        assert_eq!(t, "Hi there\n\nSee x (https://x.com//y) & `c`");
    }

    #[test]
    fn links() {
        let l = extract_links(Some(
            "<a href=\"https://a.com\">A</a> <a href=\"https://a.com\">dupe</a> <a href=\"https://b.com\"></a>",
        ));
        assert_eq!(
            l,
            vec![
                Link { text: "A".into(), url: "https://a.com".into() },
                Link { text: "https://b.com".into(), url: "https://b.com".into() },
            ]
        );
    }

    #[test]
    fn hn_links() {
        assert_eq!(parse_hn_item_link("https://news.ycombinator.com/item?id=15"), Some(HnItemRef { id: 15, anchor_id: None }));
        assert_eq!(
            parse_hn_item_link("https://news.ycombinator.com/item?id=1#7"),
            Some(HnItemRef { id: 1, anchor_id: Some(7) })
        );
        assert_eq!(parse_hn_item_link("https://news.ycombinator.com/user?id=pg"), None);
        assert_eq!(parse_hn_item_link("https://example.com/item?id=1"), None);
        assert_eq!(parse_hn_item_link("https://news.ycombinator.com/item?id=abc"), None);
    }
}
