use ratatui::style::Color;

pub const BASE: Color = Color::Rgb(0x1e, 0x1e, 0x2e);
pub const TEXT: Color = Color::Rgb(0xcd, 0xd6, 0xf4);
pub const OVERLAY0: Color = Color::Rgb(0x6c, 0x70, 0x86);
pub const SURFACE1: Color = Color::Rgb(0x45, 0x47, 0x5a);
pub const ACCENT: Color = Color::Rgb(0xfa, 0xb3, 0x87); // peach
pub const RED: Color = Color::Rgb(0xf3, 0x8b, 0xa8);
pub const YELLOW: Color = Color::Rgb(0xf9, 0xe2, 0xaf);
pub const TEAL: Color = Color::Rgb(0x94, 0xe2, 0xd5);
pub const GREEN: Color = Color::Rgb(0xa6, 0xe3, 0xa1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Blocked,
    Working,
    Done,
    Idle,
    Unknown,
}

impl Status {
    pub fn parse(s: &str) -> Status {
        match s {
            "blocked" => Status::Blocked,
            "working" => Status::Working,
            "done" => Status::Done,
            "idle" => Status::Idle,
            _ => Status::Unknown,
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Status::Blocked => 0,
            Status::Working => 1,
            Status::Done => 2,
            Status::Idle => 3,
            Status::Unknown => 4,
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Status::Blocked => "◉",
            Status::Working => "◍",
            Status::Done => "●",
            Status::Idle => "✓",
            Status::Unknown => "○",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Status::Blocked => RED,
            Status::Working => YELLOW,
            Status::Done => TEAL,
            Status::Idle => GREEN,
            Status::Unknown => OVERLAY0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_and_unknown() {
        assert_eq!(Status::parse("blocked"), Status::Blocked);
        assert_eq!(Status::parse("working"), Status::Working);
        assert_eq!(Status::parse("done"), Status::Done);
        assert_eq!(Status::parse("idle"), Status::Idle);
        assert_eq!(Status::parse("unknown"), Status::Unknown);
        assert_eq!(Status::parse("garbage"), Status::Unknown);
    }

    #[test]
    fn worst_status_ranks_blocked_first() {
        let mut v = [Status::Idle, Status::Blocked, Status::Working];
        v.sort_by_key(|s| s.rank());
        assert_eq!(v[0], Status::Blocked);
    }

    #[test]
    fn glyphs_match_spec() {
        assert_eq!(Status::Blocked.glyph(), "◉");
        assert_eq!(Status::Working.glyph(), "◍");
        assert_eq!(Status::Done.glyph(), "●");
        assert_eq!(Status::Idle.glyph(), "✓");
        assert_eq!(Status::Unknown.glyph(), "○");
    }
}
