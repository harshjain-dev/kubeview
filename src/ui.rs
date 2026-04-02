use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState, Tabs, Wrap},
    Frame,
};

use crate::app::{App, InputMode, Tab};
use crate::theme::Theme;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    if area.width < 40 || area.height < 10 {
        return;
    }

    // Fill the entire frame with the theme background
    let t = app.theme.colors();
    frame.render_widget(
        Block::default().style(Style::default().bg(t.bg).fg(t.fg)),
        area,
    );

    match app.input_mode {
        InputMode::Viewing => {
            draw_main_layout(frame, app, area);
            draw_viewer(frame, app, area);
            return;
        }
        InputMode::ClusterPicker => {
            draw_main_layout(frame, app, area);
            draw_cluster_picker(frame, app, area);
            return;
        }
        InputMode::Help => {
            draw_main_layout(frame, app, area);
            draw_help(frame, app, area);
            return;
        }
        InputMode::PathInput => {
            draw_main_layout(frame, app, area);
            draw_path_input(frame, app, area);
            return;
        }
        InputMode::ScaleInput => {
            draw_main_layout(frame, app, area);
            draw_scale_input(frame, app, area);
            return;
        }
        InputMode::ContainerPicker => {
            draw_main_layout(frame, app, area);
            draw_container_picker(frame, app, area);
            return;
        }
        InputMode::PortInput => {
            draw_main_layout(frame, app, area);
            draw_port_input(frame, app, area);
            return;
        }
        InputMode::Confirm => {
            draw_main_layout(frame, app, area);
            draw_confirm_dialog(frame, app, area);
            return;
        }
        InputMode::SecretKeyPicker => {
            draw_main_layout(frame, app, area);
            draw_secret_key_picker(frame, app, area);
            return;
        }
        InputMode::SecretValueInput => {
            draw_main_layout(frame, app, area);
            draw_secret_value_input(frame, app, area);
            return;
        }
        _ => {}
    }

    draw_main_layout(frame, app, area);
}

fn draw_main_layout(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Length(2), // Tabs
            Constraint::Min(5),    // Content
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    draw_title_bar(frame, app, chunks[0]);
    draw_tabs(frame, app, chunks[1]);
    draw_content(frame, app, chunks[2]);
    draw_status_bar(frame, app, chunks[3]);
}

fn draw_title_bar(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 2 || area.height < 1 {
        return;
    }

    let t = app.theme.colors();
    let ctx = &app.current_context;
    let is_prod = ctx.contains("prod") || ctx.contains("production");

    let ctx_style = if is_prod {
        Style::default().fg(t.danger).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.accent)
    };

    let title_line = Line::from(vec![
        Span::styled(" kubeview  ctx:", Style::default().fg(t.muted)),
        Span::styled(ctx, ctx_style),
        Span::styled("  ns:", Style::default().fg(t.muted)),
        Span::styled(&app.current_namespace, Style::default().fg(t.secondary)),
        Span::styled(
            format!("  [{}]", app.theme.name()),
            Style::default().fg(t.muted),
        ),
    ]);

    let title_bar = Paragraph::new(title_line)
        .style(Style::default().bg(t.overlay).fg(t.fg));
    frame.render_widget(title_bar, area);
}

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 2 || area.height < 1 {
        return;
    }

    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let num = i + 1;
            Line::from(format!(" {num}:{} ", tab.title()))
        })
        .collect();

    let t = app.theme.colors();
    let tabs = Tabs::new(titles)
        .select(app.active_tab)
        .style(Style::default().bg(t.overlay).fg(t.muted))
        .highlight_style(Style::default().fg(t.accent).add_modifier(Modifier::BOLD))
        .divider("│");
    frame.render_widget(tabs, area);
}

fn draw_content(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 10 || area.height < 3 {
        return;
    }

    match Tab::ALL[app.active_tab] {
        Tab::Pods => draw_pods_tab(frame, app, area),
        Tab::Services => draw_services_tab(frame, app, area),
        Tab::Deployments => draw_deployments_tab(frame, app, area),
        Tab::Ingresses => draw_ingresses_tab(frame, app, area),
        Tab::Secrets => draw_secrets_tab(frame, app, area),
        Tab::Events => draw_events_tab(frame, app, area),
    }
}

// ── Pods tab ──────────────────────────────────────────────────────────────────

fn draw_pods_tab(frame: &mut Frame, app: &App, area: Rect) {
    let has_detail_space = area.width >= 80;

    let chunks = if has_detail_space {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(area)
    };

    draw_pod_list(frame, app, chunks[0]);

    if has_detail_space && chunks.len() > 1 {
        draw_pod_detail(frame, app, chunks[1]);
    }
}

