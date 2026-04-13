mod theme;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Tabs, Wrap,
};

use crate::app::{
    AppState, FooterAction, JobColumn, Modal, ModalAction, MouseHit, OverviewColumn, Page, RowKind,
    SortDirection, UiHitMap, UserColumn,
};
use crate::model::{JobRecord, MetricMode, PartitionOverview, UserUsage, format_mem_mb};
use crate::ui::theme::Theme;

pub use theme::Theme as ThemePalette;

const RESOURCE_SEGMENT_WIDTH: usize = 32;
const NAME_SEGMENT_WIDTH: usize = 36;
const PARTITION_SEGMENT_WIDTH: usize = 40;

pub fn render(frame: &mut Frame, app: &AppState) -> UiHitMap {
    let theme = Theme::from_choice(app.settings.theme, app.settings.no_color);
    let mut hit_map = UiHitMap::default();
    let footer_height = if app.settings.compact { 3 } else { 4 };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(footer_height),
        ])
        .split(frame.area());

    render_header(frame, layout[0], app, &theme);
    render_tabs(frame, layout[1], app, &theme, &mut hit_map);
    render_body(frame, layout[2], app, &theme, &mut hit_map);
    render_footer(frame, layout[3], app, &theme, &mut hit_map);

    if let Some(modal) = &app.modal {
        render_modal(frame, modal, app, &theme, &mut hit_map);
    }

    hit_map
}

fn render_header(frame: &mut Frame, area: Rect, app: &AppState, theme: &Theme) {
    let (status, status_style) = app.status_badge(theme);
    let lines = vec![
        Line::from(vec![
            Span::styled("sqtop", theme.title.add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            badge_span(status.to_ascii_uppercase(), status_style),
            Span::raw(format!(
                "  Host: {}  User: {}  Time: {}  Refresh: {:.1}s",
                app.snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.hostname.as_str())
                    .unwrap_or("loading"),
                app.settings.user,
                app.now_string(),
                app.settings.interval
            )),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "Sample: {} ms  Metric: {}  Sort: {}  Filter: {}",
                app.snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.sample_duration_ms.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                app.metric_mode.label().to_ascii_uppercase(),
                app.sort_label(),
                app.filter_label()
            )),
            Span::raw("  "),
            toggle_badge("Mine-only", app.show_only_mine, theme),
            Span::raw("  "),
            toggle_badge("Search", !app.active_global_query().is_empty(), theme),
            Span::raw("  "),
            toggle_badge("Pinned", app.pinned_partition.is_some(), theme),
            Span::raw("  "),
            toggle_badge("Stale", app.is_stale(), theme),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "Visible query: {}",
                if app.active_global_query().is_empty() {
                    "All items".to_string()
                } else {
                    app.active_global_query().to_string()
                }
            )),
            Span::raw("  "),
            Span::styled(
                app.status_message()
                    .map(|message| message.to_string())
                    .unwrap_or_else(|| {
                        app.snapshot
                            .as_ref()
                            .map(|snapshot| {
                                if snapshot.notes.is_empty() {
                                    "No active warnings".to_string()
                                } else {
                                    format!("Warnings: {}", snapshot.notes.join(" | "))
                                }
                            })
                            .unwrap_or_else(|| "Collecting scheduler data".to_string())
                    }),
                theme.muted,
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_tabs(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let pages = app.page_tabs();
    let titles: Vec<Line> = pages
        .iter()
        .map(|page| Line::from(Span::raw(page.label())))
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::BOTTOM))
        .highlight_style(theme.accent.add_modifier(Modifier::BOLD))
        .select(app.current_main_page_index());
    frame.render_widget(tabs, area);

    let segments = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Ratio(1, pages.len() as u32); pages.len()])
        .split(area);
    for (page, rect) in pages.into_iter().zip(segments.iter().copied()) {
        hit_map.push(rect, MouseHit::Tab(page));
    }
}

fn render_body(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    if app.snapshot.is_none() {
        frame.render_widget(
            Paragraph::new("Collecting lightweight Slurm data for the first screen...")
                .block(Block::default().borders(Borders::ALL).title("Loading"))
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let body_chunks = if app.input_mode == crate::app::InputMode::Normal {
        vec![area]
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(8)])
            .split(area)
            .to_vec()
    };

    let content_area = if body_chunks.len() == 2 {
        render_input_bar(frame, body_chunks[0], app, theme);
        body_chunks[1]
    } else {
        body_chunks[0]
    };

    match app.current_page() {
        Page::Overview => render_overview(frame, content_area, app, theme, hit_map),
        Page::MyJobs => render_jobs(frame, content_area, app, theme, hit_map, true),
        Page::Users => render_users(frame, content_area, app, theme, hit_map),
        Page::AllJobs => render_jobs(frame, content_area, app, theme, hit_map, false),
        Page::PartitionDetail => render_partition_detail(frame, content_area, app, theme, hit_map),
        Page::NodeDetail => render_node_detail(frame, content_area, app, theme, hit_map),
    }
}

fn render_overview(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(7)])
        .split(area);
    render_overview_summary(frame, sections[0], app, theme);

    let rows = app.visible_partitions();
    let running_capacity = rows
        .iter()
        .map(|partition| partition.total_usage().running_total(app.metric_mode))
        .max()
        .unwrap_or(1);
    let pending_capacity = rows
        .iter()
        .map(|partition| partition.total_usage().pending_total(app.metric_mode))
        .max()
        .unwrap_or(1);
    let table_rows: Vec<Row> = rows
        .iter()
        .map(|partition| {
            let usage = partition.used_for_pressure(app.metric_mode);
            let pressure = pressure_bar_cell(partition, app.metric_mode, theme, 18);
            let mine_running = metric_value_line(
                partition.mine.running_total(app.metric_mode),
                running_capacity,
                8,
                theme.mine,
                theme,
            );
            let mine_pending = metric_value_line(
                partition.mine.pending_total(app.metric_mode),
                pending_capacity,
                8,
                theme.mine,
                theme,
            );
            let others_running = metric_value_line(
                partition.others.running_total(app.metric_mode),
                running_capacity,
                8,
                theme.other,
                theme,
            );
            let others_pending = metric_value_line(
                partition.others.pending_total(app.metric_mode),
                pending_capacity,
                8,
                theme.other,
                theme,
            );
            let resources = format_partition_resources(partition, app.metric_mode, usage);
            Row::new(vec![
                Cell::from(Line::from(vec![Span::styled(
                    partition.name.clone(),
                    theme
                        .partition_style(&partition.name)
                        .add_modifier(Modifier::BOLD),
                )])),
                Cell::from(partition.state.clone()),
                Cell::from(partition.total_nodes.to_string()),
                Cell::from(pressure),
                Cell::from(mine_running),
                Cell::from(mine_pending),
                Cell::from(others_running),
                Cell::from(others_pending),
                Cell::from(format!(
                    "Running: {}  Pending: {}",
                    partition.total_usage().running_total(MetricMode::Jobs),
                    partition.total_usage().pending_total(MetricMode::Jobs),
                )),
                Cell::from(resources),
            ])
            .style(partition_state_style(partition, theme))
        })
        .collect();

    let constraints = [
        Constraint::Length(12),
        Constraint::Length(8),
        Constraint::Length(7),
        Constraint::Length(23),
        Constraint::Length(18),
        Constraint::Length(18),
        Constraint::Length(18),
        Constraint::Length(18),
        Constraint::Length(21),
        Constraint::Min(22),
    ];
    let headers = vec![
        sortable_header(
            "Partition",
            app.overview_sort.column == OverviewColumn::Partition,
            app.overview_sort.direction,
            theme,
        ),
        Line::from("State"),
        sortable_header(
            "Nodes",
            app.overview_sort.column == OverviewColumn::Nodes,
            app.overview_sort.direction,
            theme,
        ),
        sortable_header(
            "Pressure",
            app.overview_sort.column == OverviewColumn::Pressure,
            app.overview_sort.direction,
            theme,
        ),
        sortable_header(
            "Mine Running",
            app.overview_sort.column == OverviewColumn::MineRunning,
            app.overview_sort.direction,
            theme,
        ),
        sortable_header(
            "Mine Pending",
            app.overview_sort.column == OverviewColumn::MinePending,
            app.overview_sort.direction,
            theme,
        ),
        sortable_header(
            "Others Running",
            app.overview_sort.column == OverviewColumn::OthersRunning,
            app.overview_sort.direction,
            theme,
        ),
        sortable_header(
            "Others Pending",
            app.overview_sort.column == OverviewColumn::OthersPending,
            app.overview_sort.direction,
            theme,
        ),
        sortable_header(
            "Total Jobs",
            app.overview_sort.column == OverviewColumn::TotalJobs,
            app.overview_sort.direction,
            theme,
        ),
        Line::from("Resources"),
    ];

    let table = Table::new(table_rows, constraints.clone())
        .header(Row::new(headers).style(theme.title.add_modifier(Modifier::BOLD)))
        .block(
            Block::default()
                .title("Partition Overview")
                .borders(Borders::ALL),
        )
        .row_highlight_style(theme.highlight)
        .highlight_symbol(">> ");

    let mut state = TableState::default();
    state.select(if rows.is_empty() {
        None
    } else {
        Some(app.selected_overview.min(rows.len().saturating_sub(1)))
    });
    frame.render_stateful_widget(table, sections[1], &mut state);
    register_table_rows(hit_map, sections[1], rows.len(), RowKind::Overview);
    register_header_hits(
        hit_map,
        sections[1],
        &constraints,
        &[
            Some(MouseHit::OverviewHeader(OverviewColumn::Partition)),
            None,
            Some(MouseHit::OverviewHeader(OverviewColumn::Nodes)),
            Some(MouseHit::OverviewHeader(OverviewColumn::Pressure)),
            Some(MouseHit::OverviewHeader(OverviewColumn::MineRunning)),
            Some(MouseHit::OverviewHeader(OverviewColumn::MinePending)),
            Some(MouseHit::OverviewHeader(OverviewColumn::OthersRunning)),
            Some(MouseHit::OverviewHeader(OverviewColumn::OthersPending)),
            Some(MouseHit::OverviewHeader(OverviewColumn::TotalJobs)),
            None,
        ],
    );
}

