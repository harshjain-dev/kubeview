use ratatui::style::Color;

/// Semantic color palette — every UI element maps to one of these.
pub struct Theme {
    pub bg: Color,       // terminal background fill
    pub overlay: Color,  // title bar / slightly elevated surface
    pub surface: Color,  // selection row bg, status bar bg
    pub fg: Color,       // primary foreground text
    pub muted: Color,    // column headers, dimmed labels
    pub accent: Color,   // tabs, key hints, hostnames, primary highlights
    pub secondary: Color,// namespace, ports, secondary info
    pub success: Color,  // Running pods, ready deployments
    pub warning: Color,  // Pending, Warning events, yellow states
    pub danger: Color,   // Failed, error, prod context, quit key
    pub info: Color,     // Succeeded pods, informational
    pub special: Color,  // TSH cluster, Terminating pods
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThemeVariant {
    Default,
    Dracula,
    Nord,
    TokyoNight,
}

impl ThemeVariant {
    pub const ALL: [ThemeVariant; 4] = [
        ThemeVariant::Default,
        ThemeVariant::Dracula,
        ThemeVariant::Nord,
        ThemeVariant::TokyoNight,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ThemeVariant::Default => "Default",
            ThemeVariant::Dracula => "Dracula",
            ThemeVariant::Nord => "Nord",
            ThemeVariant::TokyoNight => "Tokyo Night",
        }
    }

    pub fn next(self) -> ThemeVariant {
        let idx = ThemeVariant::ALL.iter().position(|t| *t == self).unwrap_or(0);
        ThemeVariant::ALL[(idx + 1) % ThemeVariant::ALL.len()]
    }

    pub fn colors(self) -> Theme {
        match self {
            ThemeVariant::Default => Theme {
                bg:        Color::Rgb(13, 13, 17),
                overlay:   Color::Rgb(22, 22, 28),
                surface:   Color::Rgb(40, 42, 54),
                fg:        Color::Rgb(220, 220, 230),
                muted:     Color::Rgb(90, 92, 110),
                accent:    Color::Rgb(80, 200, 220),
                secondary: Color::Rgb(250, 200, 70),
                success:   Color::Rgb(80, 200, 120),
                warning:   Color::Rgb(250, 180, 50),
                danger:    Color::Rgb(230, 80, 80),
                info:      Color::Rgb(100, 150, 230),
                special:   Color::Rgb(200, 120, 230),
            },
            ThemeVariant::Dracula => Theme {
                bg:        Color::Rgb(40, 42, 54),
                overlay:   Color::Rgb(33, 34, 44),
                surface:   Color::Rgb(68, 71, 90),
                fg:        Color::Rgb(248, 248, 242),
                muted:     Color::Rgb(98, 114, 164),
                accent:    Color::Rgb(255, 121, 198),
                secondary: Color::Rgb(241, 250, 140),
                success:   Color::Rgb(80, 250, 123),
                warning:   Color::Rgb(255, 184, 108),
                danger:    Color::Rgb(255, 85, 85),
                info:      Color::Rgb(139, 233, 253),
                special:   Color::Rgb(189, 147, 249),
            },
            ThemeVariant::Nord => Theme {
                bg:        Color::Rgb(46, 52, 64),
                overlay:   Color::Rgb(40, 45, 56),
                surface:   Color::Rgb(67, 76, 94),
                fg:        Color::Rgb(236, 239, 244),
                muted:     Color::Rgb(76, 86, 106),
                accent:    Color::Rgb(136, 192, 208),
                secondary: Color::Rgb(129, 161, 193),
                success:   Color::Rgb(163, 190, 140),
                warning:   Color::Rgb(235, 203, 139),
                danger:    Color::Rgb(191, 97, 106),
                info:      Color::Rgb(94, 129, 172),
                special:   Color::Rgb(180, 142, 173),
            },
            ThemeVariant::TokyoNight => Theme {
                bg:        Color::Rgb(26, 27, 38),
                overlay:   Color::Rgb(20, 22, 30),
                surface:   Color::Rgb(41, 46, 66),
                fg:        Color::Rgb(169, 177, 214),
                muted:     Color::Rgb(86, 95, 137),
                accent:    Color::Rgb(122, 162, 247),
                secondary: Color::Rgb(224, 175, 104),
                success:   Color::Rgb(158, 206, 106),
                warning:   Color::Rgb(224, 175, 104),
                danger:    Color::Rgb(247, 118, 142),
                info:      Color::Rgb(125, 207, 255),
                special:   Color::Rgb(187, 154, 247),
            },
        }
    }
}