fn draw_pod_list(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 10 || area.height < 3 {
        return;
    }

    let t = app.theme.colors();
    let header_cells = ["NAME", "STATUS", "READY", "RESTARTS", "AGE"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(t.muted)));
    let header = Row::new(header_cells)
        .height(1)
        .style(Style::default().bg(t.bg));

    let rows: Vec<Row> = app
        .filtered_pods
        .iter()
        .enumerate()
        .map(|(i, pod)| {
            let is_selected = i == app.pod_selected;
            let row_style = if is_selected {
                Style::default().bg(t.surface).fg(t.fg).add_modifier(Modifier::BOLD)
            } else if pod.restarts > 20 {
                Style::default().fg(t.danger)
            } else if pod.restarts > 5 {
                Style::default().fg(t.warning)
            } else {
                Style::default().fg(t.fg)
            };

            let status_style = Style::default().fg(pod.status_color());

            Row::new(vec![
                Cell::from(pod.name.clone()),
                Cell::from(pod.status.clone()).style(status_style),
                Cell::from(pod.ready.clone()),
                Cell::from(pod.restarts.to_string()),
                Cell::from(pod.age.clone()),
            ])
            .style(row_style)
        })
        .collect();

    let title = if app.input_mode == InputMode::Search {
        format!("Pods [{}/{}] /{}",
            app.pod_selected + 1,
            app.filtered_pods.len(),
            app.search_query)
    } else {
        format!("Pods [{}/{}]", app.pod_selected + 1, app.filtered_pods.len())
    };

    let available = area.width.saturating_sub(2);
    let name_width = available.saturating_mul(40) / 100;
    let status_width = available.saturating_mul(20) / 100;
    let ready_width = available.saturating_mul(15) / 100;
    let restarts_width = available.saturating_mul(12) / 100;
    let age_width =
        available.saturating_sub(name_width + status_width + ready_width + restarts_width);

    let table = Table::new(
        rows,
        [
            Constraint::Length(name_width),
            Constraint::Length(status_width),
            Constraint::Length(ready_width),
            Constraint::Length(restarts_width),
            Constraint::Length(age_width),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(t.surface)
            .fg(t.accent)
            .add_modifier(Modifier::BOLD),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(t.bg).fg(t.fg)),
    );

    let mut state = TableState::default();
    state.select(Some(app.pod_selected));

    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_pod_detail(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 10 || area.height < 3 {
        return;
    }

    let t = app.theme.colors();

    let detail_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(10).min(10)),
            Constraint::Min(5),
        ])
        .split(area);

    if let Some(pod) = app.selected_pod() {
        let info_lines = vec![
            Line::from(vec![
                Span::styled("Name:    ", Style::default().fg(t.muted)),
                Span::styled(&pod.name, Style::default().fg(t.fg)),
            ]),
            Line::from(vec![
                Span::styled("Status:  ", Style::default().fg(t.muted)),
                Span::styled(&pod.status, Style::default().fg(pod.status_color())),
            ]),
            Line::from(vec![
                Span::styled("Ready:   ", Style::default().fg(t.muted)),
                Span::styled(&pod.ready, Style::default().fg(t.fg)),
            ]),
            Line::from(vec![
                Span::styled("Image:   ", Style::default().fg(t.muted)),
                Span::styled(&pod.image, Style::default().fg(t.fg)),
            ]),
            Line::from(vec![
                Span::styled("Node:    ", Style::default().fg(t.muted)),
                Span::styled(&pod.node, Style::default().fg(t.fg)),
            ]),
            Line::from(vec![
                Span::styled("IP:      ", Style::default().fg(t.muted)),
                Span::styled(&pod.ip, Style::default().fg(t.secondary)),
            ]),
            Line::from(vec![
                Span::styled("Restarts:", Style::default().fg(t.muted)),
                Span::styled(
                    format!(" {}", pod.restarts),
                    Style::default().fg(if pod.restarts > 5 { t.warning } else { t.fg }),
                ),
            ]),
        ];

        let info = Paragraph::new(info_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Detail")
                    .style(Style::default().bg(t.bg).fg(t.fg)),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(info, detail_chunks[0]);
    } else {
        let empty = Paragraph::new("No pod selected")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Detail")
                    .style(Style::default().bg(t.bg).fg(t.fg)),
            );
        frame.render_widget(empty, detail_chunks[0]);
    }

    let key = |k: &'static str, c| Span::styled(k, Style::default().fg(c).add_modifier(Modifier::BOLD));
    let txt = |s: &'static str| Span::styled(s, Style::default().fg(t.fg));

    let actions = vec![
        Line::from(vec![key(" l ", t.accent), txt("logs           "), key(" s ", t.success), txt("svc-log")]),
        Line::from(vec![key(" e ", t.accent), txt("exec shell     "), key(" p ", t.warning), txt("port-fwd")]),
        Line::from(vec![key(" d ", t.accent), txt("describe       "), key(" y ", t.accent), txt("YAML")]),
        Line::from(vec![key(" P ", t.warning), txt("port-fwd list  "), key(" H ", t.accent), txt("helm")]),
        Line::from(vec![key(" c ", t.special), txt("TSH cluster    "), key(" / ", t.accent), txt("search")]),
        Line::from(vec![key(" T ", t.special), txt("theme          "), key(" ? ", t.muted), txt("help")]),
        Line::from(vec![key(" q ", t.danger), txt("quit")]),
    ];

    let actions_widget = Paragraph::new(actions)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Actions")
                .style(Style::default().bg(t.bg).fg(t.fg)),
        );
    frame.render_widget(actions_widget, detail_chunks[1]);
}