fn render_overview_summary(frame: &mut Frame, area: Rect, app: &AppState, theme: &Theme) {
    let parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(2, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
        ])
        .split(area);

    let top_partitions: Vec<&PartitionOverview> =
        app.visible_partitions().into_iter().take(3).collect();
    let summary_lines = if top_partitions.is_empty() {
        vec![Line::from("No partitions match the current filters")]
    } else {
        top_partitions
            .iter()
            .map(|partition| {
                let total = partition.total_usage();
                Line::from(vec![
                    Span::styled(
                        format!("{:<10}", partition.name),
                        theme
                            .partition_style(&partition.name)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  Pressure "),
                    Span::raw(pressure_bar_text(
                        partition.used_for_pressure(app.metric_mode),
                        partition.capacity_for(app.metric_mode),
                        10,
                    )),
                    Span::raw(format!(
                        "  Running: Mine {} / Others {}  Pending: Mine {} / Others {}",
                        partition.mine.running_jobs,
                        partition.others.running_jobs,
                        partition.mine.pending_jobs,
                        partition.others.pending_jobs
                    )),
                    Span::raw(format!(
                        "  Total active jobs: {}",
                        total.active_total(MetricMode::Jobs)
                    )),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(summary_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Hot Partitions"),
            )
            .wrap(Wrap { trim: true }),
        parts[0],
    );

    let counts = app
        .snapshot
        .as_ref()
        .map(|snapshot| {
            let mine = snapshot
                .jobs
                .iter()
                .filter(|job| job.is_mine && job.active)
                .count();
            let active = snapshot.jobs.iter().filter(|job| job.active).count();
            (mine, active)
        })
        .unwrap_or((0, 0));
    let side_lines = vec![
        Line::from(format!("Visible active jobs: {}", counts.1)),
        Line::from(format!("Visible active jobs owned by you: {}", counts.0)),
        Line::from(vec![toggle_badge(
            "Advanced resource columns",
            app.settings.show_advanced_resources,
            theme,
        )]),
    ];
    frame.render_widget(
        Paragraph::new(side_lines)
            .block(Block::default().borders(Borders::ALL).title("Status"))
            .wrap(Wrap { trim: true }),
        parts[1],
    );

    render_trend_panel(frame, parts[2], "Cluster trend", app.cluster_trend(), theme);
}

fn render_jobs(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
    mine_only: bool,
) {
    let rows = if mine_only {
        app.visible_my_jobs()
    } else {
        app.visible_all_jobs()
    };
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Min(8)])
        .split(area);
    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(30), Constraint::Length(18)])
        .split(sections[0]);
    let trend_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(7)])
        .split(header_layout[0]);
    let header_lines = vec![
        Line::from(vec![
            Span::styled(
                if mine_only { "My Jobs" } else { "All Jobs" },
                theme.title.add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  Visible jobs: {}  State filter: {}",
                rows.len(),
                app.filter_label()
            )),
            Span::raw("  "),
            toggle_badge("Mine-only view", app.show_only_mine, theme),
        ]),
        Line::from(format!(
            "Metric: {}  Sort: {}  Current user: {}  Horizontal view: column {} (Left / Right)",
            app.metric_mode.label().to_ascii_uppercase(),
            app.sort_label(),
            app.settings.user,
            app.job_horizontal_offset + 1
        )),
    ];
    frame.render_widget(
        Paragraph::new(header_lines)
            .block(Block::default().borders(Borders::ALL).title("Queue"))
            .wrap(Wrap { trim: true }),
        trend_area[0],
    );
    render_trend_panel(
        frame,
        trend_area[1],
        if mine_only {
            "My job trend"
        } else {
            "Cluster job trend"
        },
        if mine_only {
            app.my_trend()
        } else {
            app.cluster_trend()
        },
        theme,
    );
    let back_label = "← Overview (b)";
    frame.render_widget(
        Paragraph::new(back_label)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(theme.accent)
                    .title("Back"),
            ),
        header_layout[1],
    );
    hit_map.push(
        header_layout[1],
        MouseHit::Footer(FooterAction::BackOverview),
    );

    let show_user = !mine_only;
    let resource_scale = JobResourceScale::from_jobs(&rows);
    let resource_texts = rows
        .iter()
        .map(|job| resource_footprint_text(job, &resource_scale))
        .collect::<Vec<_>>();
    let resource_segments = max_segments(&resource_texts, RESOURCE_SEGMENT_WIDTH);
    let name_segments = max_segments(
        &rows.iter().map(|job| job.name.clone()).collect::<Vec<_>>(),
        NAME_SEGMENT_WIDTH,
    );
    let where_width = rows
        .iter()
        .map(|job| job.location_or_reason.chars().count())
        .max()
        .unwrap_or(24)
        .clamp(24, 72) as u16;
    let mut headers = vec![
        sortable_header(
            "Job ID",
            app.job_sort.column == JobColumn::JobId,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "Partition",
            app.job_sort.column == JobColumn::Partition,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "State",
            app.job_sort.column == JobColumn::State,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "Runtime",
            app.job_sort.column == JobColumn::Runtime,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "Time limit",
            app.job_sort.column == JobColumn::TimeLimit,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "Nodes",
            app.job_sort.column == JobColumn::Nodes,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "CPUs",
            app.job_sort.column == JobColumn::Cpus,
            app.job_sort.direction,
            theme,
        ),
    ];
    let mut column_widths = vec![8, 11, 11, 9, 10, 4, 6];
    let mut header_hits = vec![
        Some(MouseHit::JobHeader(JobColumn::JobId)),
        Some(MouseHit::JobHeader(JobColumn::Partition)),
        Some(MouseHit::JobHeader(JobColumn::State)),
        Some(MouseHit::JobHeader(JobColumn::Runtime)),
        Some(MouseHit::JobHeader(JobColumn::TimeLimit)),
        Some(MouseHit::JobHeader(JobColumn::Nodes)),
        Some(MouseHit::JobHeader(JobColumn::Cpus)),
    ];
    if show_user {
        headers.insert(
            1,
            sortable_header(
                "User",
                app.job_sort.column == JobColumn::User,
                app.job_sort.direction,
                theme,
            ),
        );
        column_widths.insert(1, 10);
        header_hits.insert(1, Some(MouseHit::JobHeader(JobColumn::User)));
    }
    for segment in 0..resource_segments {
        headers.push(sortable_header(
            if resource_segments > 1 && segment == 0 {
                "Resource footprint"
            } else if resource_segments > 1 {
                "Resource footprint →"
            } else {
                "Resource footprint"
            },
            false,
            app.job_sort.direction,
            theme,
        ));
        column_widths.push(RESOURCE_SEGMENT_WIDTH as u16);
        header_hits.push(None);
    }
    headers.push(sortable_header(
        "Placement or reason",
        app.job_sort.column == JobColumn::WhereWhy,
        app.job_sort.direction,
        theme,
    ));
    column_widths.push(where_width);
    header_hits.push(Some(MouseHit::JobHeader(JobColumn::WhereWhy)));
    for segment in 0..name_segments {
        headers.push(sortable_header(
            if name_segments > 1 && segment == 0 {
                "Name"
            } else if name_segments > 1 {
                "Name →"
            } else {
                "Name"
            },
            app.job_sort.column == JobColumn::Name,
            app.job_sort.direction,
            theme,
        ));
        column_widths.push(NAME_SEGMENT_WIDTH as u16);
        header_hits.push(Some(MouseHit::JobHeader(JobColumn::Name)));
    }

    let full_rows: Vec<Vec<Cell>> = rows
        .iter()
        .zip(resource_texts.iter())
        .map(|(job, resource_text)| {
            let mut cells = vec![Cell::from(job.job_id.clone())];
            if show_user {
                cells.push(Cell::from(Line::from(vec![Span::styled(
                    job.user.clone(),
                    if job.is_mine { theme.mine } else { theme.other },
                )])));
            }
            cells.extend([
                Cell::from(Line::from(vec![Span::styled(
                    job.partition_raw.clone(),
                    theme.partition_style(job.primary_partition()),
                )])),
                Cell::from(job.state.clone()),
                Cell::from(job.runtime_raw.clone()),
                Cell::from(job.time_limit_raw.clone()),
                Cell::from(job.nodes.to_string()),
                Cell::from(
                    job.cpus
                        .map(|value: u32| value.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
            ]);
            for segment in 0..resource_segments {
                cells.push(Cell::from(segment_text(
                    resource_text,
                    segment,
                    RESOURCE_SEGMENT_WIDTH,
                )));
            }
            cells.push(Cell::from(job.location_or_reason.clone()));
            for segment in 0..name_segments {
                cells.push(Cell::from(segment_text(
                    &job.name,
                    segment,
                    NAME_SEGMENT_WIDTH,
                )));
            }
            cells
        })
        .collect();
    let (visible_indices, hidden_left, hidden_right) = visible_column_indices(
        &column_widths,
        sections[1].width.saturating_sub(4),
        app.job_horizontal_offset,
    );
    let constraints = visible_indices
        .iter()
        .map(|index| Constraint::Length(column_widths[*index]))
        .collect::<Vec<_>>();
    let visible_headers = visible_indices
        .iter()
        .map(|index| headers[*index].clone())
        .collect::<Vec<_>>();
    let visible_hits = visible_indices
        .iter()
        .map(|index| header_hits[*index].clone())
        .collect::<Vec<_>>();
    let table_rows: Vec<Row> = rows
        .iter()
        .zip(full_rows.iter())
        .map(|(job, cells)| {
            let visible_cells = visible_indices
                .iter()
                .map(|index| cells[*index].clone())
                .collect::<Vec<_>>();
            Row::new(visible_cells).style(job_state_style(job, theme))
        })
        .collect();

    let title = format!(
        "{}  Hidden left: {}  Hidden right: {}",
        if mine_only { "My Jobs" } else { "All Jobs" },
        hidden_left,
        hidden_right
    );
    let table = Table::new(table_rows, constraints.clone())
        .header(Row::new(visible_headers).style(theme.title.add_modifier(Modifier::BOLD)))
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(theme.highlight)
        .highlight_symbol(">> ");
    let mut state = TableState::default();
    let selected = if mine_only {
        app.selected_my_jobs
    } else {
        app.selected_all_jobs
    };
    state.select(if rows.is_empty() {
        None
    } else {
        Some(selected.min(rows.len().saturating_sub(1)))
    });
    frame.render_stateful_widget(table, sections[1], &mut state);
    register_table_rows(
        hit_map,
        sections[1],
        rows.len(),
        if mine_only {
            RowKind::MyJobs
        } else {
            RowKind::AllJobs
        },
    );
    register_header_hits(hit_map, sections[1], &constraints, &visible_hits);
}

fn render_users(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let users = app.visible_users();
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(9),
            Constraint::Min(8),
        ])
        .split(area);

    let header_lines = vec![
        Line::from(vec![
            Span::styled("User View", theme.title.add_modifier(Modifier::BOLD)),
            Span::raw(format!(
                "  Visible users: {}  Filter: {}",
                users.len(),
                app.filter_label()
            )),
            Span::raw("  "),
            toggle_badge("Mine-only view", app.show_only_mine, theme),
        ]),
        Line::from(format!(
            "Sort: {}  Search query: {}  Horizontal view: column {} (Left / Right)",
            app.sort_label(),
            if app.active_global_query().is_empty() {
                "All users".to_string()
            } else {
                app.active_global_query().to_string()
            },
            app.job_horizontal_offset + 1
        )),
    ];
    frame.render_widget(
        Paragraph::new(header_lines)
            .block(Block::default().borders(Borders::ALL).title("Users"))
            .wrap(Wrap { trim: true }),
        sections[0],
    );

    let user_scale = UserResourceScale::from_users(&users);
    let user_footprints = users
        .iter()
        .map(|usage| user_resource_footprint_text(usage, &user_scale))
        .collect::<Vec<_>>();
    let footprint_segments = max_segments(&user_footprints, RESOURCE_SEGMENT_WIDTH);
    let partition_texts = users
        .iter()
        .map(|usage| usage.top_partitions_summary(3))
        .collect::<Vec<_>>();
    let partition_segments = max_segments(&partition_texts, PARTITION_SEGMENT_WIDTH);
    let mut headers = vec![
        sortable_header(
            "User",
            app.user_sort.column == UserColumn::User,
            app.user_sort.direction,
            theme,
        ),
        sortable_header(
            "Running jobs",
            app.user_sort.column == UserColumn::RunningJobs,
            app.user_sort.direction,
            theme,
        ),
        sortable_header(
            "Pending jobs",
            app.user_sort.column == UserColumn::PendingJobs,
            app.user_sort.direction,
            theme,
        ),
        sortable_header(
            "Total jobs",
            app.user_sort.column == UserColumn::TotalJobs,
            app.user_sort.direction,
            theme,
        ),
    ];
    let mut column_widths = vec![12, 13, 13, 11];
    let mut header_hits = vec![
        Some(MouseHit::UserHeader(UserColumn::User)),
        Some(MouseHit::UserHeader(UserColumn::RunningJobs)),
        Some(MouseHit::UserHeader(UserColumn::PendingJobs)),
        Some(MouseHit::UserHeader(UserColumn::TotalJobs)),
    ];
    for segment in 0..footprint_segments {
        headers.push(sortable_header(
            if footprint_segments > 1 && segment > 0 {
                "Resource footprint →"
            } else {
                "Resource footprint"
            },
            app.user_sort.column == UserColumn::ResourceFootprint,
            app.user_sort.direction,
            theme,
        ));
        column_widths.push(RESOURCE_SEGMENT_WIDTH as u16);
        header_hits.push(Some(MouseHit::UserHeader(UserColumn::ResourceFootprint)));
    }
    for segment in 0..partition_segments {
        headers.push(sortable_header(
            if partition_segments > 1 && segment > 0 {
                "Top partitions →"
            } else {
                "Top partitions"
            },
            app.user_sort.column == UserColumn::Partitions,
            app.user_sort.direction,
            theme,
        ));
        column_widths.push(PARTITION_SEGMENT_WIDTH as u16);
        header_hits.push(Some(MouseHit::UserHeader(UserColumn::Partitions)));
    }
    let full_rows: Vec<Vec<Cell>> = users
        .iter()
        .zip(user_footprints.iter().zip(partition_texts.iter()))
        .map(|(usage, (footprint, partition_text))| {
            let mut cells = vec![
                Cell::from(Line::from(vec![Span::styled(
                    usage.user.clone(),
                    if usage.is_current_user {
                        theme.mine.add_modifier(Modifier::BOLD)
                    } else {
                        theme.other
                    },
                )])),
                Cell::from(usage.jobs.running_jobs.to_string()),
                Cell::from(usage.jobs.pending_jobs.to_string()),
                Cell::from(usage.total_jobs().to_string()),
            ];
            for segment in 0..footprint_segments {
                cells.push(Cell::from(segment_text(
                    footprint,
                    segment,
                    RESOURCE_SEGMENT_WIDTH,
                )));
            }
            for segment in 0..partition_segments {
                cells.push(Cell::from(segment_text(
                    partition_text,
                    segment,
                    PARTITION_SEGMENT_WIDTH,
                )));
            }
            cells
        })
        .collect();
    let (visible_indices, hidden_left, hidden_right) = visible_column_indices(
        &column_widths,
        sections[1].width.saturating_sub(4),
        app.job_horizontal_offset,
    );
    let constraints = visible_indices
        .iter()
        .map(|index| Constraint::Length(column_widths[*index]))
        .collect::<Vec<_>>();
    let visible_headers = visible_indices
        .iter()
        .map(|index| headers[*index].clone())
        .collect::<Vec<_>>();
    let visible_hits = visible_indices
        .iter()
        .map(|index| header_hits[*index].clone())
        .collect::<Vec<_>>();
    let user_rows: Vec<Row> = users
        .iter()
        .zip(full_rows.iter())
        .map(|(_, cells)| {
            Row::new(
                visible_indices
                    .iter()
                    .map(|index| cells[*index].clone())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let mut state = TableState::default();
    state.select(if users.is_empty() {
        None
    } else {
        Some(app.selected_users.min(users.len().saturating_sub(1)))
    });
    frame.render_stateful_widget(
        Table::new(user_rows, constraints.clone())
            .header(Row::new(visible_headers).style(theme.title.add_modifier(Modifier::BOLD)))
            .block(Block::default().borders(Borders::ALL).title(format!(
                "Active users  Hidden left: {}  Hidden right: {}",
                hidden_left, hidden_right
            )))
            .row_highlight_style(theme.highlight)
            .highlight_symbol(">> "),
        sections[1],
        &mut state,
    );
    register_table_rows(hit_map, sections[1], users.len(), RowKind::Users);
    register_header_hits(hit_map, sections[1], &constraints, &visible_hits);

    let selected_usage = app.selected_user_usage();
    let selected_jobs = app.visible_selected_user_jobs();
    let resource_scale = JobResourceScale::from_jobs(&selected_jobs);
    let summary_lines = if let Some(usage) = &selected_usage {
        vec![
            Line::from(vec![
                Span::styled(
                    format!("Selected user: {}", usage.user),
                    if usage.is_current_user {
                        theme.mine.add_modifier(Modifier::BOLD)
                    } else {
                        theme.title.add_modifier(Modifier::BOLD)
                    },
                ),
                Span::raw(format!(
                    "  Running: {}  Pending: {}  CPUs: {}  GPUs: {}",
                    usage.jobs.running_jobs,
                    usage.jobs.pending_jobs,
                    usage.total_cpus(),
                    usage.total_gpus(),
                )),
            ]),
            Line::from(format!(
                "Main partitions: {}",
                usage.top_partitions_summary(4)
            )),
        ]
    } else {
        vec![Line::from("No active user matches the current filters")]
    };
    let selected_resource_texts = selected_jobs
        .iter()
        .map(|job| resource_footprint_text(job, &resource_scale))
        .collect::<Vec<_>>();
    let resource_segments = max_segments(&selected_resource_texts, RESOURCE_SEGMENT_WIDTH);
    let name_segments = max_segments(
        &selected_jobs
            .iter()
            .map(|job| job.name.clone())
            .collect::<Vec<_>>(),
        NAME_SEGMENT_WIDTH,
    );
    let lower = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(sections[2]);
    frame.render_widget(
        Paragraph::new(summary_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Selected user"),
            )
            .wrap(Wrap { trim: true }),
        lower[0],
    );
    let mut headers = vec![
        Line::from("Job ID"),
        Line::from("Partition"),
        Line::from("State"),
        Line::from("Runtime"),
    ];
    let mut column_widths = vec![8, 12, 11, 9];
    for segment in 0..resource_segments {
        headers.push(Line::from(if resource_segments > 1 && segment > 0 {
            "Resource footprint →"
        } else {
            "Resource footprint"
        }));
        column_widths.push(RESOURCE_SEGMENT_WIDTH as u16);
    }
    for segment in 0..name_segments {
        headers.push(Line::from(if name_segments > 1 && segment > 0 {
            "Name →"
        } else {
            "Name"
        }));
        column_widths.push(NAME_SEGMENT_WIDTH as u16);
    }
    let full_rows: Vec<Vec<Cell>> = selected_jobs
        .iter()
        .zip(selected_resource_texts.iter())
        .map(|(job, resource_text)| {
            let mut cells = vec![
                Cell::from(job.job_id.clone()),
                Cell::from(Line::from(vec![Span::styled(
                    job.partition_raw.clone(),
                    theme.partition_style(job.primary_partition()),
                )])),
                Cell::from(job.state.clone()),
                Cell::from(job.runtime_raw.clone()),
            ];
            for segment in 0..resource_segments {
                cells.push(Cell::from(segment_text(
                    resource_text,
                    segment,
                    RESOURCE_SEGMENT_WIDTH,
                )));
            }
            for segment in 0..name_segments {
                cells.push(Cell::from(segment_text(
                    &job.name,
                    segment,
                    NAME_SEGMENT_WIDTH,
                )));
            }
            cells
        })
        .collect();
    let (visible_indices, hidden_left, hidden_right) = visible_column_indices(
        &column_widths,
        lower[1].width.saturating_sub(4),
        app.job_horizontal_offset,
    );
    let constraints = visible_indices
        .iter()
        .map(|index| Constraint::Length(column_widths[*index]))
        .collect::<Vec<_>>();
    let visible_headers = visible_indices
        .iter()
        .map(|index| headers[*index].clone())
        .collect::<Vec<_>>();
    let job_rows: Vec<Row> = selected_jobs
        .iter()
        .zip(full_rows.iter())
        .take(lower[1].height.saturating_sub(3) as usize)
        .map(|(job, cells)| {
            Row::new(
                visible_indices
                    .iter()
                    .map(|index| cells[*index].clone())
                    .collect::<Vec<_>>(),
            )
            .style(job_state_style(job, theme))
        })
        .collect();
    frame.render_widget(
        Table::new(job_rows, constraints)
            .header(Row::new(visible_headers).style(theme.title.add_modifier(Modifier::BOLD)))
            .block(Block::default().borders(Borders::ALL).title(format!(
                "Jobs for selected user  Hidden left: {}  Hidden right: {}",
                hidden_left, hidden_right
            ))),
        lower[1],
    );
}

fn render_partition_detail(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let Some(partition) = app.selected_partition_detail() else {
        frame.render_widget(
            Paragraph::new("No partition selected").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Partition Detail"),
            ),
            area,
        );
        return;
    };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(16), Constraint::Min(8)])
        .split(area);
    render_partition_detail_summary(frame, layout[0], app, partition, theme, hit_map);

    let jobs = app.visible_partition_jobs(&partition.name);
    let resource_scale = JobResourceScale::from_jobs(&jobs);
    let resource_texts = jobs
        .iter()
        .map(|job| resource_footprint_text(job, &resource_scale))
        .collect::<Vec<_>>();
    let resource_segments = max_segments(&resource_texts, RESOURCE_SEGMENT_WIDTH);
    let name_segments = max_segments(
        &jobs.iter().map(|job| job.name.clone()).collect::<Vec<_>>(),
        NAME_SEGMENT_WIDTH,
    );
    let where_width = jobs
        .iter()
        .map(|job| job.location_or_reason.chars().count())
        .max()
        .unwrap_or(24)
        .clamp(24, 72) as u16;
    let mut headers = vec![
        sortable_header(
            "Job ID",
            app.job_sort.column == JobColumn::JobId,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "User",
            app.job_sort.column == JobColumn::User,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "State",
            app.job_sort.column == JobColumn::State,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "Runtime",
            app.job_sort.column == JobColumn::Runtime,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "Where / Why",
            app.job_sort.column == JobColumn::WhereWhy,
            app.job_sort.direction,
            theme,
        ),
    ];
    let mut column_widths = vec![8, 10, 10, 9, where_width];
    let mut header_hits = vec![
        Some(MouseHit::JobHeader(JobColumn::JobId)),
        Some(MouseHit::JobHeader(JobColumn::User)),
        Some(MouseHit::JobHeader(JobColumn::State)),
        Some(MouseHit::JobHeader(JobColumn::Runtime)),
        Some(MouseHit::JobHeader(JobColumn::WhereWhy)),
    ];
    for segment in 0..resource_segments {
        headers.push(sortable_header(
            if resource_segments > 1 && segment > 0 {
                "Resource footprint →"
            } else {
                "Resource footprint"
            },
            false,
            app.job_sort.direction,
            theme,
        ));
        column_widths.push(RESOURCE_SEGMENT_WIDTH as u16);
        header_hits.push(None);
    }
    for segment in 0..name_segments {
        headers.push(sortable_header(
            if name_segments > 1 && segment > 0 {
                "Name →"
            } else {
                "Name"
            },
            app.job_sort.column == JobColumn::Name,
            app.job_sort.direction,
            theme,
        ));
        column_widths.push(NAME_SEGMENT_WIDTH as u16);
        header_hits.push(Some(MouseHit::JobHeader(JobColumn::Name)));
    }
    let full_rows: Vec<Vec<Cell>> = jobs
        .iter()
        .zip(resource_texts.iter())
        .map(|(job, resource_text)| {
            let mut cells = vec![
                Cell::from(job.job_id.clone()),
                Cell::from(Line::from(vec![Span::styled(
                    job.user.clone(),
                    if job.is_mine { theme.mine } else { theme.other },
                )])),
                Cell::from(job.state.clone()),
                Cell::from(job.runtime_raw.clone()),
                Cell::from(job.location_or_reason.clone()),
            ];
            for segment in 0..resource_segments {
                cells.push(Cell::from(segment_text(
                    resource_text,
                    segment,
                    RESOURCE_SEGMENT_WIDTH,
                )));
            }
            for segment in 0..name_segments {
                cells.push(Cell::from(segment_text(
                    &job.name,
                    segment,
                    NAME_SEGMENT_WIDTH,
                )));
            }
            cells
        })
        .collect();
    let (visible_indices, hidden_left, hidden_right) = visible_column_indices(
        &column_widths,
        layout[1].width.saturating_sub(4),
        app.job_horizontal_offset,
    );
    let constraints = visible_indices
        .iter()
        .map(|index| Constraint::Length(column_widths[*index]))
        .collect::<Vec<_>>();
    let visible_headers = visible_indices
        .iter()
        .map(|index| headers[*index].clone())
        .collect::<Vec<_>>();
    let visible_hits = visible_indices
        .iter()
        .map(|index| header_hits[*index].clone())
        .collect::<Vec<_>>();
    let table_rows: Vec<Row> = jobs
        .iter()
        .zip(full_rows.iter())
        .map(|(job, cells)| {
            Row::new(
                visible_indices
                    .iter()
                    .map(|index| cells[*index].clone())
                    .collect::<Vec<_>>(),
            )
            .style(job_state_style(job, theme))
        })
        .collect();

    let table = Table::new(table_rows, constraints.clone())
        .header(Row::new(visible_headers).style(theme.title.add_modifier(Modifier::BOLD)))
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Jobs  Hidden left: {}  Hidden right: {}",
            hidden_left, hidden_right
        )))
        .row_highlight_style(theme.highlight)
        .highlight_symbol(">> ");
    let mut state = TableState::default();
    state.select(if jobs.is_empty() {
        None
    } else {
        Some(
            app.selected_partition_jobs
                .min(jobs.len().saturating_sub(1)),
        )
    });
    frame.render_stateful_widget(table, layout[1], &mut state);
    register_table_rows(hit_map, layout[1], jobs.len(), RowKind::PartitionJobs);
    register_header_hits(hit_map, layout[1], &constraints, &visible_hits);
}

