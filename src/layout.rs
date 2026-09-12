use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct PaneAreas {
    pub request_list: Rect,
    pub request_detail: Option<Rect>,
    pub response_pane: Rect,
}

pub fn pane_areas(size: (u16, u16), show_request_detail: bool) -> PaneAreas {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(Rect::new(0, 0, size.0, size.1));

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(areas[1]);

    if show_request_detail {
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(columns[1]);
        PaneAreas {
            request_list: columns[0],
            request_detail: Some(right[0]),
            response_pane: right[1],
        }
    } else {
        PaneAreas {
            request_list: columns[0],
            request_detail: None,
            response_pane: columns[1],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_areas_matches_inline_layout_when_detail_closed() {
        let size = (80, 20);
        let areas = pane_areas(size, false);

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(Rect::new(0, 0, 80, 20));
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(vertical[1]);

        assert_eq!(areas.request_list, columns[0]);
        assert_eq!(areas.response_pane, columns[1]);
        assert_eq!(areas.request_detail, None);
    }
}