// ── Services tab ──────────────────────────────────────────────────────────────

fn draw_services_tab(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 10 || area.height < 3 {
        return;
    }

    let t = app.theme.colors();
    let header_cells = ["NAME", "TYPE", "CLUSTER-IP", "EXTERNAL-IP", "PORTS", "AGE"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(t.muted)));
    let header = Row::new(header_cells)
        .height(1)
        .style(Style::default().bg(t.bg));

    let rows: Vec<Row> = app
        .services
        .iter()
        .enumerate()
        .map(|(i, svc)| {
            let selected = i == app.svc_selected;
            Row::new(vec![
                Cell::from(svc.name.clone()),
                Cell::from(svc.type_.clone()).style(Style::default().fg(t.secondary)),
                Cell::from(svc.cluster_ip.clone()),
                Cell::from(svc.external_ip.clone()).style(Style::default().fg(t.accent)),
                Cell::from(svc.ports.clone()).style(Style::default().fg(t.info)),
                Cell::from(svc.age.clone()),
            ])
            .style(if selected {
                Style::default().bg(t.surface).fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            })
        })
        .collect();

    let title = format!("Services ({})", app.services.len());

    let available = area.width.saturating_sub(2);
    let name_w = available.saturating_mul(28) / 100;
    let type_w = available.saturating_mul(13) / 100;
    let cip_w = available.saturating_mul(15) / 100;
    let eip_w = available.saturating_mul(15) / 100;
    let ports_w = available.saturating_mul(20) / 100;
    let age_w = available.saturating_sub(name_w + type_w + cip_w + eip_w + ports_w);

    if app.services.is_empty() {
        let placeholder = Paragraph::new("\n  No services found. Press r to refresh.")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .style(Style::default().bg(t.bg).fg(t.fg)),
            )
            .style(Style::default().fg(t.muted));
        frame.render_widget(placeholder, area);
        return;
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(name_w),
            Constraint::Length(type_w),
            Constraint::Length(cip_w),
            Constraint::Length(eip_w),
            Constraint::Length(ports_w),
            Constraint::Length(age_w),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(t.surface)
            .fg(t.accent)
            .add_modifier(Modifier::BOLD),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(t.bg).fg(t.fg)),
    );

    let mut state = TableState::default();
    state.select(Some(app.svc_selected));
    frame.render_stateful_widget(table, area, &mut state);
}

// ── Status bar ────────────────────────────────────────────────────────────────

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 2 || area.height < 1 {
        return;
    }

    let t = app.theme.colors();

    let mode_str = match app.input_mode {
        InputMode::Normal => "NORMAL",
        InputMode::Search => "SEARCH",
        InputMode::Viewing => "VIEW",
        InputMode::ClusterPicker => "TSH",
        InputMode::Help => "HELP",
        InputMode::PathInput => "PATH",
        InputMode::ScaleInput => "SCALE",
        InputMode::ContainerPicker => "CONTAINER",
        InputMode::PortInput => "PORT-FWD",
        InputMode::Confirm => "CONFIRM",
        InputMode::SecretKeyPicker => "SECRET-KEY",
        InputMode::SecretValueInput => "SECRET-EDIT",
    };

    let loading_indicator = if app.loading { " ⟳ " } else { "" };
    let status_text = format!(
        " {}{} │ {} │ {} │  j/k:nav  /:search  l:logs  s:svc-log  d:desc  c:cluster  ?:help  q:quit",
        mode_str, loading_indicator, app.current_context, app.status_message
    );

    let max_width = area.width as usize;
    let display: String = status_text.chars().take(max_width).collect();

    let bar = Paragraph::new(display)
        .style(Style::default().bg(t.surface).fg(t.fg));
    frame.render_widget(bar, area);
}

// ── Viewer overlay ────────────────────────────────────────────────────────────