fn render_partition_detail_summary(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    partition: &PartitionOverview,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(2, 5),
            Constraint::Ratio(2, 5),
            Constraint::Ratio(1, 5),
        ])
        .split(area);
    let trend_and_nodes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(6)])
        .split(chunks[1]);

    let total = partition.total_usage();
    let left_lines = vec![
        Line::from(vec![
            Span::styled(
                format!("{} ", partition.name),
                theme
                    .partition_style(&partition.name)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "State: {}  Total nodes: {}",
                partition.state, partition.total_nodes
            )),
        ]),
        Line::from(vec![
            Span::raw("Pressure: "),
            Span::raw(pressure_bar_text(
                partition.used_for_pressure(app.metric_mode),
                partition.capacity_for(app.metric_mode),
                26,
            )),
        ]),
        Line::from(vec![
            Span::raw("Running ownership: "),
            Span::raw(stacked_bar_text(
                partition.mine.running_total(app.metric_mode),
                partition.others.running_total(app.metric_mode),
                partition.capacity_for(app.metric_mode),
                26,
            )),
        ]),
        Line::from(vec![
            Span::raw("Pending ownership: "),
            Span::raw(stacked_bar_text(
                partition.mine.pending_total(app.metric_mode),
                partition.others.pending_total(app.metric_mode),
                Some(
                    partition.mine.pending_total(app.metric_mode)
                        + partition.others.pending_total(app.metric_mode)
                        + 1,
                ),
                26,
            )),
        ]),
        Line::from(vec![
            Span::raw("Running versus pending jobs: "),
            Span::raw(stacked_bar_text_with_labels(
                u64::from(total.running_jobs),
                u64::from(total.pending_jobs),
                Some(u64::from(total.running_jobs + total.pending_jobs).max(1)),
                26,
                "Running",
                "Pending",
            )),
        ]),
        Line::from(format!(
            "CPU in use: {} / {}  GPU in use: {} / {}  Memory capacity: {}",
            total.running_cpus,
            partition
                .total_cpus
                .map(|value| value.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            total.running_gpus,
            partition
                .total_gpus
                .map(|value| value.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            partition
                .total_mem_mb
                .map(format_mem_mb)
                .unwrap_or_else(|| "N/A".to_string())
        )),
    ];
    frame.render_widget(
        Paragraph::new(left_lines)
            .block(Block::default().borders(Borders::ALL).title("Usage"))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    render_trend_panel(
        frame,
        trend_and_nodes[0],
        &format!("{} trend", partition.name),
        app.partition_trend(&partition.name)
            .unwrap_or(app.cluster_trend()),
        theme,
    );

    let right_lines = build_node_state_lines(partition, theme);
    frame.render_widget(
        Paragraph::new(right_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Node State Distribution"),
            )
            .wrap(Wrap { trim: true }),
        chunks[2],
    );

    let nodes = app.visible_partition_nodes(&partition.name);
    let node_rows: Vec<Row> = nodes
        .iter()
        .map(|node| {
            Row::new(vec![
                Cell::from(node.node_name.clone()),
                Cell::from(node.state.clone()),
                Cell::from(format!(
                    "CPUs {}  GPUs {}",
                    node.cpus
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                    node.gpus
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "N/A".to_string())
                )),
            ])
        })
        .collect();
    let mut node_state = TableState::default();
    node_state.select(if nodes.is_empty() {
        None
    } else {
        Some(
            app.selected_partition_node
                .min(nodes.len().saturating_sub(1)),
        )
    });
    frame.render_stateful_widget(
        Table::new(
            node_rows,
            [
                Constraint::Length(12),
                Constraint::Length(9),
                Constraint::Min(16),
            ],
        )
        .header(
            Row::new(["Node", "State", "Capacity"]).style(theme.title.add_modifier(Modifier::BOLD)),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Nodes in this Partition"),
        )
        .row_highlight_style(theme.highlight)
        .highlight_symbol(">> "),
        trend_and_nodes[1],
        &mut node_state,
    );
    register_table_rows(
        hit_map,
        trend_and_nodes[1],
        nodes.len(),
        RowKind::PartitionNodes,
    );
}

