use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::app::Focus;

pub struct PaneAreas {
    pub request_list: Rect,
    pub request_detail: Option<Rect>,
    pub response_pane: Rect,
}

pub fn pane_areas(
    size: (u16, u16),
    show_request_detail: bool,
    fullscreen: Option<Focus>,
) -> PaneAreas {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(Rect::new(0, 0, size.0, size.1));
    let body = areas[1];

    // Focus is never RequestDetail while the detail pane is hidden: App demotes
    // focus to RequestList when detail closes. Fall back to the split layout if
    // that invariant is ever violated.
    let fullscreen = match fullscreen {
        Some(Focus::RequestDetail) if !show_request_detail => None,
        fullscreen => fullscreen,
    };

    match fullscreen {
        Some(Focus::RequestList) => PaneAreas {
            request_list: body,
            request_detail: None,
            response_pane: Rect::ZERO,
        },
        Some(Focus::RequestDetail) => PaneAreas {
            request_list: Rect::ZERO,
            request_detail: Some(body),
            response_pane: Rect::ZERO,
        },
        Some(Focus::ResponsePane) => PaneAreas {
            request_list: Rect::ZERO,
            request_detail: None,
            response_pane: body,
        },
        None => {
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(body);

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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Focus;

    fn body_rect(size: (u16, u16)) -> Rect {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(Rect::new(0, 0, size.0, size.1));

        vertical[1]
    }

    #[test]
    fn test_pane_areas_matches_inline_layout_when_detail_closed() {
        let size = (80, 20);
        let areas = pane_areas(size, false, None);

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

    #[test]
    fn test_fullscreen_request_list_fills_body_with_detail_closed() {
        let areas = pane_areas((80, 20), false, Some(Focus::RequestList));

        assert_eq!(areas.request_list, body_rect((80, 20)));
        assert_eq!(areas.response_pane, Rect::ZERO);
        assert_eq!(areas.request_detail, None);
    }

    #[test]
    fn test_fullscreen_request_list_fills_body_with_detail_open() {
        let areas = pane_areas((80, 20), true, Some(Focus::RequestList));

        assert_eq!(areas.request_list, body_rect((80, 20)));
        assert_eq!(areas.response_pane, Rect::ZERO);
        assert_eq!(areas.request_detail, None);
    }

    #[test]
    fn test_fullscreen_response_pane_fills_body_with_detail_closed() {
        let areas = pane_areas((80, 20), false, Some(Focus::ResponsePane));

        assert_eq!(areas.request_list, Rect::ZERO);
        assert_eq!(areas.response_pane, body_rect((80, 20)));
        assert_eq!(areas.request_detail, None);
    }

    #[test]
    fn test_fullscreen_response_pane_fills_body_with_detail_open() {
        let areas = pane_areas((80, 20), true, Some(Focus::ResponsePane));

        assert_eq!(areas.request_list, Rect::ZERO);
        assert_eq!(areas.response_pane, body_rect((80, 20)));
        assert_eq!(areas.request_detail, None);
    }

    #[test]
    fn test_fullscreen_request_detail_fills_body_when_open() {
        let areas = pane_areas((80, 20), true, Some(Focus::RequestDetail));

        assert_eq!(areas.request_list, Rect::ZERO);
        assert_eq!(areas.response_pane, Rect::ZERO);
        assert_eq!(areas.request_detail, Some(body_rect((80, 20))));
    }
}