fn draw_viewer(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 10 || area.height < 5 {
        return;
    }

    let t = app.theme.colors();

    frame.render_widget(Clear, area);
    // Re-fill background after Clear
    frame.render_widget(
        Block::default().style(Style::default().bg(t.bg)),
        area,
    );

    let margin = if area.width > 44 { 2 } else { 0 };
    let inner = Rect {
        x: area.x + margin,
        y: area.y + 1,
        width: area.width.saturating_sub(margin * 2),
        height: area.height.saturating_sub(2),
    };

    let visible_height = inner.height.saturating_sub(2) as usize;
    let total_lines = app.viewer_content.len();

    let start = if total_lines > visible_height {
        app.viewer_scroll
            .min(total_lines.saturating_sub(visible_height))
    } else {
        0
    };

    let visible_lines: Vec<Line> = app
        .viewer_content
        .iter()
        .skip(start)
        .take(visible_height)
        .map(|l| colorize_log_line(l, &t))
        .collect();

    let is_live = app.log_stream.is_some();
    let live_indicator = if is_live { " ● LIVE" } else { "" };

    let scroll_info = if total_lines > 0 {
        format!(
            "{}{} [{}/{}]",
            app.viewer_title,
            live_indicator,
            start + 1,
            total_lines
        )
    } else {
        format!("{}{}", app.viewer_title, live_indicator)
    };

    let bottom_hint = if is_live {
        " Esc:stop  j/k:scroll  g/G:top/bottom "
    } else {
        " Esc:close  j/k:scroll  g/G:top/bottom "
    };

    let viewer = Paragraph::new(visible_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(scroll_info)
            .title_bottom(bottom_hint)
            .style(Style::default().bg(t.bg).fg(t.fg)),
    );

    frame.render_widget(viewer, inner);
}

// ── TSH cluster picker popup ──────────────────────────────────────────────────

fn draw_cluster_picker(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();

    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = (app.tsh_clusters.len() as u16 + 4).min(area.height.saturating_sub(4));

    let popup = centered_rect(popup_width, popup_height, area);
    frame.render_widget(Clear, popup);

    let items: Vec<ListItem> = app
        .tsh_clusters
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == app.cluster_picker_index {
                Style::default()
                    .fg(t.accent)
                    .add_modifier(Modifier::BOLD)
                    .bg(t.surface)
            } else {
                Style::default().fg(t.fg)
            };
            ListItem::new(format!("  {name}  ")).style(style)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.cluster_picker_index));

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" TSH Clusters (tsh kube login) ")
            .title_bottom(" j/k:nav  Enter:login  Esc:cancel ")
            .style(Style::default().bg(t.overlay).fg(t.fg)),
    );

    frame.render_stateful_widget(list, popup, &mut list_state);
}

// ── Help popup ────────────────────────────────────────────────────────────────

fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();

    let popup_width = 54u16.min(area.width.saturating_sub(4));
    let popup_height = 28u16.min(area.height.saturating_sub(4));

    let popup = centered_rect(popup_width, popup_height, area);
    frame.render_widget(Clear, popup);

    let key = |k: &'static str| Span::styled(k, Style::default().fg(t.accent).add_modifier(Modifier::BOLD));
    let txt = |s: &'static str| Span::styled(s, Style::default().fg(t.fg));

    let lines = vec![
        Line::from(vec![key(" Navigation (all tabs) "), txt("")]),
        Line::from(vec![key("  j/k ↑↓      "), txt("  Move up/down")]),
        Line::from(vec![key("  g / G       "), txt("  Jump top/bottom")]),
        Line::from(vec![key("  Tab/Shift+Tab "), txt("  Next/prev tab")]),
        Line::from(vec![key("  1–6         "), txt("  Switch tab directly")]),
        Line::from(vec![key("  n           "), txt("  Cycle namespace")]),
        Line::from(vec![key("  c           "), txt("  TSH cluster picker")]),
        Line::from(Span::raw("")),
        Line::from(vec![key(" Pods (tab 1) "), txt("")]),
        Line::from(vec![key("  l           "), txt("  kubectl logs (last 200)")]),
        Line::from(vec![key("  s           "), txt("  tail service log (live, path prompt)")]),
        Line::from(vec![key("  e           "), txt("  exec into pod shell")]),
        Line::from(vec![key("  p           "), txt("  port-forward (local:remote)")]),
        Line::from(vec![key("  P           "), txt("  view active port-forwards")]),
        Line::from(vec![key("  d           "), txt("  describe pod")]),
        Line::from(vec![key("  y           "), txt("  YAML view")]),
        Line::from(vec![key("  /           "), txt("  fuzzy search")]),
        Line::from(Span::raw("")),
        Line::from(vec![key(" Deployments (tab 3) "), txt("")]),
        Line::from(vec![key("  s           "), txt("  scale replicas")]),
        Line::from(vec![key("  r           "), txt("  rollout restart (confirm)")]),
        Line::from(vec![key("  d           "), txt("  describe")]),
        Line::from(vec![key("  y           "), txt("  YAML view")]),
        Line::from(Span::raw("")),
        Line::from(vec![key(" Ingresses (tab 4) / Secrets (tab 5) "), txt("")]),
        Line::from(vec![key("  y/Enter     "), txt("  view YAML / decoded data")]),
        Line::from(vec![key("  r           "), txt("  refresh")]),
        Line::from(Span::raw("")),
        Line::from(vec![key(" Common "), txt("")]),
        Line::from(vec![key("  r           "), txt("  refresh current tab")]),
        Line::from(vec![key("  H           "), txt("  helm list")]),
        Line::from(vec![key("  Esc/q       "), txt("  close viewer")]),
        Line::from(vec![key("  T           "), txt("  cycle theme (Default/Dracula/Nord/Tokyo Night)")]),
        Line::from(vec![key("  ?           "), txt("  this help")]),
        Line::from(vec![key("  q           "), txt("  quit")]),
    ];

    let help = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .title_alignment(Alignment::Center)
                .title_bottom(" Esc / q / ? to close ")
                .style(Style::default().bg(t.overlay).fg(t.fg)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(help, popup);
}