fn render_node_detail(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let Some(node) = app.selected_node_detail() else {
        frame.render_widget(
            Paragraph::new("No node selected")
                .block(Block::default().borders(Borders::ALL).title("Node Detail")),
            area,
        );
        return;
    };

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(4),
            Constraint::Min(8),
        ])
        .split(area);

    let summary_lines = if node.loading {
        vec![Line::from(format!(
            "Loading node data for {}...",
            node.node_name
        ))]
    } else {
        let detail = node.detail.as_ref();
        vec![
            Line::from(vec![
                Span::styled(
                    format!("{} ", node.node_name),
                    theme.title.add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(
                    "Partition: {}  State: {}",
                    detail
                        .and_then(|detail| detail.partition.as_deref())
                        .unwrap_or(node.partition.as_str()),
                    detail
                        .and_then(|detail| detail.state.as_deref())
                        .unwrap_or("N/A")
                )),
            ]),
            Line::from(format!(
                "CPU allocation: {} / {}  GPU allocation: {} / {}  Memory allocation: {} / {}",
                detail
                    .and_then(|detail| detail.cpu_alloc)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
                detail
                    .and_then(|detail| detail.cpu_total)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
                detail
                    .and_then(|detail| detail.gpu_alloc)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
                detail
                    .and_then(|detail| detail.gpu_total)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
                detail
                    .and_then(|detail| detail.mem_alloc_mb)
                    .map(format_mem_mb)
                    .unwrap_or_else(|| "N/A".to_string()),
                detail
                    .and_then(|detail| detail.mem_total_mb)
                    .map(format_mem_mb)
                    .unwrap_or_else(|| "N/A".to_string()),
            )),
            Line::from(format!(
                "Node reason: {}",
                detail
                    .and_then(|detail| detail.reason.as_deref())
                    .unwrap_or("No node-level reason reported")
            )),
            Line::from(format!(
                "Loaded at: {}  Node jobs visible after filters: {}",
                node.last_loaded_at.as_deref().unwrap_or("pending"),
                app.visible_node_jobs().len()
            )),
        ]
    };
    frame.render_widget(
        Paragraph::new(summary_lines)
            .block(Block::default().borders(Borders::ALL).title("Node Summary"))
            .wrap(Wrap { trim: true }),
        sections[0],
    );

    let filter_lines = vec![
        Line::from(format!(
            "User filter: {}  State filter: {}",
            app.current_node_user_filter(node),
            app.current_node_state_filter(node)
        )),
        Line::from(format!(
            "Where filter: {}  Why filter: {}",
            if app.active_node_where_filter().is_empty() {
                "All placements".to_string()
            } else {
                app.active_node_where_filter().to_string()
            },
            if app.active_node_why_filter().is_empty() {
                "All reasons".to_string()
            } else {
                app.active_node_why_filter().to_string()
            }
        )),
    ];
    frame.render_widget(
        Paragraph::new(filter_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Interactive Filters"),
            )
            .wrap(Wrap { trim: true }),
        sections[1],
    );

    let jobs = app.visible_node_jobs();
    let resource_scale = JobResourceScale::from_jobs(&jobs);
    let resource_texts = jobs
        .iter()
        .map(|job| resource_footprint_text(job, &resource_scale))
        .collect::<Vec<_>>();
    let resource_segments = max_segments(&resource_texts, RESOURCE_SEGMENT_WIDTH);
    let name_segments = max_segments(
        &jobs.iter().map(|job| job.name.clone()).collect::<Vec<_>>(),
        NAME_SEGMENT_WIDTH,
    );
    let partition_width = jobs
        .iter()
        .map(|job| job.partition_raw.chars().count())
        .max()
        .unwrap_or(11)
        .clamp(11, 32) as u16;
    let where_width = jobs
        .iter()
        .map(|job| job.location_or_reason.chars().count())
        .max()
        .unwrap_or(18)
        .clamp(18, 72) as u16;
    let why_width = jobs
        .iter()
        .map(|job| {
            if job.pending {
                job.location_or_reason.chars().count()
            } else {
                job.state.chars().count()
            }
        })
        .max()
        .unwrap_or(14)
        .clamp(14, 48) as u16;
    let mut headers = vec![
        sortable_header(
            "Job ID",
            app.job_sort.column == JobColumn::JobId,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "User",
            app.job_sort.column == JobColumn::User,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "State",
            app.job_sort.column == JobColumn::State,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "Runtime",
            app.job_sort.column == JobColumn::Runtime,
            app.job_sort.direction,
            theme,
        ),
        sortable_header(
            "Partition",
            app.job_sort.column == JobColumn::Partition,
            app.job_sort.direction,
            theme,
        ),
        sortable_header("Where", false, app.job_sort.direction, theme),
        sortable_header("Why", false, app.job_sort.direction, theme),
    ];
    let mut column_widths = vec![8, 10, 11, 10, partition_width, where_width, why_width];
    let mut header_hits = vec![
        Some(MouseHit::JobHeader(JobColumn::JobId)),
        Some(MouseHit::JobHeader(JobColumn::User)),
        Some(MouseHit::JobHeader(JobColumn::State)),
        Some(MouseHit::JobHeader(JobColumn::Runtime)),
        Some(MouseHit::JobHeader(JobColumn::Partition)),
        None,
        None,
    ];
    for segment in 0..resource_segments {
        headers.push(sortable_header(
            if resource_segments > 1 && segment > 0 {
                "Resource footprint →"
            } else {
                "Resource footprint"
            },
            false,
            app.job_sort.direction,
            theme,
        ));
        column_widths.push(RESOURCE_SEGMENT_WIDTH as u16);
        header_hits.push(None);
    }
    for segment in 0..name_segments {
        headers.push(sortable_header(
            if name_segments > 1 && segment > 0 {
                "Name →"
            } else {
                "Name"
            },
            app.job_sort.column == JobColumn::Name,
            app.job_sort.direction,
            theme,
        ));
        column_widths.push(NAME_SEGMENT_WIDTH as u16);
        header_hits.push(Some(MouseHit::JobHeader(JobColumn::Name)));
    }
    let full_rows: Vec<Vec<Cell>> = jobs
        .iter()
        .zip(resource_texts.iter())
        .map(|(job, resource_text)| {
            let why_text = if job.pending {
                job.location_or_reason.clone()
            } else {
                job.state.clone()
            };
            let mut cells = vec![
                Cell::from(job.job_id.clone()),
                Cell::from(Line::from(vec![Span::styled(
                    job.user.clone(),
                    if job.is_mine { theme.mine } else { theme.other },
                )])),
                Cell::from(job.state.clone()),
                Cell::from(job.runtime_raw.clone()),
                Cell::from(Line::from(vec![Span::styled(
                    job.partition_raw.clone(),
                    theme.partition_style(job.primary_partition()),
                )])),
                Cell::from(job.location_or_reason.clone()),
                Cell::from(why_text),
            ];
            for segment in 0..resource_segments {
                cells.push(Cell::from(segment_text(
                    resource_text,
                    segment,
                    RESOURCE_SEGMENT_WIDTH,
                )));
            }
            for segment in 0..name_segments {
                cells.push(Cell::from(segment_text(
                    &job.name,
                    segment,
                    NAME_SEGMENT_WIDTH,
                )));
            }
            cells
        })
        .collect();
    let (visible_indices, hidden_left, hidden_right) = visible_column_indices(
        &column_widths,
        sections[2].width.saturating_sub(4),
        app.job_horizontal_offset,
    );
    let constraints = visible_indices
        .iter()
        .map(|index| Constraint::Length(column_widths[*index]))
        .collect::<Vec<_>>();
    let visible_headers = visible_indices
        .iter()
        .map(|index| headers[*index].clone())
        .collect::<Vec<_>>();
    let visible_hits = visible_indices
        .iter()
        .map(|index| header_hits[*index].clone())
        .collect::<Vec<_>>();
    let table_rows: Vec<Row> = jobs
        .iter()
        .zip(full_rows.iter())
        .map(|(job, cells)| {
            Row::new(
                visible_indices
                    .iter()
                    .map(|index| cells[*index].clone())
                    .collect::<Vec<_>>(),
            )
            .style(job_state_style(job, theme))
        })
        .collect();

    let mut table_state = TableState::default();
    table_state.select(if jobs.is_empty() {
        None
    } else {
        Some(node.selected_job.min(jobs.len().saturating_sub(1)))
    });
    frame.render_stateful_widget(
        Table::new(table_rows, constraints.clone())
            .header(Row::new(visible_headers).style(theme.title.add_modifier(Modifier::BOLD)))
            .block(Block::default().borders(Borders::ALL).title(format!(
                "Jobs on this Node  Hidden left: {}  Hidden right: {}",
                hidden_left, hidden_right
            )))
            .row_highlight_style(theme.highlight)
            .highlight_symbol(">> "),
        sections[2],
        &mut table_state,
    );
    register_table_rows(hit_map, sections[2], jobs.len(), RowKind::NodeJobs);
    register_header_hits(hit_map, sections[2], &constraints, &visible_hits);
}

