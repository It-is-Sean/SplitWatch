use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

#[derive(Clone, Copy)]
pub(crate) struct ModalAction<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub color: Color,
}

pub(crate) struct ModalActions<'a> {
    items: Vec<ModalAction<'a>>,
    background: Color,
    separator: Color,
    right_padding: u16,
}

impl<'a> ModalActions<'a> {
    pub(crate) fn new(
        items: impl Into<Vec<ModalAction<'a>>>,
        background: Color,
        separator: Color,
    ) -> Self {
        Self {
            items: items.into(),
            background,
            separator,
            right_padding: 2,
        }
    }

    pub(crate) fn render(&self, frame: &mut Frame, modal: Rect) {
        let Some(group) = self.group_rect(modal) else {
            return;
        };
        frame.render_widget(
            Paragraph::new(Line::from(self.spans())).style(Style::default().bg(self.background)),
            group,
        );
    }

    pub(crate) fn hit_test(&self, modal: Rect, x: u16, y: u16) -> Option<&'a str> {
        if self.items.is_empty() {
            return None;
        }
        for action in self.action_rects(modal) {
            if x >= action.rect.x
                && x < action.rect.x + action.rect.width
                && y >= action.rect.y
                && y < action.rect.y + action.rect.height
            {
                return Some(action.id);
            }
        }
        None
    }

    pub(crate) fn group_rect(&self, modal: Rect) -> Option<Rect> {
        if self.items.is_empty() || modal.width < 4 || modal.height < 1 {
            return None;
        }
        let width = self.group_width();
        let required = width.saturating_add(self.right_padding).saturating_add(1);
        if modal.width <= required {
            return None;
        }
        let x = modal.x + modal.width.saturating_sub(required);
        Some(Rect::new(
            x,
            modal.y + modal.height.saturating_sub(1),
            width,
            1,
        ))
    }

    fn action_rects(&self, modal: Rect) -> Vec<ActionRect<'a>> {
        let Some(group) = self.group_rect(modal) else {
            return Vec::new();
        };

        let mut rects = Vec::with_capacity(self.items.len());
        let mut x = group.x + 1;
        for (idx, item) in self.items.iter().enumerate() {
            let width = item.label.chars().count() as u16;
            rects.push(ActionRect {
                id: item.id,
                rect: Rect::new(x, group.y, width, 1),
            });
            x += width;
            if idx + 1 < self.items.len() {
                x += 3;
            }
        }
        rects
    }

    fn group_width(&self) -> u16 {
        let label_width: u16 = self
            .items
            .iter()
            .map(|item| item.label.chars().count() as u16)
            .sum();
        let separators = self.items.len().saturating_sub(1) as u16 * 3;
        2 + label_width + separators
    }

    fn spans(&self) -> Vec<Span<'a>> {
        let mut spans = Vec::with_capacity(self.items.len() * 2 + 1);
        spans.push(Span::styled(
            " ".to_string(),
            Style::default().bg(self.background),
        ));
        for (idx, item) in self.items.iter().enumerate() {
            spans.push(Span::styled(
                item.label.to_string(),
                Style::default().fg(item.color).bg(self.background),
            ));
            if idx + 1 < self.items.len() {
                spans.push(Span::styled(
                    " · ".to_string(),
                    Style::default().fg(self.separator).bg(self.background),
                ));
            }
        }
        spans.push(Span::styled(
            " ".to_string(),
            Style::default().bg(self.background),
        ));
        spans
    }
}

struct ActionRect<'a> {
    id: &'a str,
    rect: Rect,
}