// ── Deployments tab ───────────────────────────────────────────────────────────

fn draw_deployments_tab(frame: &mut Frame, app: &App, area: Rect) {
    let has_detail = area.width >= 80;
    let chunks = if has_detail {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(area)
    };

    draw_deploy_list(frame, app, chunks[0]);
    if has_detail && chunks.len() > 1 {
        draw_deploy_detail(frame, app, chunks[1]);
    }
}

fn draw_deploy_list(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();

    let header = Row::new(
        ["NAME", "READY", "UP-TO-DATE", "AVAILABLE", "AGE"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(t.muted))),
    )
    .height(1)
    .style(Style::default().bg(t.bg));

    let rows: Vec<Row> = app
        .deployments
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let selected = i == app.deploy_selected;
            let ready_parts: Vec<&str> = d.ready.splitn(2, '/').collect();
            let ready_color = if ready_parts.len() == 2
                && ready_parts[0] == ready_parts[1]
                && ready_parts[0] != "0"
            {
                t.success
            } else {
                t.warning
            };
            Row::new(vec![
                Cell::from(d.name.clone()),
                Cell::from(d.ready.clone()).style(Style::default().fg(ready_color)),
                Cell::from(d.up_to_date.to_string()),
                Cell::from(d.available.to_string()),
                Cell::from(d.age.clone()),
            ])
            .style(if selected {
                Style::default().bg(t.surface).fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            })
        })
        .collect();

    let title = format!("Deployments ({})", app.deployments.len());
    let avail = area.width.saturating_sub(2);
    let nw = avail * 40 / 100;
    let rw = avail * 15 / 100;
    let uw = avail * 15 / 100;
    let aw = avail * 15 / 100;
    let agew = avail.saturating_sub(nw + rw + uw + aw);

    let table = Table::new(
        rows,
        [
            Constraint::Length(nw),
            Constraint::Length(rw),
            Constraint::Length(uw),
            Constraint::Length(aw),
            Constraint::Length(agew),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(t.surface)
            .fg(t.accent)
            .add_modifier(Modifier::BOLD),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(t.bg).fg(t.fg)),
    );

    let mut state = TableState::default();
    state.select(Some(app.deploy_selected));
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_deploy_detail(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(8)])
        .split(area);

    if let Some(d) = app.selected_deployment() {
        let ready_parts: Vec<&str> = d.ready.splitn(2, '/').collect();
        let ready_color = if ready_parts.len() == 2
            && ready_parts[0] == ready_parts[1]
            && ready_parts[0] != "0"
        {
            t.success
        } else {
            t.warning
        };

        let info = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Name:      ", Style::default().fg(t.muted)),
                Span::styled(&d.name, Style::default().fg(t.fg)),
            ]),
            Line::from(vec![
                Span::styled("Ready:     ", Style::default().fg(t.muted)),
                Span::styled(&d.ready, Style::default().fg(ready_color)),
            ]),
            Line::from(vec![
                Span::styled("Replicas:  ", Style::default().fg(t.muted)),
                Span::styled(d.replicas.to_string(), Style::default().fg(t.fg)),
            ]),
            Line::from(vec![
                Span::styled("Available: ", Style::default().fg(t.muted)),
                Span::styled(d.available.to_string(), Style::default().fg(t.fg)),
            ]),
            Line::from(vec![
                Span::styled("Image:     ", Style::default().fg(t.muted)),
                Span::styled(&d.image, Style::default().fg(t.fg)),
            ]),
            Line::from(vec![
                Span::styled("Age:       ", Style::default().fg(t.muted)),
                Span::styled(&d.age, Style::default().fg(t.fg)),
            ]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Detail")
                .style(Style::default().bg(t.bg).fg(t.fg)),
        )
        .wrap(Wrap { trim: true });
        frame.render_widget(info, chunks[0]);
    } else {
        frame.render_widget(
            Paragraph::new("No deployment selected")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Detail")
                        .style(Style::default().bg(t.bg).fg(t.fg)),
                ),
            chunks[0],
        );
    }

    let key = |k: &'static str, c| Span::styled(k, Style::default().fg(c).add_modifier(Modifier::BOLD));
    let txt = |s: &'static str| Span::styled(s, Style::default().fg(t.fg));

    let actions = Paragraph::new(vec![
        Line::from(vec![key(" s ", t.accent), txt("Scale      "), key(" r ", t.warning), txt("Restart")]),
        Line::from(vec![key(" d ", t.accent), txt("Describe   "), key(" y ", t.accent), txt("YAML")]),
        Line::from(vec![key(" R ", t.muted), txt("Refresh")]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Actions")
            .style(Style::default().bg(t.bg).fg(t.fg)),
    );
    frame.render_widget(actions, chunks[1]);
}

// ── Events tab ────────────────────────────────────────────────────────────────

fn draw_events_tab(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();

    let header = Row::new(
        ["LAST SEEN", "TYPE", "REASON", "OBJECT", "COUNT", "MESSAGE"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(t.muted))),
    )
    .height(1)
    .style(Style::default().bg(t.bg));

    let rows: Vec<Row> = app
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let is_warning = e.type_ == "Warning";
            let type_color = if is_warning { t.warning } else { t.success };
            let selected = i == app.event_selected;

            Row::new(vec![
                Cell::from(e.last_seen.clone()),
                Cell::from(e.type_.clone()).style(Style::default().fg(type_color)),
                Cell::from(e.reason.clone()),
                Cell::from(e.object.clone()),
                Cell::from(e.count.to_string()),
                Cell::from(e.message.clone()),
            ])
            .style(if selected {
                Style::default().bg(t.surface).fg(t.accent).add_modifier(Modifier::BOLD)
            } else if is_warning {
                Style::default().fg(t.warning)
            } else {
                Style::default().fg(t.fg)
            })
        })
        .collect();

    let title = format!("Events ({}) — most recent first", app.events.len());
    let avail = area.width.saturating_sub(2);
    let lsw = avail * 10 / 100;
    let tw = avail * 9 / 100;
    let rw = avail * 14 / 100;
    let ow = avail * 20 / 100;
    let cw = 5u16;
    let mw = avail.saturating_sub(lsw + tw + rw + ow + cw);

    let table = Table::new(
        rows,
        [
            Constraint::Length(lsw),
            Constraint::Length(tw),
            Constraint::Length(rw),
            Constraint::Length(ow),
            Constraint::Length(cw),
            Constraint::Length(mw),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(t.surface)
            .fg(t.accent)
            .add_modifier(Modifier::BOLD),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(t.bg).fg(t.fg)),
    );

    let mut state = TableState::default();
    state.select(Some(app.event_selected));
    frame.render_stateful_widget(table, area, &mut state);
}