#[allow(dead_code)]
fn render_history(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(8)])
        .split(area);
    let intro = vec![
        Line::from(vec![
            Span::styled("History", theme.title.add_modifier(Modifier::BOLD)),
            Span::raw(format!(
                "  window {}  visible {}  mine-only {}",
                app.history_window_label(),
                app.visible_history().len(),
                if app.show_only_mine { "on" } else { "off" }
            )),
        ]),
        Line::from(format!(
            "sort {}  state {}  loaded {}",
            app.sort_label(),
            app.filter_label(),
            app.history
                .last_loaded_at
                .clone()
                .unwrap_or_else(|| "pending".to_string())
        )),
    ];
    frame.render_widget(
        Paragraph::new(intro)
            .block(Block::default().borders(Borders::ALL).title("History View"))
            .wrap(Wrap { trim: true }),
        sections[0],
    );

    if !app.history.available && !app.history.loading {
        frame.render_widget(
            Paragraph::new(
                "sacct data unavailable in this environment. History page stays visible and degrades gracefully.",
            )
            .block(Block::default().borders(Borders::ALL).title("History Unavailable"))
            .wrap(Wrap { trim: true }),
            sections[1],
        );
        return;
    }

    let rows = app.visible_history();
    let table_rows: Vec<Row> = rows
        .iter()
        .map(|history| {
            let mut cells = vec![
                Cell::from(history.job_id.clone()),
                Cell::from(Line::from(vec![Span::styled(
                    history.user.clone(),
                    if history.is_mine {
                        theme.mine
                    } else {
                        theme.other
                    },
                )])),
                Cell::from(history.partition.clone().unwrap_or_else(|| "-".to_string())),
                Cell::from(history.state.clone()),
                Cell::from(history.exit_code.clone()),
                Cell::from(history.elapsed_raw.clone()),
                Cell::from(history.end.clone().unwrap_or_else(|| "-".to_string())),
            ];
            if app.settings.show_advanced_resources {
                cells.push(Cell::from(history.resources_summary()));
            }
            cells.push(Cell::from(history.name.clone()));
            Row::new(cells).style(if history.is_mine {
                theme.mine
            } else {
                theme.other
            })
        })
        .collect();

    let mut constraints = vec![
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(11),
        Constraint::Length(12),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(19),
    ];
    let mut headers = vec![
        "JobID",
        "User",
        "Partition",
        "State",
        "Exit",
        "Elapsed",
        "End",
    ]
    .into_iter()
    .map(ToOwned::to_owned)
    .collect::<Vec<_>>();
    if app.settings.show_advanced_resources {
        constraints.push(Constraint::Length(18));
        headers.push("AllocTRES".to_string());
    }
    constraints.push(Constraint::Min(20));
    headers.push("Name".to_string());

    let table = Table::new(table_rows, constraints)
        .header(Row::new(headers).style(theme.title.add_modifier(Modifier::BOLD)))
        .block(Block::default().borders(Borders::ALL).title("History Jobs"))
        .row_highlight_style(theme.highlight)
        .highlight_symbol(">> ");
    let mut state = TableState::default();
    state.select(if rows.is_empty() {
        None
    } else {
        Some(app.selected_history.min(rows.len().saturating_sub(1)))
    });
    frame.render_stateful_widget(table, sections[1], &mut state);
    register_table_rows(hit_map, sections[1], rows.len(), RowKind::History);
}

fn render_footer(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if app.settings.compact {
            vec![Constraint::Length(1), Constraint::Length(2)]
        } else {
            vec![
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(2),
            ]
        })
        .split(area);

    render_footer_actions(frame, sections[0], app, theme, hit_map);

    let nav = format!(
        "Navigation: Tab switch pages  j/k move selection  Enter open detail  i show job detail  Left / Right move horizontal view  Mouse click or double-click  Wheel scroll"
    );
    frame.render_widget(
        Paragraph::new(nav)
            .style(theme.muted)
            .block(Block::default()),
        sections[1],
    );

    let page_hint = match app.current_page() {
        Page::Overview => {
            "Overview: g change metric  p pin partition  Enter open partition detail  Click column headers to sort"
        }
        Page::MyJobs | Page::AllJobs => {
            "Queue: f change state filter  s change sort  m toggle mine-only  x cancel selected job  X review visible jobs for cancel  Click column headers to sort  Back: b or ← Overview"
        }
        Page::PartitionDetail => {
            "Partition: s change job sort  [ or ] choose node  n open selected node  g metric  m toggle mine-only  x cancel selected job  b back"
        }
        Page::Users => {
            "Users: s change sort  m toggle mine-only  Click column headers to sort  Search filters users and the selected user's jobs"
        }
        Page::NodeDetail => {
            "Node: u user filter  f state filter  w where filter  y why filter  c clear filters  x cancel selected job  b back"
        }
    };
    let tail = format!(
        "{}  Search: /  Help: h  Mouse: tabs, rows, footer buttons, and modal buttons",
        page_hint
    );
    let last_index = if app.settings.compact { 1 } else { 2 };
    frame.render_widget(
        Paragraph::new(tail)
            .style(theme.muted)
            .block(Block::default().borders(Borders::TOP)),
        sections[last_index],
    );
}

