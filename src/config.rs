// User settings: ~/.config/hntui/config.toml. The app only ever reads this
// file; anything it remembers on its own (the last theme, the last version
// whose notes you saw) goes to state.json instead, so a file you edit by
// hand is never rewritten under you.

use std::path::PathBuf;

use crate::store;
use crate::theme::ThemeName;

pub const TEMPLATE: &str = r#"# hntui settings. Delete a line to go back to its default.

# Theme at startup: "last" (whatever `t` left it on), "dark" or "light".
theme = "last"

# Mouse support: clicks, the scroll wheel, the right-click menu. Set it to
# false to leave the mouse to your terminal, so click-and-drag selects text.
mouse = true
"#;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeSetting {
    Last,
    Fixed(ThemeName),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub theme: ThemeSetting,
    pub mouse: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config { theme: ThemeSetting::Last, mouse: true }
    }
}

pub fn path() -> PathBuf {
    store::config_dir().join("config.toml")
}

/// Reads config.toml. Problems never stop the app from starting: a file
/// that doesn't parse means all defaults, a bad setting means that
/// setting's default, and either way the warning goes to the status bar.
pub fn load() -> (Config, Option<String>) {
    match std::fs::read_to_string(path()) {
        Ok(text) => parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Config::default(), None),
        Err(e) => (Config::default(), Some(format!("couldn't read config.toml ({e}), using defaults"))),
    }
}

pub fn parse(text: &str) -> (Config, Option<String>) {
    let mut c = Config::default();
    let table: toml::Table = match text.parse() {
        Ok(t) => t,
        Err(e) => {
            let line = e.span().map(|s| text[..s.start.min(text.len())].matches('\n').count() + 1);
            let at = line.map(|l| format!(" line {l}")).unwrap_or_default();
            let msg = e.message().lines().next().unwrap_or("invalid TOML").trim_end_matches('.');
            return (c, Some(format!("config.toml{at} isn't valid TOML ({msg}), using defaults")));
        }
    };
    let mut problems = vec![];
    for (key, value) in &table {
        match (key.as_str(), value) {
            ("theme", toml::Value::String(s)) => match s.as_str() {
                "last" => c.theme = ThemeSetting::Last,
                "dark" => c.theme = ThemeSetting::Fixed(ThemeName::Dark),
                "light" => c.theme = ThemeSetting::Fixed(ThemeName::Light),
                _ => problems.push(r#"theme should be "last", "dark" or "light""#.to_string()),
            },
            ("theme", _) => problems.push(r#"theme should be "last", "dark" or "light""#.to_string()),
            ("mouse", toml::Value::Boolean(b)) => c.mouse = *b,
            ("mouse", _) => problems.push("mouse should be true or false".to_string()),
            (k, _) => problems.push(format!("unknown setting \"{k}\"")),
        }
    }
    let warning = (!problems.is_empty()).then(|| format!("config.toml: {}", problems.join("; ")));
    (c, warning)
}

/// `hntui config`: show where the file is and what's in effect, creating
/// the commented template on first use. `hntui config edit` opens it.
pub fn command(edit: bool) -> std::io::Result<()> {
    let p = path();
    if !p.exists() {
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&p, TEMPLATE)?;
        println!("created {}", p.display());
    }
    if edit {
        let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| "vi".into());
        // $EDITOR may carry flags ("code --wait")
        let mut parts = editor.split_whitespace();
        let status = std::process::Command::new(parts.next().unwrap_or("vi")).args(parts).arg(&p).status()?;
        if !status.success() {
            eprintln!("hntui: {editor} exited with {status}");
        }
        return Ok(());
    }
    let (c, warning) = load();
    println!("{}", p.display());
    let theme = match c.theme {
        ThemeSetting::Last => "last",
        ThemeSetting::Fixed(ThemeName::Dark) => "dark",
        ThemeSetting::Fixed(ThemeName::Light) => "light",
    };
    println!("  theme = \"{theme}\"\n  mouse = {}", c.mouse);
    if let Some(w) = warning {
        println!("warning: {w}");
    }
    println!("edit it with: hntui config edit");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_is_the_defaults() {
        assert_eq!(parse(TEMPLATE), (Config::default(), None));
    }

    #[test]
    fn settings_and_problems() {
        let (c, w) = parse("theme = \"light\"\nmouse = false\n");
        assert_eq!(c, Config { theme: ThemeSetting::Fixed(ThemeName::Light), mouse: false });
        assert_eq!(w, None);

        // one bad value keeps the rest
        let (c, w) = parse("theme = \"blue\"\nmouse = false\nmous = true\n");
        assert_eq!(c, Config { theme: ThemeSetting::Last, mouse: false });
        assert_eq!(w.unwrap(), r#"config.toml: unknown setting "mous"; theme should be "last", "dark" or "light""#);

        // unparseable: all defaults, with the line
        let (c, w) = parse("theme = \"dark\"\nmouse = \n");
        assert_eq!(c, Config::default());
        assert!(w.unwrap().starts_with("config.toml line 2 isn't valid TOML"), "got a line number");
    }
}