// ── Scale input prompt ────────────────────────────────────────────────────────

fn draw_scale_input(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();

    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup = centered_rect(popup_width, 6, area);
    frame.render_widget(Clear, popup);

    let content = vec![
        Line::from(vec![
            Span::styled("Deployment: ", Style::default().fg(t.muted)),
            Span::styled(&app.scale_deploy_name, Style::default().fg(t.accent)),
        ]),
        Line::from(vec![
            Span::styled("Current:    ", Style::default().fg(t.muted)),
            Span::styled(app.scale_current_replicas.to_string(), Style::default().fg(t.fg)),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("Replicas:   ", Style::default().fg(t.muted)),
            Span::styled(
                format!("{}█", app.scale_input_buffer),
                Style::default().fg(t.fg),
            ),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Scale Deployment (Enter to apply, Esc to cancel) ")
                .title_style(Style::default().fg(t.warning))
                .style(Style::default().bg(t.overlay).fg(t.fg)),
        ),
        popup,
    );
}

// ── Path input prompt ─────────────────────────────────────────────────────────

fn draw_path_input(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();

    let pod_name = app
        .selected_pod()
        .map(|p| p.name.as_str())
        .unwrap_or("?");

    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = 5u16;
    let popup = centered_rect(popup_width, popup_height, area);
    frame.render_widget(Clear, popup);

    let input_display = format!("{}█", app.path_input_buffer);

    let content = vec![
        Line::from(vec![
            Span::styled("Pod: ", Style::default().fg(t.muted)),
            Span::styled(pod_name, Style::default().fg(t.accent)),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("Path: ", Style::default().fg(t.muted)),
            Span::styled(&input_display, Style::default().fg(t.fg)),
        ]),
    ];

    let prompt = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Tail log file (Enter to start, Esc to cancel) ")
            .title_style(Style::default().fg(t.warning))
            .style(Style::default().bg(t.overlay).fg(t.fg)),
    );

    frame.render_widget(prompt, popup);
}

// ── Ingresses tab ─────────────────────────────────────────────────────────────