fn render_footer_actions(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let actions = app.footer_actions();
    let mut spans = Vec::new();
    let mut x = area.x;
    for action in actions {
        let label = footer_action_label(action);
        let token = format!("[{label}] ");
        spans.push(Span::styled(
            token.clone(),
            theme.accent.add_modifier(Modifier::BOLD),
        ));
        let rect = Rect::new(x, area.y, token.len() as u16, 1);
        hit_map.push(rect, MouseHit::Footer(action));
        x = x.saturating_add(token.len() as u16);
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_modal(
    frame: &mut Frame,
    modal: &Modal,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    hit_map.push(frame.area(), MouseHit::Modal(ModalAction::Close));
    match modal {
        Modal::Help => render_help_modal(frame, app, theme, hit_map),
        Modal::JobDetail(detail) => render_job_detail_modal(frame, detail, theme, hit_map),
        Modal::ConfirmCancel(preview) => {
            render_cancel_confirm_modal(frame, preview, theme, hit_map)
        }
        Modal::CancelResult(report) => render_cancel_result_modal(frame, report, theme, hit_map),
    }
}

fn render_input_bar(frame: &mut Frame, area: Rect, app: &AppState, theme: &Theme) {
    let (title, value, hint) = match app.input_mode {
        crate::app::InputMode::Search => (
            "Search",
            app.active_global_query(),
            "Type to filter immediately. Enter keeps the query. Esc cancels.",
        ),
        crate::app::InputMode::NodeWhereFilter => (
            "Node Filter: Where",
            app.active_node_where_filter(),
            "Filter by partition, placement, or node-related text. Enter keeps it. Esc cancels.",
        ),
        crate::app::InputMode::NodeWhyFilter => (
            "Node Filter: Why",
            app.active_node_why_filter(),
            "Filter by reason text. Enter keeps it. Esc cancels.",
        ),
        crate::app::InputMode::Normal => return,
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(title, theme.title.add_modifier(Modifier::BOLD)),
            Span::raw(": "),
            Span::styled(
                if value.is_empty() {
                    " ".repeat(1)
                } else {
                    value.to_string()
                },
                theme.accent.add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("▏", theme.accent),
        ]),
        Line::from(Span::styled(hint, theme.muted)),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Input"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn badge_span(label: String, style: Style) -> Span<'static> {
    Span::styled(format!("[{label}]"), style.add_modifier(Modifier::BOLD))
}

fn toggle_badge(label: &str, enabled: bool, theme: &Theme) -> Span<'static> {
    let state = if enabled { "ON" } else { "OFF" };
    let style = if enabled { theme.success } else { theme.danger };
    Span::styled(
        format!("{label}: [{state}]"),
        style.add_modifier(Modifier::BOLD),
    )
}

fn render_help_modal(frame: &mut Frame, _app: &AppState, theme: &Theme, hit_map: &mut UiHitMap) {
    let area = centered_rect(78, 78, frame.area());
    frame.render_widget(Clear, area);
    let text = vec![
        Line::from(Span::styled(
            "Navigation",
            theme.title.add_modifier(Modifier::BOLD),
        )),
        Line::from(
            "Tab / Shift-Tab switch top-level pages. j/k or arrows move the current list. Enter opens the focused detail. b or Esc goes back.",
        ),
        Line::from(
            "Mouse: click tabs, click rows to select, double-click rows to open, wheel scrolls lists, footer buttons are clickable.",
        ),
        Line::from(
            "Click sortable column headers in Overview, Users, My Jobs, and All Jobs to sort immediately. Click the same header again to reverse the direction.",
        ),
        Line::from(""),
        Line::from(Span::styled(
            "Views",
            theme.title.add_modifier(Modifier::BOLD),
        )),
        Line::from(
            "Overview shows partition pressure plus separate Mine / Others counts for running and pending work.",
        ),
        Line::from(
            "Overview, My Jobs, All Jobs, and Partition Detail keep rolling trend panels for running and pending job counts.",
        ),
        Line::from(
            "User View ranks active users by jobs and resources, then shows the selected user's active jobs and main partitions.",
        ),
        Line::from(
            "Partition Detail shows pressure, running-versus-pending jobs, node-state distribution, and a node list for that partition.",
        ),
        Line::from(
            "Node Detail is loaded on demand with node-local jobs and interactive filters for user, state, where, and why.",
        ),
        Line::from(""),
        Line::from(Span::styled(
            "Actions",
            theme.title.add_modifier(Modifier::BOLD),
        )),
        Line::from(
            "/ search immediately filters the visible list as you type. In queue views, f changes the main state filter. s changes sort. g changes metric. m toggles mine-only view.",
        ),
        Line::from(
            "In Partition Detail, [ and ] change the selected node and n opens that node. In Node Detail, u changes the user filter, w edits the where filter, y edits the why filter, and c clears node filters.",
        ),
        Line::from(
            "i shows job detail. x cancels the selected job. X reviews visible jobs for bulk cancel.",
        ),
        Line::from(
            "Wide queue tables can be moved horizontally with Left / Right. In the job-detail modal, clicking outside the panel closes it.",
        ),
        Line::from(
            "All cancel flows are previewed first and only allow your own active jobs by default.",
        ),
        Line::from(""),
        Line::from(Span::styled(
            "Data Sources",
            theme.title.add_modifier(Modifier::BOLD),
        )),
        Line::from("Overview and queue pages: sinfo + squeue text formats."),
        Line::from("Partition totals: scontrol show partition when available."),
        Line::from("Node Detail: scontrol show node -o plus squeue -w <node>."),
        Line::from("Job Detail: scontrol show job -o plus sacct job lookup."),
        Line::from(""),
        Line::from("Press Enter / Esc / h to close"),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .wrap(Wrap { trim: true }),
        area,
    );
    hit_map.push(area, MouseHit::Modal(ModalAction::Ignore));
}

fn render_job_detail_modal(
    frame: &mut Frame,
    detail: &crate::app::JobDetailModal,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let area = centered_rect(82, 78, frame.area());
    frame.render_widget(Clear, area);
    hit_map.push(area, MouseHit::Modal(ModalAction::Ignore));
    if detail.loading {
        frame.render_widget(
            Paragraph::new(format!("Loading detail for job {}...", detail.job_id))
                .block(Block::default().borders(Borders::ALL).title("Job Detail"))
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    if let Some(detail) = &detail.detail {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(6),
                Constraint::Min(7),
            ])
            .split(area);
        let top = Line::from(vec![
            Span::styled(
                format!("Job {}", detail.job_id),
                theme.title.add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                detail.state.as_deref().unwrap_or("N/A").to_string(),
                theme.accent.add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Click outside, Esc, b, or q to close"),
        ]);
        frame.render_widget(
            Paragraph::new(top).block(Block::default().borders(Borders::ALL).title("Job Detail")),
            layout[0],
        );

        let top_blocks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(layout[1]);
        render_kv_table(
            frame,
            top_blocks[0],
            "Basic",
            &vec![
                ("Job ID".to_string(), detail.job_id.clone()),
                (
                    "Name".to_string(),
                    detail.name.clone().unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "User".to_string(),
                    detail.user.clone().unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "Partition".to_string(),
                    detail
                        .partition
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "State".to_string(),
                    detail.state.clone().unwrap_or_else(|| "N/A".to_string()),
                ),
                ("Priority".to_string(), "N/A".to_string()),
            ],
            theme,
        );
        render_kv_table(
            frame,
            top_blocks[1],
            "Resources",
            &vec![
                (
                    "Nodes".to_string(),
                    detail
                        .nodes
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "Tasks".to_string(),
                    detail
                        .n_tasks
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "CPUs".to_string(),
                    detail
                        .cpus
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "Memory".to_string(),
                    detail
                        .memory_mb
                        .map(format_mem_mb)
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "GPUs".to_string(),
                    detail
                        .requested_gpus
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "GRES".to_string(),
                    detail.gres.clone().unwrap_or_else(|| "N/A".to_string()),
                ),
            ],
            theme,
        );

        let mid_blocks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(layout[2]);
        render_kv_table(
            frame,
            mid_blocks[0],
            "Scheduling",
            &vec![
                (
                    "Runtime".to_string(),
                    detail
                        .runtime_raw
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "Time limit".to_string(),
                    detail
                        .time_limit_raw
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "Submit time".to_string(),
                    detail
                        .submit_time
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "Start time".to_string(),
                    detail
                        .start_time
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "End time".to_string(),
                    detail.end_time.clone().unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "Exit code".to_string(),
                    detail
                        .exit_code
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
            ],
            theme,
        );
        render_kv_table(
            frame,
            mid_blocks[1],
            "Placement / Reason",
            &vec![
                (
                    "Node list".to_string(),
                    detail
                        .node_list
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "Reason".to_string(),
                    detail.reason.clone().unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "ReqTRES".to_string(),
                    detail.req_tres.clone().unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "AllocTRES".to_string(),
                    detail
                        .alloc_tres
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
            ],
            theme,
        );

        let lower_blocks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(layout[3]);
        render_wrapped_field(
            frame,
            lower_blocks[0],
            "Paths",
            &[
                format!("Workdir: {}", detail.work_dir.as_deref().unwrap_or("N/A")),
                format!("Stdout: {}", detail.stdout_path.as_deref().unwrap_or("N/A")),
                format!("Stderr: {}", detail.stderr_path.as_deref().unwrap_or("N/A")),
            ],
            theme,
        );
        render_wrapped_field(
            frame,
            lower_blocks[1],
            "Extra",
            &[
                format!("Command: {}", detail.command.as_deref().unwrap_or("N/A")),
                format!(
                    "Notes: {}",
                    if detail.source_notes.is_empty() {
                        "none".to_string()
                    } else {
                        detail.source_notes.join(" | ")
                    }
                ),
            ],
            theme,
        );

        frame.render_widget(
            Paragraph::new(
                "Mouse: click outside the panel to close. Keyboard: Esc, b, q, or Enter.",
            )
            .block(Block::default().borders(Borders::ALL).title("Close")),
            layout[4],
        );
    } else {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from("Job detail unavailable"),
                Line::from(
                    detail
                        .error
                        .clone()
                        .unwrap_or_else(|| "Unknown error".to_string()),
                ),
            ])
            .block(Block::default().borders(Borders::ALL).title("Job Detail")),
            area,
        );
    }
}

fn render_cancel_confirm_modal(
    frame: &mut Frame,
    preview: &crate::app::CancelPreview,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let area = centered_rect(78, 72, frame.area());
    frame.render_widget(Clear, area);
    hit_map.push(area, MouseHit::Modal(ModalAction::Ignore));
    let top = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(area);
    let intro = vec![
        Line::from(Span::styled(
            "Dangerous action",
            theme.danger.add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "{}  You are about to cancel {} job(s). Only your active jobs will be affected.",
            preview.title,
            preview.allowed_count()
        )),
        Line::from("Review the full list below before confirming."),
    ];
    frame.render_widget(
        Paragraph::new(intro)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Confirm Cancel"),
            )
            .wrap(Wrap { trim: true }),
        top[0],
    );

    let rows: Vec<Row> = preview
        .candidates
        .iter()
        .take(top[1].height.saturating_sub(3) as usize)
        .map(|candidate| {
            Row::new(vec![
                Cell::from(candidate.job_id.clone()),
                Cell::from(candidate.user.clone()),
                Cell::from(candidate.partition.clone()),
                Cell::from(candidate.state.clone()),
                Cell::from(if candidate.allowed {
                    "yes".to_string()
                } else {
                    candidate.reason.clone().unwrap_or_else(|| "no".to_string())
                }),
                Cell::from(candidate.name.clone()),
            ])
            .style(if candidate.allowed {
                theme.warning
            } else {
                theme.muted
            })
        })
        .collect();
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(11),
                Constraint::Length(10),
                Constraint::Length(28),
                Constraint::Min(18),
            ],
        )
        .header(
            Row::new(["JobID", "User", "Partition", "State", "Will cancel", "Name"])
                .style(theme.title.add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::ALL).title("Preview")),
        top[1],
    );

    let buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20),
            Constraint::Length(20),
            Constraint::Min(1),
        ])
        .split(top[2]);
    let confirm_rect = buttons[0];
    let cancel_rect = buttons[1];
    frame.render_widget(
        Paragraph::new("Confirm [Enter / y]")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(theme.danger)
                    .title("Confirm"),
            ),
        confirm_rect,
    );
    frame.render_widget(
        Paragraph::new("Back [Esc / n]")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Cancel")),
        cancel_rect,
    );
    hit_map.push(confirm_rect, MouseHit::Modal(ModalAction::Confirm));
    hit_map.push(cancel_rect, MouseHit::Modal(ModalAction::Cancel));
}

