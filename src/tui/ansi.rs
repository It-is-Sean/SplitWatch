use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

pub(crate) fn ansi_text(input: &str, base: Style) -> Text<'static> {
    let mut parser = AnsiParser::new(base);
    parser.parse(input)
}

struct AnsiParser {
    base: Style,
    style: Style,
}

impl AnsiParser {
    fn new(base: Style) -> Self {
        Self { base, style: base }
    }

    fn parse(&mut self, input: &str) -> Text<'static> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut buf = String::new();
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            match chars[i] {
                '\x1b' if i + 1 < chars.len() => {
                    self.flush_buf(&mut buf, &mut spans);
                    match chars[i + 1] {
                        '[' => {
                            i += 2;
                            let mut seq = String::new();
                            while i < chars.len() && !is_csi_final(chars[i]) {
                                seq.push(chars[i]);
                                i += 1;
                            }
                            if i < chars.len() && chars[i] == 'm' {
                                self.apply_sgr(&seq);
                            }
                        }
                        ']' => {
                            i += 2;
                            while i < chars.len() {
                                if chars[i] == '\x07' {
                                    break;
                                }
                                if chars[i] == '\x1b' && i + 1 < chars.len() && chars[i + 1] == '\\'
                                {
                                    i += 1;
                                    break;
                                }
                                i += 1;
                            }
                        }
                        _ => {}
                    }
                }
                '\r' => {}
                '\t' => buf.push_str("    "),
                '\n' => {
                    self.flush_buf(&mut buf, &mut spans);
                    lines.push(Line::from(std::mem::take(&mut spans)));
                }
                ch => buf.push(ch),
            }
            i += 1;
        }

        self.flush_buf(&mut buf, &mut spans);
        lines.push(Line::from(spans));
        Text::from(lines)
    }

    fn flush_buf(&self, buf: &mut String, spans: &mut Vec<Span<'static>>) {
        if !buf.is_empty() {
            spans.push(Span::styled(std::mem::take(buf), self.style));
        }
    }

    fn apply_sgr(&mut self, seq: &str) {
        let codes = if seq.is_empty() {
            vec![0]
        } else {
            seq.split(';')
                .filter_map(|part| part.parse::<u16>().ok())
                .collect::<Vec<_>>()
        };

        let mut i = 0;
        while i < codes.len() {
            match codes[i] {
                0 => self.style = self.base,
                1 => self.style = self.style.add_modifier(Modifier::BOLD),
                2 => self.style = self.style.add_modifier(Modifier::DIM),
                3 => self.style = self.style.add_modifier(Modifier::ITALIC),
                4 => self.style = self.style.add_modifier(Modifier::UNDERLINED),
                5 => self.style = self.style.add_modifier(Modifier::SLOW_BLINK),
                6 => self.style = self.style.add_modifier(Modifier::RAPID_BLINK),
                7 => self.style = self.style.add_modifier(Modifier::REVERSED),
                8 => self.style = self.style.add_modifier(Modifier::HIDDEN),
                9 => self.style = self.style.add_modifier(Modifier::CROSSED_OUT),
                22 => self.style = self.style.remove_modifier(Modifier::BOLD),
                23 => self.style = self.style.remove_modifier(Modifier::ITALIC),
                24 => self.style = self.style.remove_modifier(Modifier::UNDERLINED),
                25 => {
                    self.style = self
                        .style
                        .remove_modifier(Modifier::SLOW_BLINK | Modifier::RAPID_BLINK)
                }
                27 => self.style = self.style.remove_modifier(Modifier::REVERSED),
                28 => self.style = self.style.remove_modifier(Modifier::HIDDEN),
                29 => self.style = self.style.remove_modifier(Modifier::CROSSED_OUT),
                30..=37 => self.style.fg = Some(basic_color(codes[i] - 30, false)),
                39 => self.style.fg = self.base.fg,
                40..=47 => self.style.bg = Some(basic_color(codes[i] - 40, false)),
                49 => self.style.bg = self.base.bg,
                90..=97 => self.style.fg = Some(basic_color(codes[i] - 90, true)),
                100..=107 => self.style.bg = Some(basic_color(codes[i] - 100, true)),
                38 | 48 => {
                    let is_fg = codes[i] == 38;
                    if let Some((color, consumed)) = extended_color(&codes[i + 1..]) {
                        if is_fg {
                            self.style.fg = Some(color);
                        } else {
                            self.style.bg = Some(color);
                        }
                        i += consumed;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

fn is_csi_final(ch: char) -> bool {
    ('@'..='~').contains(&ch)
}

fn extended_color(codes: &[u16]) -> Option<(Color, usize)> {
    match codes {
        [5, idx, ..] => Some((Color::Indexed(*idx as u8), 2)),
        [2, r, g, b, ..] => Some((Color::Rgb(*r as u8, *g as u8, *b as u8), 4)),
        _ => None,
    }
}

fn basic_color(index: u16, bright: bool) -> Color {
    match (index, bright) {
        (0, false) => Color::Black,
        (1, false) => Color::Red,
        (2, false) => Color::Green,
        (3, false) => Color::Yellow,
        (4, false) => Color::Blue,
        (5, false) => Color::Magenta,
        (6, false) => Color::Cyan,
        (7, false) => Color::Gray,
        (0, true) => Color::DarkGray,
        (1, true) => Color::LightRed,
        (2, true) => Color::LightGreen,
        (3, true) => Color::LightYellow,
        (4, true) => Color::LightBlue,
        (5, true) => Color::LightMagenta,
        (6, true) => Color::LightCyan,
        _ => Color::White,
    }
}

#[cfg(test)]
mod tests {
    use super::ansi_text;
    use ratatui::style::{Color, Modifier, Style};

    #[test]
    fn parses_basic_sgr_styles() {
        let text = ansi_text("\x1b[31mred\x1b[0m plain", Style::default());
        assert_eq!(text.lines.len(), 1);
        assert_eq!(text.lines[0].spans[0].content.as_ref(), "red");
        assert_eq!(text.lines[0].spans[0].style.fg, Some(Color::Red));
        assert_eq!(text.lines[0].spans[1].content.as_ref(), " plain");
    }

    #[test]
    fn parses_bold_truecolor() {
        let text = ansi_text(
            "\x1b[1;38;2;1;2;3mhi\x1b[0m",
            Style::default().fg(Color::White),
        );
        let span = &text.lines[0].spans[0];
        assert_eq!(span.style.fg, Some(Color::Rgb(1, 2, 3)));
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn strips_osc_and_non_sgr_csi_sequences() {
        let text = ansi_text(
            "a\x1b]8;;https://example.com\x07link\x1b]8;;\x07b\x1b[2Kc",
            Style::default(),
        );
        let rendered = text.lines[0]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(rendered, "alinkbc");
    }

    #[test]
    fn parses_extended_modifiers() {
        let text = ansi_text("\x1b[3;4;9mstyled\x1b[0m", Style::default());
        let span = &text.lines[0].spans[0];
        assert!(span.style.add_modifier.contains(Modifier::ITALIC));
        assert!(span.style.add_modifier.contains(Modifier::UNDERLINED));
        assert!(span.style.add_modifier.contains(Modifier::CROSSED_OUT));
    }
}