fn draw_ingresses_tab(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();
    let header = Row::new(
        ["NAME", "HOSTS", "PATHS", "AGE"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(t.muted))),
    )
    .height(1)
    .style(Style::default().bg(t.bg));

    let rows: Vec<Row> = app
        .ingresses
        .iter()
        .enumerate()
        .map(|(i, ing)| {
            let selected = i == app.ingress_selected;
            Row::new(vec![
                Cell::from(ing.name.clone()),
                Cell::from(ing.hosts.clone()).style(Style::default().fg(t.accent)),
                Cell::from(ing.paths.clone()).style(Style::default().fg(t.fg)),
                Cell::from(ing.age.clone()),
            ])
            .style(if selected {
                Style::default().bg(t.surface).fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            })
        })
        .collect();

    let title = format!("Ingresses ({})  — d: describe  y: YAML", app.ingresses.len());
    let avail = area.width.saturating_sub(2);
    let nw = avail * 25 / 100;
    let hw = avail * 25 / 100;
    let pw = avail * 42 / 100;
    let agew = avail.saturating_sub(nw + hw + pw);

    if app.ingresses.is_empty() {
        frame.render_widget(
            Paragraph::new("\n  No ingresses found. Press r to refresh.")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                        .style(Style::default().bg(t.bg).fg(t.fg)),
                )
                .style(Style::default().fg(t.muted)),
            area,
        );
        return;
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(nw),
            Constraint::Length(hw),
            Constraint::Length(pw),
            Constraint::Length(agew),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(t.surface)
            .fg(t.accent)
            .add_modifier(Modifier::BOLD),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(t.bg).fg(t.fg)),
    );

    let mut state = TableState::default();
    state.select(Some(app.ingress_selected));
    frame.render_stateful_widget(table, area, &mut state);
}

// ── Secrets tab ───────────────────────────────────────────────────────────────

fn draw_secrets_tab(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();
    let header = Row::new(
        ["NAME", "TYPE", "KEYS", "AGE"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(t.muted))),
    )
    .height(1)
    .style(Style::default().bg(t.bg));

    let rows: Vec<Row> = app
        .secrets
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let selected = i == app.secret_selected;
            Row::new(vec![
                Cell::from(s.name.clone()),
                Cell::from(s.type_.clone()).style(Style::default().fg(t.muted)),
                Cell::from(s.keys.to_string()),
                Cell::from(s.age.clone()),
            ])
            .style(if selected {
                Style::default().bg(t.surface).fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            })
        })
        .collect();

    let title = format!("Secrets ({})  — Enter/d: decoded  y: raw YAML  e: edit", app.secrets.len());
    let avail = area.width.saturating_sub(2);
    let nw = avail * 50 / 100;
    let tw = avail * 35 / 100;
    let kw = 6u16;
    let agew = avail.saturating_sub(nw + tw + kw);

    if app.secrets.is_empty() {
        frame.render_widget(
            Paragraph::new("\n  No secrets found. Press r to refresh.")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                        .style(Style::default().bg(t.bg).fg(t.fg)),
                )
                .style(Style::default().fg(t.muted)),
            area,
        );
        return;
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(nw),
            Constraint::Length(tw),
            Constraint::Length(kw),
            Constraint::Length(agew),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(t.surface)
            .fg(t.accent)
            .add_modifier(Modifier::BOLD),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(t.bg).fg(t.fg)),
    );

    let mut state = TableState::default();
    state.select(Some(app.secret_selected));
    frame.render_stateful_widget(table, area, &mut state);
}

// ── Confirmation dialog ───────────────────────────────────────────────────────

fn draw_confirm_dialog(frame: &mut Frame, app: &App, area: Rect) {
    use crate::app::ConfirmAction;

    let t = app.theme.colors();

    let popup_width = 56u16.min(area.width.saturating_sub(4));
    let popup = centered_rect(popup_width, 7, area);
    frame.render_widget(Clear, popup);

    let (title, message) = match &app.confirm_action {
        Some(ConfirmAction::RolloutRestart { name, .. }) => (
            " Confirm Rollout Restart ",
            format!("Restart all pods for deployment '{name}'?"),
        ),
        None => (" Confirm ", "Are you sure?".into()),
    };

    let yes_style = if app.confirm_yes {
        Style::default().fg(t.bg).bg(t.success).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.success)
    };
    let no_style = if !app.confirm_yes {
        Style::default().fg(t.bg).bg(t.danger).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.danger)
    };

    let content = vec![
        Line::from(Span::raw("")),
        Line::from(Span::styled(&message, Style::default().fg(t.fg))),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::raw("          "),
            Span::styled("  YES  ", yes_style),
            Span::raw("    "),
            Span::styled("  NO  ", no_style),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .title_style(Style::default().fg(t.warning))
                    .title_bottom(" ←/→ or Tab: switch  Enter: confirm  Esc/n: cancel ")
                    .style(Style::default().bg(t.overlay).fg(t.fg)),
            )
            .alignment(Alignment::Left),
        popup,
    );
}

// ── Secret key picker popup ───────────────────────────────────────────────────