fn render_cancel_result_modal(
    frame: &mut Frame,
    report: &crate::app::CancelReport,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let area = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, area);
    hit_map.push(area, MouseHit::Modal(ModalAction::Ignore));
    let mut lines = vec![
        Line::from(Span::styled(
            format!(
                "{} cancel results: {} succeeded, {} failed",
                match report.scope {
                    crate::app::CancelScope::Single => "Single",
                    crate::app::CancelScope::Visible => "Bulk",
                },
                report.succeeded(),
                report.failed()
            ),
            theme.title.add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for outcome in report.results.iter().take(16) {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<8}", outcome.job_id),
                if outcome.success {
                    theme.success
                } else {
                    theme.danger
                },
            ),
            Span::raw(" "),
            Span::raw(&outcome.message),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Press Enter / Esc to close"));
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Cancel Result"),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn register_table_rows(hit_map: &mut UiHitMap, area: Rect, row_count: usize, kind: RowKind) {
    let body_y = area.y.saturating_add(2);
    let width = area.width.saturating_sub(2);
    for index in 0..row_count {
        let row = body_y.saturating_add(index as u16);
        if row >= area.y.saturating_add(area.height.saturating_sub(1)) {
            break;
        }
        hit_map.push(
            Rect::new(area.x.saturating_add(1), row, width, 1),
            MouseHit::Row(kind, index),
        );
    }
}

fn register_header_hits(
    hit_map: &mut UiHitMap,
    area: Rect,
    constraints: &[Constraint],
    hits: &[Option<MouseHit>],
) {
    // ratatui tables do not expose header hit-testing, so we mirror the header layout with the
    // same column constraints and register per-column mouse targets ourselves.
    if constraints.is_empty() || hits.is_empty() {
        return;
    }
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        1,
    );
    let rects = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints.iter().copied())
        .split(inner);
    for (rect, hit) in rects.into_iter().zip(hits.iter()) {
        if let Some(hit) = hit {
            hit_map.push(*rect, hit.clone());
        }
    }
}

fn render_kv_table(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    rows: &[(String, String)],
    theme: &Theme,
) {
    let table_rows = rows.iter().map(|(label, value)| {
        Row::new(vec![
            Cell::from(Line::from(vec![Span::styled(
                label.clone(),
                theme.title.add_modifier(Modifier::BOLD),
            )])),
            Cell::from(value.clone()),
        ])
    });
    frame.render_widget(
        Table::new(table_rows, [Constraint::Length(14), Constraint::Min(12)])
            .block(Block::default().borders(Borders::ALL).title(title)),
        area,
    );
}