fn draw_secret_key_picker(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();

    let popup_width = 54u16.min(area.width.saturating_sub(4));
    let popup_height = (app.secret_key_picker_list.len() as u16 + 4)
        .min(area.height.saturating_sub(4))
        .max(6);

    let popup = centered_rect(popup_width, popup_height, area);
    frame.render_widget(Clear, popup);

    let items: Vec<ListItem> = app
        .secret_key_picker_list
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let style = if i == app.secret_key_picker_idx {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD).bg(t.surface)
            } else {
                Style::default().fg(t.fg)
            };
            ListItem::new(format!("  {key}  ")).style(style)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.secret_key_picker_idx));

    let title = format!(" Edit key — {} ", app.secret_key_picker_secret);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_bottom(" j/k:nav  Enter:edit  Esc:cancel ")
            .style(Style::default().bg(t.overlay).fg(t.fg)),
    );

    frame.render_stateful_widget(list, popup, &mut list_state);
}

// ── Secret value editor ───────────────────────────────────────────────────────

fn draw_secret_value_input(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();

    let popup_width = 70u16.min(area.width.saturating_sub(4));
    let popup = centered_rect(popup_width, 7, area);
    frame.render_widget(Clear, popup);

    let input_display = format!("{}█", app.secret_value_input_buffer);

    let content = vec![
        Line::from(vec![
            Span::styled("Secret: ", Style::default().fg(t.muted)),
            Span::styled(&app.secret_key_picker_secret, Style::default().fg(t.accent)),
        ]),
        Line::from(vec![
            Span::styled("Key:    ", Style::default().fg(t.muted)),
            Span::styled(&app.secret_value_input_key, Style::default().fg(t.secondary)),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("Value:  ", Style::default().fg(t.muted)),
            Span::styled(&input_display, Style::default().fg(t.fg)),
        ]),
        Line::from(vec![
            Span::styled(
                "  (stored as base64; empty value clears the key)",
                Style::default().fg(t.muted),
            ),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Edit Secret Value (Enter to save, Esc to cancel) ")
                .title_style(Style::default().fg(t.warning))
                .style(Style::default().bg(t.overlay).fg(t.fg)),
        ),
        popup,
    );
}

// ── Container picker popup ────────────────────────────────────────────────────

fn draw_container_picker(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();

    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = (app.container_picker_list.len() as u16 + 4).min(area.height.saturating_sub(4)).max(6);

    let popup = centered_rect(popup_width, popup_height, area);
    frame.render_widget(Clear, popup);

    let items: Vec<ListItem> = app
        .container_picker_list
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == app.container_picker_idx {
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD).bg(t.surface)
            } else {
                Style::default().fg(t.fg)
            };
            ListItem::new(format!("  {name}  ")).style(style)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.container_picker_idx));

    let title = format!(" Containers — {} ", app.container_picker_pod);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_bottom(" j/k:nav  Enter:exec  Esc:cancel ")
            .style(Style::default().bg(t.overlay).fg(t.fg)),
    );

    frame.render_stateful_widget(list, popup, &mut list_state);
}

// ── Port-forward input prompt ─────────────────────────────────────────────────

fn draw_port_input(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme.colors();

    let pod_name = app.selected_pod().map(|p| p.name.as_str()).unwrap_or("?");

    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup = centered_rect(popup_width, 7, area);
    frame.render_widget(Clear, popup);

    let input_display = format!("{}█", app.port_input_buffer);

    let active_count = app.port_forwards.iter().filter(|pf| pf.active).count();

    let content = vec![
        Line::from(vec![
            Span::styled("Pod: ", Style::default().fg(t.muted)),
            Span::styled(pod_name, Style::default().fg(t.accent)),
        ]),
        Line::from(vec![
            Span::styled("Active forwards: ", Style::default().fg(t.muted)),
            Span::styled(active_count.to_string(), Style::default().fg(t.secondary)),
            Span::styled("  (P to view)", Style::default().fg(t.muted)),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("local:remote  ", Style::default().fg(t.muted)),
            Span::styled(&input_display, Style::default().fg(t.fg)),
        ]),
        Line::from(vec![
            Span::styled("  e.g. 8080:8080 or 8080", Style::default().fg(t.muted)),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Port Forward (Enter to start, Esc to cancel) ")
                .title_style(Style::default().fg(t.warning))
                .style(Style::default().bg(t.overlay).fg(t.fg)),
        ),
        popup,
    );
}

// ── Log level colorizer ───────────────────────────────────────────────────────

fn colorize_log_line<'a>(line: &'a str, t: &Theme) -> Line<'a> {
    let upper = line.to_uppercase();
    let color = if upper.contains("ERROR") || upper.contains("FATAL") || upper.contains("CRITICAL") {
        Some(t.danger)
    } else if upper.contains("WARN") {
        Some(t.warning)
    } else if upper.contains("DEBUG") || upper.contains("TRACE") {
        Some(t.muted)
    } else {
        None
    };

    match color {
        Some(c) => Line::from(Span::styled(line, Style::default().fg(c))),
        None => Line::from(Span::styled(line, Style::default().fg(t.fg))),
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

/// Returns a centered Rect of the given width/height inside `area`.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