fn render_wrapped_field(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    lines: &[String],
    theme: &Theme,
) {
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .style(theme.muted)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_trend_panel(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    trend: &crate::app::TrendSeries,
    theme: &Theme,
) {
    let block = Block::default().borders(Borders::ALL).title(format!(
        "{}  Running {}  Pending {}",
        title,
        trend.latest_running(),
        trend.latest_pending()
    ));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width < 2 || inner.height < 3 {
        return;
    }

    let chart_height = inner.height.saturating_sub(1) as usize;
    let chart_width = inner.width as usize;
    let lines = build_trend_lines(
        &trend.running_points(),
        &trend.pending_points(),
        chart_width,
        chart_height,
        theme,
    );
    frame.render_widget(Paragraph::new(lines), inner);
}

fn visible_column_indices(
    widths: &[u16],
    available_width: u16,
    offset: usize,
) -> (Vec<usize>, usize, usize) {
    let mut used = 0u16;
    let mut indices = Vec::new();
    for index in offset.min(widths.len())..widths.len() {
        let width = widths[index];
        if !indices.is_empty() && used.saturating_add(width) > available_width {
            break;
        }
        used = used.saturating_add(width);
        indices.push(index);
    }
    if indices.is_empty() && !widths.is_empty() {
        indices.push(offset.min(widths.len() - 1));
    }
    let hidden_left = offset.min(widths.len());
    let hidden_right = widths
        .len()
        .saturating_sub(indices.last().map(|value| value + 1).unwrap_or(0));
    (indices, hidden_left, hidden_right)
}

fn max_segments(values: &[String], segment_width: usize) -> usize {
    values
        .iter()
        .map(|value| value.chars().count().max(1).div_ceil(segment_width))
        .max()
        .unwrap_or(1)
        .max(1)
}

fn segment_text(text: &str, segment: usize, width: usize) -> String {
    text.chars().skip(segment * width).take(width).collect()
}

fn build_trend_lines(
    running: &[u64],
    pending: &[u64],
    width: usize,
    height: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let height = height.max(3);
    let width = width.max(2);
    let run = tail_sample(running, width);
    let pend = tail_sample(pending, width);
    let max_value = run
        .iter()
        .chain(pend.iter())
        .copied()
        .max()
        .unwrap_or(1)
        .max(1);

    let mut lines = Vec::with_capacity(height);
    for row in 0..height {
        let threshold = height - row;
        let mut spans = Vec::with_capacity(width);
        for column in 0..width {
            let run_level = scaled_level(run[column], max_value, height);
            let pend_level = scaled_level(pend[column], max_value, height);
            let span = match (run_level >= threshold, pend_level >= threshold) {
                (true, true) => Span::styled("◎", theme.running_pending_overlap),
                (true, false) => Span::styled("●", theme.running),
                (false, true) => Span::styled("○", theme.pending),
                (false, false) => Span::styled("·", theme.muted),
            };
            spans.push(span);
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn tail_sample(values: &[u64], width: usize) -> Vec<u64> {
    if values.is_empty() {
        return vec![0; width];
    }
    if values.len() >= width {
        values[values.len() - width..].to_vec()
    } else {
        let mut padded = vec![0; width - values.len()];
        padded.extend_from_slice(values);
        padded
    }
}

fn scaled_level(value: u64, max_value: u64, height: usize) -> usize {
    (((value as f64 / max_value.max(1) as f64) * height as f64).round() as usize).min(height)
}

fn centered_rect(width_pct: u16, height_pct: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_pct) / 2),
            Constraint::Percentage(height_pct),
            Constraint::Percentage((100 - height_pct) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(vertical[1]);
    horizontal[1]
}

fn sortable_header(
    label: &str,
    active: bool,
    direction: SortDirection,
    theme: &Theme,
) -> Line<'static> {
    if active {
        Line::from(vec![
            Span::styled(label.to_string(), theme.accent.add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(
                direction.arrow().to_string(),
                theme.accent.add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(Span::styled(
            label.to_string(),
            theme.title.add_modifier(Modifier::BOLD),
        ))
    }
}

#[derive(Debug, Clone, Copy)]
struct JobResourceScale {
    max_nodes: u32,
    max_cpus: u32,
    max_gpus: u32,
}

impl JobResourceScale {
    fn from_jobs(jobs: &[&JobRecord]) -> Self {
        Self {
            max_nodes: jobs.iter().map(|job| job.nodes).max().unwrap_or(1).max(1),
            max_cpus: jobs
                .iter()
                .map(|job| job.cpus.unwrap_or(0))
                .max()
                .unwrap_or(1)
                .max(1),
            max_gpus: jobs
                .iter()
                .map(|job| job.requested_gpus.unwrap_or(0))
                .max()
                .unwrap_or(1)
                .max(1),
        }
    }

    fn node_digits(self) -> usize {
        digit_width(self.max_nodes)
    }

    fn cpu_digits(self) -> usize {
        digit_width(self.max_cpus)
    }

    fn gpu_digits(self) -> usize {
        digit_width(self.max_gpus)
    }
}

#[derive(Debug, Clone, Copy)]
struct UserResourceScale {
    max_nodes: u32,
    max_cpus: u32,
    max_gpus: u32,
}

impl UserResourceScale {
    fn from_users(users: &[UserUsage]) -> Self {
        Self {
            max_nodes: users
                .iter()
                .map(|usage| usage.total_nodes())
                .max()
                .unwrap_or(1)
                .max(1),
            max_cpus: users
                .iter()
                .map(|usage| usage.total_cpus().min(u64::from(u32::MAX)) as u32)
                .max()
                .unwrap_or(1)
                .max(1),
            max_gpus: users
                .iter()
                .map(|usage| usage.total_gpus().min(u64::from(u32::MAX)) as u32)
                .max()
                .unwrap_or(1)
                .max(1),
        }
    }

    fn node_digits(self) -> usize {
        digit_width(self.max_nodes)
    }

    fn cpu_digits(self) -> usize {
        digit_width(self.max_cpus)
    }

    fn gpu_digits(self) -> usize {
        digit_width(self.max_gpus)
    }
}

fn metric_value_line(
    value: u64,
    scale: u64,
    width: usize,
    style: Style,
    theme: &Theme,
) -> Line<'static> {
    let filled = scaled_bar_width(value, scale.max(1), width);
    let digits = scale.max(1).to_string().len();
    Line::from(vec![
        Span::styled("[".to_string(), theme.muted),
        Span::styled("█".repeat(filled), style),
        Span::styled("·".repeat(width.saturating_sub(filled)), theme.muted),
        Span::styled("]".to_string(), theme.muted),
        Span::raw(format!(" {:>digits$}", value)),
    ])
}

fn resource_footprint_text(job: &JobRecord, scale: &JobResourceScale) -> String {
    [
        metric_segment_text(
            "Node",
            Some(job.nodes),
            scale.max_nodes,
            scale.node_digits(),
            5,
        ),
        metric_segment_text("CPU", job.cpus, scale.max_cpus, scale.cpu_digits(), 5),
        metric_segment_text(
            "GPU",
            job.requested_gpus,
            scale.max_gpus,
            scale.gpu_digits(),
            5,
        ),
    ]
    .join("  ")
}

fn user_resource_footprint_text(usage: &UserUsage, scale: &UserResourceScale) -> String {
    [
        metric_segment_text(
            "Node",
            Some(usage.total_nodes()),
            scale.max_nodes,
            scale.node_digits(),
            5,
        ),
        metric_segment_text(
            "CPU",
            Some(usage.total_cpus().min(u64::from(u32::MAX)) as u32),
            scale.max_cpus,
            scale.cpu_digits(),
            5,
        ),
        metric_segment_text(
            "GPU",
            Some(usage.total_gpus().min(u64::from(u32::MAX)) as u32),
            scale.max_gpus,
            scale.gpu_digits(),
            5,
        ),
    ]
    .join("  ")
}

fn metric_bar_text(value: u32, max_value: u32, width: usize, digits: usize) -> String {
    let filled = scaled_bar_width(u64::from(value), u64::from(max_value.max(1)), width);
    format!(
        "[{}{}] {:>digits$}",
        "█".repeat(filled),
        "·".repeat(width.saturating_sub(filled)),
        value
    )
}

fn metric_segment_text(
    label: &str,
    value: Option<u32>,
    max_value: u32,
    digits: usize,
    width: usize,
) -> String {
    match value {
        Some(value) => format!(
            "{label:<4} {}",
            metric_bar_text(value, max_value, width, digits)
        ),
        None => format!("{label:<4} [n/a]"),
    }
}

fn digit_width(value: u32) -> usize {
    value.max(1).to_string().len()
}

fn pressure_bar_cell(
    partition: &PartitionOverview,
    metric: MetricMode,
    theme: &Theme,
    width: usize,
) -> Line<'static> {
    pressure_bar_line(
        partition.used_for_pressure(metric),
        partition.capacity_for(metric),
        width,
        theme,
    )
}

fn pressure_bar_line(
    used: u64,
    capacity: Option<u64>,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let Some(capacity) = capacity else {
        return Line::from(Span::styled("[n/a]", theme.muted));
    };
    let capacity = capacity.max(1);
    let ratio = (used as f64 / capacity as f64).clamp(0.0, 1.0);
    let filled = ((width as f64) * ratio).round() as usize;
    let style = if ratio >= 0.85 {
        theme.danger
    } else if ratio >= 0.6 {
        theme.warning
    } else {
        theme.success
    };
    Line::from(vec![
        Span::styled("[".to_string(), theme.muted),
        Span::styled("█".repeat(filled), style),
        Span::styled("·".repeat(width.saturating_sub(filled)), theme.muted),
        Span::styled("]".to_string(), theme.muted),
        Span::raw(format!(" {:>3.0}%", ratio * 100.0)),
    ])
}

#[allow(dead_code)]
fn stacked_usage_cell(
    mine: u64,
    other: u64,
    capacity: Option<u64>,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    stacked_bar_line(mine, other, capacity, width, theme)
}

#[allow(dead_code)]
fn stacked_bar_line(
    mine: u64,
    other: u64,
    capacity: Option<u64>,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let capacity = capacity.unwrap_or((mine + other).max(1)).max(1);
    let mine_width = scaled_bar_width(mine, capacity, width);
    let other_width =
        scaled_bar_width(other, capacity, width).min(width.saturating_sub(mine_width));
    let blank = width.saturating_sub(mine_width + other_width);
    Line::from(vec![
        Span::styled("[".to_string(), theme.muted),
        Span::styled("█".repeat(mine_width), theme.mine),
        Span::styled("█".repeat(other_width), theme.other),
        Span::styled("·".repeat(blank), theme.muted),
        Span::styled("]".to_string(), theme.muted),
        Span::raw(format!("  Mine: {}  Others: {}", mine, other)),
    ])
}

fn scaled_bar_width(value: u64, capacity: u64, width: usize) -> usize {
    (((value as f64 / capacity.max(1) as f64) * width as f64).round() as usize).min(width)
}

fn build_node_state_lines(partition: &PartitionOverview, _theme: &Theme) -> Vec<Line<'static>> {
    let total = partition.total_nodes.max(1) as u64;
    let mut lines = Vec::new();
    for key in ["idle", "mix", "alloc", "drain", "down"] {
        let count = *partition.node_state_counts.get(key).unwrap_or(&0) as u64;
        lines.push(Line::from(vec![
            Span::raw(format!("{:<5} ", key)),
            Span::raw(pressure_bar_text(count, Some(total), 12)),
            Span::raw(format!("  Nodes: {}", count)),
        ]));
    }
    lines
}

fn pressure_bar_text(used: u64, capacity: Option<u64>, width: usize) -> String {
    let Some(capacity) = capacity else {
        return "[n/a]".to_string();
    };
    let capacity = capacity.max(1);
    let ratio = (used as f64 / capacity as f64).clamp(0.0, 1.0);
    let filled = ((width as f64) * ratio).round() as usize;
    format!(
        "[{}{}] {:>3.0}%",
        "█".repeat(filled),
        "·".repeat(width.saturating_sub(filled)),
        ratio * 100.0
    )
}

fn stacked_bar_text(mine: u64, other: u64, capacity: Option<u64>, width: usize) -> String {
    stacked_bar_text_with_labels(mine, other, capacity, width, "Mine", "Others")
}

fn stacked_bar_text_with_labels(
    first: u64,
    second: u64,
    capacity: Option<u64>,
    width: usize,
    first_label: &str,
    second_label: &str,
) -> String {
    let capacity = capacity.unwrap_or((first + second).max(1)).max(1);
    let first_width = scaled_bar_width(first, capacity, width);
    let second_width =
        scaled_bar_width(second, capacity, width).min(width.saturating_sub(first_width));
    let blank = width.saturating_sub(first_width + second_width);
    format!(
        "[{}{}{}] {}: {}  {}: {}",
        "█".repeat(first_width),
        "▓".repeat(second_width),
        "·".repeat(blank),
        first_label,
        first,
        second_label,
        second
    )
}

fn partition_state_style(partition: &PartitionOverview, theme: &Theme) -> Style {
    let state = partition.state.to_ascii_uppercase();
    if state.contains("DOWN") || partition.node_state_counts.contains_key("down") {
        theme.danger
    } else if partition.node_state_counts.contains_key("drain") {
        theme.warning
    } else if partition
        .pressure_ratio(MetricMode::Nodes)
        .is_some_and(|ratio| ratio >= 0.8)
    {
        theme.warning
    } else {
        Style::default()
    }
}

fn job_state_style(job: &JobRecord, theme: &Theme) -> Style {
    match job.state.as_str() {
        "RUNNING" => {
            if job.is_mine {
                theme.mine.add_modifier(Modifier::BOLD)
            } else {
                theme.running
            }
        }
        "PENDING" | "CONFIGURING" => {
            if job.is_mine {
                theme.pending.add_modifier(Modifier::BOLD)
            } else {
                theme.pending
            }
        }
        "FAILED" | "TIMEOUT" | "CANCELLED" => theme.danger,
        _ => {
            if job.is_mine {
                theme.mine
            } else {
                theme.other
            }
        }
    }
}

fn footer_action_label(action: FooterAction) -> &'static str {
    match action {
        FooterAction::BackOverview => "← Overview (b)",
        FooterAction::Refresh => "r Refresh",
        FooterAction::Help => "h Help",
        FooterAction::ToggleMine => "m Mine/All",
        FooterAction::OpenDetail => "Enter Detail",
        FooterAction::CancelJob => "x Cancel",
        FooterAction::BulkCancel => "X Cancel Visible",
        FooterAction::OpenNode => "n Open Node",
        FooterAction::ClearFilters => "c Clear Filters",
    }
}

fn format_partition_resources(
    partition: &PartitionOverview,
    metric: MetricMode,
    usage: u64,
) -> String {
    match metric {
        MetricMode::Jobs => format!(
            "Running jobs: {} / {}",
            usage,
            partition.capacity_for(metric).unwrap_or(0)
        ),
        MetricMode::Nodes => format!(
            "Busy nodes: {} / {}",
            partition.used_for_pressure(metric),
            partition.capacity_for(metric).unwrap_or(0)
        ),
        MetricMode::Cpus => format!(
            "CPU in use: {} / {}",
            usage,
            partition.total_cpus.unwrap_or_default()
        ),
        MetricMode::Gpus => format!(
            "GPU in use: {} / {}",
            usage,
            partition.total_gpus.unwrap_or_default()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        JobResourceScale, build_trend_lines, max_segments, resource_footprint_text,
        scaled_bar_width, visible_column_indices,
    };
    use crate::cli::ThemeChoice;
    use crate::model::JobRecord;
    use crate::ui::theme::Theme;

    fn sample_job(name: &str) -> JobRecord {
        JobRecord {
            job_id: "1234".to_string(),
            user: "alice".to_string(),
            account: None,
            partition_raw: "gpu_l48".to_string(),
            partitions: vec!["gpu_l48".to_string()],
            name: name.to_string(),
            state: "RUNNING".to_string(),
            runtime_raw: "01:00".to_string(),
            time_limit_raw: "02:00".to_string(),
            runtime_secs: Some(3600),
            time_limit_secs: Some(7200),
            nodes: 2,
            cpus: Some(48),
            memory_mb: Some(131072),
            requested_gpus: Some(4),
            gres: None,
            req_tres: None,
            alloc_tres: None,
            location_or_reason: "node001".to_string(),
            submit_time: None,
            priority: None,
            is_mine: true,
            active: true,
            running: true,
            pending: false,
        }
    }

    #[test]
    fn bar_width_scales_safely() {
        assert_eq!(scaled_bar_width(0, 10, 20), 0);
        assert_eq!(scaled_bar_width(5, 10, 20), 10);
        assert_eq!(scaled_bar_width(15, 10, 20), 20);
    }

    #[test]
    fn visible_columns_can_scroll_to_rightmost_name_column() {
        let widths = vec![8, 10, 12, 16, 22, 80];
        let (indices, hidden_left, hidden_right) = visible_column_indices(&widths, 40, 5);
        assert_eq!(indices, vec![5]);
        assert_eq!(hidden_left, 5);
        assert_eq!(hidden_right, 0);
    }

    #[test]
    fn trend_lines_use_requested_height() {
        let theme = Theme::from_choice(ThemeChoice::Dark, false);
        let lines = build_trend_lines(&[1, 2, 3, 4], &[4, 3, 2, 1], 4, 6, &theme);
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn resource_footprint_shows_only_node_cpu_gpu() {
        let job = sample_job("very-long-training-job-name-for-footprint-check");
        let scale = JobResourceScale::from_jobs(&[&job]);
        let text = resource_footprint_text(&job, &scale);
        assert!(text.contains("Node"));
        assert!(text.contains("CPU"));
        assert!(text.contains("GPU"));
        assert!(!text.contains("Mem"));
        assert!(!text.contains("TRES"));
    }

    #[test]
    fn long_name_and_resource_text_expand_into_multiple_segments() {
        let job = sample_job(
            "extremely-long-training-job-name-that-should-span-multiple-visible-columns",
        );
        let scale = JobResourceScale::from_jobs(&[&job]);
        let footprint = resource_footprint_text(&job, &scale);
        assert!(max_segments(&[job.name.clone()], super::NAME_SEGMENT_WIDTH) > 1);
        assert!(max_segments(&[footprint], super::RESOURCE_SEGMENT_WIDTH) > 1);
    }
}
