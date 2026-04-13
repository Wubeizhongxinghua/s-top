mod theme;

use std::cell::RefCell;

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
        Page::UserJobs => render_user_jobs(frame, content_area, app, theme, hit_map),
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
    let partition_segment_width = adaptive_segment_width(sections[1].width, 18, 10);
    let summary_segment_width = adaptive_segment_width(sections[1].width, 24, 12);
    let resource_segment_width = adaptive_segment_width(sections[1].width, 24, 12);
    let pressure_width = adaptive_segment_width(sections[1].width, 23, 12) as u16;
    let metric_width = adaptive_segment_width(sections[1].width, 18, 12) as u16;
    let pressure_bar_width = pressure_width.saturating_sub(7).max(3) as usize;
    let running_digits = running_capacity.max(1).to_string().len();
    let pending_digits = pending_capacity.max(1).to_string().len();
    let running_metric_bar_width = metric_width
        .saturating_sub((running_digits + 4) as u16)
        .max(3) as usize;
    let pending_metric_bar_width = metric_width
        .saturating_sub((pending_digits + 4) as u16)
        .max(3) as usize;
    let state_values = rows
        .iter()
        .map(|partition| partition.state.clone())
        .collect::<Vec<_>>();
    let partition_names = rows
        .iter()
        .map(|partition| partition.name.clone())
        .collect::<Vec<_>>();
    let total_job_texts = rows
        .iter()
        .map(|partition| {
            format!(
                "Running: {}  Pending: {}",
                partition.total_usage().running_total(MetricMode::Jobs),
                partition.total_usage().pending_total(MetricMode::Jobs),
            )
        })
        .collect::<Vec<_>>();
    let resource_texts = rows
        .iter()
        .map(|partition| {
            format_partition_resources(
                partition,
                app.metric_mode,
                partition.used_for_pressure(app.metric_mode),
            )
        })
        .collect::<Vec<_>>();
    let partition_segments = max_segments(&partition_names, partition_segment_width);
    let total_segments = max_segments(&total_job_texts, summary_segment_width);
    let resource_segments = max_segments(&resource_texts, resource_segment_width);
    let state_width = adaptive_text_width(&state_values, sections[1].width, 8, 18, 8);

    let mut headers = Vec::new();
    let mut column_widths = Vec::new();
    let mut header_hits = Vec::new();
    for segment in 0..partition_segments {
        headers.push(sortable_header(
            if partition_segments > 1 && segment > 0 {
                "Partition →"
            } else {
                "Partition"
            },
            app.overview_sort.column == OverviewColumn::Partition,
            app.overview_sort.direction,
            theme,
        ));
        column_widths.push(partition_segment_width as u16);
        header_hits.push(Some(MouseHit::OverviewHeader(OverviewColumn::Partition)));
    }
    headers.push(Line::from("State"));
    column_widths.push(state_width);
    header_hits.push(None);
    headers.push(sortable_header(
        "Nodes",
        app.overview_sort.column == OverviewColumn::Nodes,
        app.overview_sort.direction,
        theme,
    ));
    column_widths.push(7);
    header_hits.push(Some(MouseHit::OverviewHeader(OverviewColumn::Nodes)));
    headers.push(sortable_header(
        "Pressure",
        app.overview_sort.column == OverviewColumn::Pressure,
        app.overview_sort.direction,
        theme,
    ));
    column_widths.push(pressure_width);
    header_hits.push(Some(MouseHit::OverviewHeader(OverviewColumn::Pressure)));
    headers.push(sortable_header(
        "Mine Running",
        app.overview_sort.column == OverviewColumn::MineRunning,
        app.overview_sort.direction,
        theme,
    ));
    column_widths.push(metric_width);
    header_hits.push(Some(MouseHit::OverviewHeader(OverviewColumn::MineRunning)));
    headers.push(sortable_header(
        "Mine Pending",
        app.overview_sort.column == OverviewColumn::MinePending,
        app.overview_sort.direction,
        theme,
    ));
    column_widths.push(metric_width);
    header_hits.push(Some(MouseHit::OverviewHeader(OverviewColumn::MinePending)));
    headers.push(sortable_header(
        "Others Running",
        app.overview_sort.column == OverviewColumn::OthersRunning,
        app.overview_sort.direction,
        theme,
    ));
    column_widths.push(metric_width);
    header_hits.push(Some(MouseHit::OverviewHeader(
        OverviewColumn::OthersRunning,
    )));
    headers.push(sortable_header(
        "Others Pending",
        app.overview_sort.column == OverviewColumn::OthersPending,
        app.overview_sort.direction,
        theme,
    ));
    column_widths.push(metric_width);
    header_hits.push(Some(MouseHit::OverviewHeader(
        OverviewColumn::OthersPending,
    )));
    for segment in 0..total_segments {
        headers.push(sortable_header(
            if total_segments > 1 && segment > 0 {
                "Total Jobs →"
            } else {
                "Total Jobs"
            },
            app.overview_sort.column == OverviewColumn::TotalJobs,
            app.overview_sort.direction,
            theme,
        ));
        column_widths.push(summary_segment_width as u16);
        header_hits.push(Some(MouseHit::OverviewHeader(OverviewColumn::TotalJobs)));
    }
    for segment in 0..resource_segments {
        headers.push(Line::from(if resource_segments > 1 && segment > 0 {
            "Resources →"
        } else {
            "Resources"
        }));
        column_widths.push(resource_segment_width as u16);
        header_hits.push(None);
    }

    let full_rows: Vec<(Vec<Cell>, Style)> = rows
        .iter()
        .zip(total_job_texts.iter().zip(resource_texts.iter()))
        .map(|(partition, (total_jobs, resources))| {
            let pressure = pressure_bar_cell(partition, app.metric_mode, theme, pressure_bar_width);
            let mine_running = metric_value_line(
                partition.mine.running_total(app.metric_mode),
                running_capacity,
                running_metric_bar_width,
                theme.mine,
                theme,
            );
            let mine_pending = metric_value_line(
                partition.mine.pending_total(app.metric_mode),
                pending_capacity,
                pending_metric_bar_width,
                theme.mine,
                theme,
            );
            let others_running = metric_value_line(
                partition.others.running_total(app.metric_mode),
                running_capacity,
                running_metric_bar_width,
                theme.other,
                theme,
            );
            let others_pending = metric_value_line(
                partition.others.pending_total(app.metric_mode),
                pending_capacity,
                pending_metric_bar_width,
                theme.other,
                theme,
            );
            let mut cells = Vec::new();
            for segment in 0..partition_segments {
                cells.push(Cell::from(Line::from(vec![Span::styled(
                    segment_text(&partition.name, segment, partition_segment_width),
                    theme
                        .partition_style(&partition.name)
                        .add_modifier(Modifier::BOLD),
                )])));
            }
            cells.extend([
                Cell::from(partition.state.clone()),
                Cell::from(partition.total_nodes.to_string()),
                Cell::from(pressure),
                Cell::from(mine_running),
                Cell::from(mine_pending),
                Cell::from(others_running),
                Cell::from(others_pending),
            ]);
            for segment in 0..total_segments {
                cells.push(Cell::from(segment_text(
                    total_jobs,
                    segment,
                    summary_segment_width,
                )));
            }
            for segment in 0..resource_segments {
                cells.push(Cell::from(segment_text(
                    resources,
                    segment,
                    resource_segment_width,
                )));
            }
            (cells, partition_state_style(partition, theme))
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
    let table_rows: Vec<Row> = full_rows
        .into_iter()
        .map(|(cells, style)| {
            Row::new(
                visible_indices
                    .iter()
                    .map(|index| cells[*index].clone())
                    .collect::<Vec<_>>(),
            )
            .style(style)
        })
        .collect();

    let table = Table::new(table_rows, constraints.clone())
        .header(Row::new(visible_headers).style(theme.title.add_modifier(Modifier::BOLD)))
        .block(
            Block::default()
                .title(wide_table_title(
                    "Partition Overview",
                    hidden_left,
                    hidden_right,
                ))
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
    register_header_hits(hit_map, sections[1], &constraints, &visible_hits);
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
            "Metric: {}  Sort: {}  Current user: {}  Horizontal view: column {} (← →)",
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
    let back_label = "← Overview";
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
    let resource_segment_width =
        adaptive_segment_width(sections[1].width, RESOURCE_SEGMENT_WIDTH, 12);
    let name_segment_width = adaptive_segment_width(sections[1].width, NAME_SEGMENT_WIDTH, 12);
    let resource_texts = rows
        .iter()
        .map(|job| resource_footprint_text(job, &resource_scale))
        .collect::<Vec<_>>();
    let resource_segments = max_segments(&resource_texts, resource_segment_width);
    let name_segments = max_segments(
        &rows.iter().map(|job| job.name.clone()).collect::<Vec<_>>(),
        name_segment_width,
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
        column_widths.push(resource_segment_width as u16);
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
        column_widths.push(name_segment_width as u16);
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
                    resource_segment_width,
                )));
            }
            cells.push(Cell::from(job.location_or_reason.clone()));
            for segment in 0..name_segments {
                cells.push(Cell::from(segment_text(
                    &job.name,
                    segment,
                    name_segment_width,
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

    let title = wide_table_title(
        if mine_only { "My Jobs" } else { "All Jobs" },
        hidden_left,
        hidden_right,
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

fn render_user_jobs(
    frame: &mut Frame,
    area: Rect,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let user_name = app.detail_user_name().unwrap_or("unknown user");
    let rows = app.visible_detail_user_jobs();
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(8)])
        .split(area);
    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(30), Constraint::Length(18)])
        .split(sections[0]);
    let header_lines = vec![
        Line::from(vec![
            Span::styled(
                format!("Jobs for {}", user_name),
                theme.title.add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  Visible jobs: {}  State filter: {}",
                rows.len(),
                app.job_filter_label()
            )),
        ]),
        Line::from(format!(
            "Sort: {}  Search query: {}  Horizontal view: column {} (← →)",
            app.sort_label(),
            if app.active_global_query().is_empty() {
                "All jobs for this user".to_string()
            } else {
                app.active_global_query().to_string()
            },
            app.job_horizontal_offset + 1
        )),
    ];
    frame.render_widget(
        Paragraph::new(header_lines)
            .block(Block::default().borders(Borders::ALL).title("User Jobs"))
            .wrap(Wrap { trim: true }),
        header_layout[0],
    );
    frame.render_widget(
        Paragraph::new("← Users")
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

    let resource_scale = JobResourceScale::from_jobs(&rows);
    let resource_segment_width =
        adaptive_segment_width(sections[1].width, RESOURCE_SEGMENT_WIDTH, 12);
    let name_segment_width = adaptive_segment_width(sections[1].width, NAME_SEGMENT_WIDTH, 12);
    let resource_texts = rows
        .iter()
        .map(|job| resource_footprint_text(job, &resource_scale))
        .collect::<Vec<_>>();
    let resource_segments = max_segments(&resource_texts, resource_segment_width);
    let name_segments = max_segments(
        &rows.iter().map(|job| job.name.clone()).collect::<Vec<_>>(),
        name_segment_width,
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
        column_widths.push(resource_segment_width as u16);
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
        column_widths.push(name_segment_width as u16);
        header_hits.push(Some(MouseHit::JobHeader(JobColumn::Name)));
    }

    let full_rows: Vec<Vec<Cell>> = rows
        .iter()
        .zip(resource_texts.iter())
        .map(|(job, resource_text)| {
            let mut cells = vec![
                Cell::from(job.job_id.clone()),
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
            ];
            for segment in 0..resource_segments {
                cells.push(Cell::from(segment_text(
                    resource_text,
                    segment,
                    resource_segment_width,
                )));
            }
            cells.push(Cell::from(job.location_or_reason.clone()));
            for segment in 0..name_segments {
                cells.push(Cell::from(segment_text(
                    &job.name,
                    segment,
                    name_segment_width,
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

    let title = wide_table_title(
        &format!("User Jobs: {}", user_name),
        hidden_left,
        hidden_right,
    );
    if rows.is_empty() {
        hit_map.set_page_rows(1);
        frame.render_widget(
            Paragraph::new(format!(
                "No active jobs for {} match the current filters.",
                user_name
            ))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(title)),
            sections[1],
        );
        return;
    }

    let table = Table::new(table_rows, constraints.clone())
        .header(Row::new(visible_headers).style(theme.title.add_modifier(Modifier::BOLD)))
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(theme.highlight)
        .highlight_symbol(">> ");
    let mut state = TableState::default();
    state.select(Some(
        app.selected_user_jobs.min(rows.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(table, sections[1], &mut state);
    register_table_rows(hit_map, sections[1], rows.len(), RowKind::UserJobs);
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
            "Sort: {}  Search query: {}  Horizontal view: column {} (← →)",
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
    let footprint_segment_width =
        adaptive_segment_width(sections[1].width, RESOURCE_SEGMENT_WIDTH, 12);
    let partition_segment_width =
        adaptive_segment_width(sections[1].width, PARTITION_SEGMENT_WIDTH, 12);
    let user_footprints = users
        .iter()
        .map(|usage| user_resource_footprint_text(usage, &user_scale))
        .collect::<Vec<_>>();
    let footprint_segments = max_segments(&user_footprints, footprint_segment_width);
    let partition_texts = users
        .iter()
        .map(|usage| usage.top_partitions_summary(3))
        .collect::<Vec<_>>();
    let partition_segments = max_segments(&partition_texts, partition_segment_width);
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
        column_widths.push(footprint_segment_width as u16);
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
        column_widths.push(partition_segment_width as u16);
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
                    footprint_segment_width,
                )));
            }
            for segment in 0..partition_segments {
                cells.push(Cell::from(segment_text(
                    partition_text,
                    segment,
                    partition_segment_width,
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
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(wide_table_title("Active users", hidden_left, hidden_right)),
            )
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
    let lower = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(sections[2]);
    let selected_resource_segment_width =
        adaptive_segment_width(lower[1].width, RESOURCE_SEGMENT_WIDTH, 12);
    let selected_name_segment_width =
        adaptive_segment_width(lower[1].width, NAME_SEGMENT_WIDTH, 12);
    let selected_resource_texts = selected_jobs
        .iter()
        .map(|job| resource_footprint_text(job, &resource_scale))
        .collect::<Vec<_>>();
    let resource_segments = max_segments(&selected_resource_texts, selected_resource_segment_width);
    let name_segments = max_segments(
        &selected_jobs
            .iter()
            .map(|job| job.name.clone())
            .collect::<Vec<_>>(),
        selected_name_segment_width,
    );
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
        column_widths.push(selected_resource_segment_width as u16);
    }
    for segment in 0..name_segments {
        headers.push(Line::from(if name_segments > 1 && segment > 0 {
            "Name →"
        } else {
            "Name"
        }));
        column_widths.push(selected_name_segment_width as u16);
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
                    selected_resource_segment_width,
                )));
            }
            for segment in 0..name_segments {
                cells.push(Cell::from(segment_text(
                    &job.name,
                    segment,
                    selected_name_segment_width,
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
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(wide_table_title(
                        "Jobs for selected user",
                        hidden_left,
                        hidden_right,
                    )),
            ),
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
    let resource_segment_width =
        adaptive_segment_width(layout[1].width, RESOURCE_SEGMENT_WIDTH, 12);
    let name_segment_width = adaptive_segment_width(layout[1].width, NAME_SEGMENT_WIDTH, 12);
    let resource_texts = jobs
        .iter()
        .map(|job| resource_footprint_text(job, &resource_scale))
        .collect::<Vec<_>>();
    let resource_segments = max_segments(&resource_texts, resource_segment_width);
    let name_segments = max_segments(
        &jobs.iter().map(|job| job.name.clone()).collect::<Vec<_>>(),
        name_segment_width,
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
        column_widths.push(resource_segment_width as u16);
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
        column_widths.push(name_segment_width as u16);
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
                    resource_segment_width,
                )));
            }
            for segment in 0..name_segments {
                cells.push(Cell::from(segment_text(
                    &job.name,
                    segment,
                    name_segment_width,
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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(wide_table_title("Jobs", hidden_left, hidden_right)),
        )
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
    let running_ownership = styled_stacked_bar_line(
        partition.mine.running_total(app.metric_mode),
        partition.others.running_total(app.metric_mode),
        partition.capacity_for(app.metric_mode),
        26,
        "Mine",
        "Others",
        theme.mine,
        theme.other,
        theme,
    );
    let pending_ownership = styled_stacked_bar_line(
        partition.mine.pending_total(app.metric_mode),
        partition.others.pending_total(app.metric_mode),
        Some(
            partition.mine.pending_total(app.metric_mode)
                + partition.others.pending_total(app.metric_mode)
                + 1,
        ),
        26,
        "Mine",
        "Others",
        theme.mine,
        theme.other,
        theme,
    );
    let running_pending = styled_stacked_bar_line(
        u64::from(total.running_jobs),
        u64::from(total.pending_jobs),
        Some(u64::from(total.running_jobs + total.pending_jobs).max(1)),
        26,
        "Running",
        "Pending",
        theme.running,
        theme.pending,
        theme,
    );
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
        {
            let mut spans = vec![Span::raw("Running ownership: ")];
            spans.extend(running_ownership.spans);
            Line::from(spans)
        },
        {
            let mut spans = vec![Span::raw("Pending ownership: ")];
            spans.extend(pending_ownership.spans);
            Line::from(spans)
        },
        {
            let mut spans = vec![Span::raw("Running versus pending jobs: ")];
            spans.extend(running_pending.spans);
            Line::from(spans)
        },
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
    let mut node_state = TableState::default();
    node_state.select(if nodes.is_empty() {
        None
    } else {
        Some(
            app.selected_partition_node
                .min(nodes.len().saturating_sub(1)),
        )
    });
    let node_names = nodes
        .iter()
        .map(|node| node.node_name.clone())
        .collect::<Vec<_>>();
    let node_name_width = adaptive_text_width(&node_names, trend_and_nodes[1].width, 10, 40, 12);
    let node_state_values = nodes
        .iter()
        .map(|node| node.state.clone())
        .collect::<Vec<_>>();
    let node_state_width =
        adaptive_text_width(&node_state_values, trend_and_nodes[1].width, 8, 18, 9);
    let capacity_texts = nodes
        .iter()
        .map(|node| {
            format!(
                "CPUs {}  GPUs {}",
                node.cpus
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
                node.gpus
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "N/A".to_string())
            )
        })
        .collect::<Vec<_>>();
    let capacity_segment_width = adaptive_segment_width(trend_and_nodes[1].width, 24, 12);
    let capacity_segments = max_segments(&capacity_texts, capacity_segment_width);
    let mut headers = vec![Line::from("Node"), Line::from("State")];
    let mut column_widths = vec![node_name_width, node_state_width];
    for segment in 0..capacity_segments {
        headers.push(Line::from(if capacity_segments > 1 && segment > 0 {
            "Capacity →"
        } else {
            "Capacity"
        }));
        column_widths.push(capacity_segment_width as u16);
    }
    let full_rows: Vec<Vec<Cell>> = nodes
        .iter()
        .zip(capacity_texts.iter())
        .map(|(node, capacity_text)| {
            let mut cells = vec![
                Cell::from(node.node_name.clone()),
                Cell::from(node.state.clone()),
            ];
            for segment in 0..capacity_segments {
                cells.push(Cell::from(segment_text(
                    capacity_text,
                    segment,
                    capacity_segment_width,
                )));
            }
            cells
        })
        .collect();
    let (visible_indices, hidden_left, hidden_right) = visible_column_indices(
        &column_widths,
        trend_and_nodes[1].width.saturating_sub(4),
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
    let visible_rows = full_rows
        .into_iter()
        .map(|cells| {
            Row::new(
                visible_indices
                    .iter()
                    .map(|index| cells[*index].clone())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    frame.render_stateful_widget(
        Table::new(visible_rows, constraints)
            .header(Row::new(visible_headers).style(theme.title.add_modifier(Modifier::BOLD)))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(wide_table_title(
                        "Nodes in this Partition",
                        hidden_left,
                        hidden_right,
                    )),
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
    let resource_segment_width =
        adaptive_segment_width(sections[2].width, RESOURCE_SEGMENT_WIDTH, 12);
    let name_segment_width = adaptive_segment_width(sections[2].width, NAME_SEGMENT_WIDTH, 12);
    let resource_texts = jobs
        .iter()
        .map(|job| resource_footprint_text(job, &resource_scale))
        .collect::<Vec<_>>();
    let resource_segments = max_segments(&resource_texts, resource_segment_width);
    let name_segments = max_segments(
        &jobs.iter().map(|job| job.name.clone()).collect::<Vec<_>>(),
        name_segment_width,
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
        column_widths.push(resource_segment_width as u16);
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
        column_widths.push(name_segment_width as u16);
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
                    resource_segment_width,
                )));
            }
            for segment in 0..name_segments {
                cells.push(Cell::from(segment_text(
                    &job.name,
                    segment,
                    name_segment_width,
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
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(wide_table_title(
                        "Jobs on this Node",
                        hidden_left,
                        hidden_right,
                    )),
            )
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

    let history_name_segment_width =
        adaptive_segment_width(sections[1].width, NAME_SEGMENT_WIDTH, 12);
    let history_partition_width = adaptive_text_width(
        &rows
            .iter()
            .map(|history| history.partition.clone().unwrap_or_else(|| "-".to_string()))
            .collect::<Vec<_>>(),
        sections[1].width,
        10,
        28,
        12,
    );
    let history_name_segments = max_segments(
        &rows
            .iter()
            .map(|history| history.name.clone())
            .collect::<Vec<_>>(),
        history_name_segment_width,
    );
    let resource_width = adaptive_segment_width(sections[1].width, 18, 12) as u16;
    let mut column_widths = vec![8, 10, history_partition_width, 12, 8, 10, 19];
    let mut headers = vec![
        Line::from("JobID"),
        Line::from("User"),
        Line::from("Partition"),
        Line::from("State"),
        Line::from("Exit"),
        Line::from("Elapsed"),
        Line::from("End"),
    ];
    if app.settings.show_advanced_resources {
        column_widths.push(resource_width);
        headers.push(Line::from("AllocTRES"));
    }
    for segment in 0..history_name_segments {
        headers.push(Line::from(if history_name_segments > 1 && segment > 0 {
            "Name →"
        } else {
            "Name"
        }));
        column_widths.push(history_name_segment_width as u16);
    }
    let full_rows: Vec<Vec<Cell>> = rows
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
            for segment in 0..history_name_segments {
                cells.push(Cell::from(segment_text(
                    &history.name,
                    segment,
                    history_name_segment_width,
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
    let visible_rows: Vec<Row> = rows
        .iter()
        .zip(full_rows.iter())
        .map(|(history, cells)| {
            Row::new(
                visible_indices
                    .iter()
                    .map(|index| cells[*index].clone())
                    .collect::<Vec<_>>(),
            )
            .style(if history.is_mine {
                theme.mine
            } else {
                theme.other
            })
        })
        .collect();

    let table = Table::new(visible_rows, constraints)
        .header(Row::new(visible_headers).style(theme.title.add_modifier(Modifier::BOLD)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(wide_table_title("History Jobs", hidden_left, hidden_right)),
        )
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
        "Navigation: Tab switch top-level pages  j/k move selection  Space next page  b previous page  Enter open detail  i show job detail  ← → horizontal view in wide tables  Mouse click or double-click  Wheel scroll"
    );
    frame.render_widget(
        Paragraph::new(nav)
            .style(theme.muted)
            .block(Block::default()),
        sections[1],
    );

    let tail = if let Some(modal) = &app.modal {
        let modal_hint = match modal {
            Modal::Help => "Help: j/k scroll  Space/b page  g/G top/bottom  q or Esc close",
            Modal::JobDetail(_) => {
                "Job Detail: j/k scroll  Space/b page  g/G top/bottom  q back  Wheel scroll"
            }
            Modal::ConfirmCancel(_) => {
                "Cancel Preview: j/k scroll  Space/b page  g/G top/bottom  Enter/y confirm  q/Esc/n back"
            }
            Modal::CancelResult(_) => {
                "Cancel Result: j/k scroll  Space/b page  g/G top/bottom  q back  Wheel scroll"
            }
        };
        modal_hint.to_string()
    } else {
        let page_hint = match app.current_page() {
            Page::Overview => {
                "Overview: q Quit  g change metric  p pin partition  Enter open partition detail  ← → horizontal view  Click column headers to sort"
            }
            Page::MyJobs | Page::AllJobs => {
                "Queue: q Quit  f change state filter  s change sort  m toggle mine-only  x cancel selected job  X review visible jobs for cancel  Enter open job detail  ← → horizontal view"
            }
            Page::PartitionDetail => {
                "Partition: q Back  Tab switch top-level pages  s change job sort  [ or ] choose node  n open selected node  g metric  m toggle mine-only  x cancel selected job  ← → horizontal view"
            }
            Page::Users => {
                "Users: q Quit  Enter open selected user's jobs  s change sort  m toggle mine-only  Space / b page  ← → horizontal view  Click column headers to sort"
            }
            Page::UserJobs => {
                "User Jobs: q Back  Enter open job detail  f change state filter  s change sort  x cancel selected job  X review visible jobs for cancel  Space / b page  ← → horizontal view"
            }
            Page::NodeDetail => {
                "Node: q Back  u user filter  f state filter  w where filter  y why filter  c clear filters  x cancel selected job  Space / b page  ← → horizontal view"
            }
        };
        format!(
            "{}  Search: /  Help: h  Mouse: tabs, rows, footer buttons, and modal buttons",
            page_hint
        )
    };
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
        let label = footer_action_label(action, app);
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
        Modal::JobDetail(detail) => render_job_detail_modal(frame, detail, app, theme, hit_map),
        Modal::ConfirmCancel(preview) => {
            render_cancel_confirm_modal(frame, preview, app, theme, hit_map)
        }
        Modal::CancelResult(report) => {
            render_cancel_result_modal(frame, report, app, theme, hit_map)
        }
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

fn render_help_modal(frame: &mut Frame, app: &AppState, theme: &Theme, hit_map: &mut UiHitMap) {
    let area = centered_rect(78, 78, frame.area());
    frame.render_widget(Clear, area);
    hit_map.push(area, MouseHit::Modal(ModalAction::Ignore));
    let text = vec![
        Line::from(Span::styled(
            "Navigation",
            theme.title.add_modifier(Modifier::BOLD),
        )),
        Line::from(
            "Tab / Shift-Tab switch top-level pages. j/k or arrows move the current list. Space pages forward, b pages back, and Enter opens the focused detail.",
        ),
        Line::from(
            "q goes back from detail pages and quits only from top-level pages. Esc also closes the current detail or modal.",
        ),
        Line::from(
            "Mouse: click tabs, click rows to select, double-click rows to open, wheel scrolls lists and modal pages, footer buttons are clickable.",
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
            "User View ranks active users by jobs and resources, then Enter opens a dedicated User Jobs page for the selected user.",
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
            "Overview, Users, and wide queue tables can be moved horizontally with ← / →. In modal detail pages, click outside the panel to close.",
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
        Line::from("Close with Enter / Esc / h / q. Scroll with j/k, arrows, Space, b, g, or G."),
    ];
    render_scrollable_detail_modal(
        frame,
        area,
        "Help",
        text,
        "j/k move  Space/b page  g/G top/bottom  q Close  Wheel scroll",
        app.modal_scroll(),
        theme.muted,
        theme,
        hit_map,
    );
}

thread_local! {
    static DETAIL_LAYOUT_CACHE: RefCell<Option<DetailLayoutCache>> = RefCell::new(None);
}

#[derive(Clone)]
struct DetailLayoutCache {
    revision: u64,
    width: u16,
    lines: Vec<Line<'static>>,
}

#[derive(Clone)]
struct KvField {
    label: String,
    value: String,
    label_style: Option<Style>,
    value_style: Option<Style>,
}

fn kv_styled(
    label: impl Into<String>,
    value: impl Into<String>,
    label_style: Style,
    value_style: Style,
) -> KvField {
    KvField {
        label: label.into(),
        value: value.into(),
        label_style: Some(label_style),
        value_style: Some(value_style),
    }
}

fn scroll_metrics(
    line_count: usize,
    height: u16,
    scroll: usize,
) -> (usize, usize, usize, usize, usize) {
    let visible_rows = height.saturating_sub(2) as usize;
    let total_rows = line_count.max(1);
    let max_scroll = total_rows.saturating_sub(visible_rows.max(1));
    let clamped = scroll.min(max_scroll);
    let start_line = if total_rows == 0 { 0 } else { clamped + 1 };
    let end_line = if total_rows == 0 {
        0
    } else {
        (clamped + visible_rows.max(1)).min(total_rows)
    };
    (
        clamped,
        total_rows,
        start_line,
        end_line,
        visible_rows.max(1),
    )
}

fn cached_detail_lines<F>(revision: u64, width: u16, build: F) -> Vec<Line<'static>>
where
    F: FnOnce(usize) -> Vec<Line<'static>>,
{
    DETAIL_LAYOUT_CACHE.with(|cache| {
        if let Some(cache) = cache.borrow().as_ref()
            && cache.revision == revision
            && cache.width == width
        {
            return cache.lines.clone();
        }
        let content_width = width.saturating_sub(2).max(1) as usize;
        let lines = build(content_width);
        *cache.borrow_mut() = Some(DetailLayoutCache {
            revision,
            width,
            lines: lines.clone(),
        });
        lines
    })
}

fn chunk_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    if text.is_empty() {
        return vec![String::new()];
    }
    let chars: Vec<char> = text.chars().collect();
    chars
        .chunks(width)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in text.split('\n') {
        lines.extend(chunk_text(raw, width));
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn pad_right(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        text.chars().take(width).collect()
    } else {
        format!("{}{}", text, " ".repeat(width - len))
    }
}

fn truncate_chars(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

fn line_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

fn state_style(state: &str, theme: &Theme) -> Style {
    let upper = state.to_ascii_uppercase();
    if upper.contains("RUN") || upper.contains("COMPLETING") || upper.contains("CONFIGURING") {
        theme.running
    } else if upper.contains("PEND") {
        theme.pending
    } else if upper.contains("FAIL")
        || upper.contains("CANCEL")
        || upper.contains("TIMEOUT")
        || upper.contains("DOWN")
    {
        theme.danger
    } else {
        theme.accent
    }
}

fn is_skipped_message(message: &str) -> bool {
    let upper = message.to_ascii_uppercase();
    upper.contains("SKIP") || upper.contains("NOT ALLOWED") || upper.contains("DENIED")
}

fn border_line(
    left: char,
    title: Option<(&str, Style)>,
    inner: usize,
    frame_style: Style,
) -> Line<'static> {
    match left {
        '┌' => {
            let title_text = truncate_chars(
                &format!(" {} ", title.map(|(text, _)| text).unwrap_or("")),
                inner,
            );
            let title_len = title_text.chars().count();
            let fill = inner.saturating_sub(title_len);
            if let Some((_, title_style)) = title {
                Line::from(vec![
                    Span::styled("┌", frame_style),
                    Span::styled(title_text, title_style.add_modifier(Modifier::BOLD)),
                    Span::styled("─".repeat(fill), frame_style),
                    Span::styled("┐", frame_style),
                ])
            } else {
                Line::from(Span::styled(
                    format!("┌{}┐", "─".repeat(inner)),
                    frame_style,
                ))
            }
        }
        '└' => Line::from(Span::styled(
            format!("└{}┘", "─".repeat(inner)),
            frame_style,
        )),
        _ => Line::from(Span::styled(
            format!(
                "{}{}{}",
                left,
                "─".repeat(inner),
                match left {
                    '├' => '┤',
                    _ => right_border_for(left),
                }
            ),
            frame_style,
        )),
    }
}

fn right_border_for(left: char) -> char {
    match left {
        '├' => '┤',
        _ => '┘',
    }
}

fn blank_box_line(inner: usize, frame_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled("│", frame_style),
        Span::raw(" ".repeat(inner)),
        Span::styled("│", frame_style),
    ])
}

fn render_kv_box(
    title: &str,
    fields: &[KvField],
    width: usize,
    stacked: bool,
    frame_style: Style,
    title_style: Style,
    label_style: Style,
) -> Vec<Line<'static>> {
    let inner = width.saturating_sub(2).max(1);
    let mut lines = Vec::new();
    lines.push(border_line(
        '┌',
        Some((title, title_style)),
        inner,
        frame_style,
    ));

    if stacked {
        for field in fields {
            let field_label_style = field
                .label_style
                .unwrap_or(label_style)
                .add_modifier(Modifier::BOLD);
            let field_value_style = field.value_style.unwrap_or(Style::default());
            lines.push(Line::from(vec![
                Span::styled("│", frame_style),
                Span::styled(pad_right(&field.label, inner), field_label_style),
                Span::styled("│", frame_style),
            ]));
            for wrapped in wrap_text(&field.value, inner.saturating_sub(2).max(1)) {
                lines.push(Line::from(vec![
                    Span::styled("│", frame_style),
                    Span::styled(
                        pad_right(&format!("  {}", wrapped), inner),
                        field_value_style,
                    ),
                    Span::styled("│", frame_style),
                ]));
            }
            lines.push(blank_box_line(inner, frame_style));
        }
        if fields.is_empty() {
            lines.push(blank_box_line(inner, frame_style));
        }
    } else {
        let label_width = fields
            .iter()
            .map(|field| field.label.chars().count())
            .max()
            .unwrap_or(0)
            .clamp(8, 18);
        let value_width = inner.saturating_sub(label_width + 1).max(1);
        for field in fields {
            let wrapped = wrap_text(&field.value, value_width);
            let field_label_style = field
                .label_style
                .unwrap_or(label_style)
                .add_modifier(Modifier::BOLD);
            let field_value_style = field.value_style.unwrap_or(Style::default());
            for (index, part) in wrapped.iter().enumerate() {
                let label = if index == 0 {
                    pad_right(&field.label, label_width)
                } else {
                    " ".repeat(label_width)
                };
                lines.push(Line::from(vec![
                    Span::styled("│", frame_style),
                    Span::styled(label, field_label_style),
                    Span::raw(" "),
                    Span::styled(pad_right(part, value_width), field_value_style),
                    Span::styled("│", frame_style),
                ]));
            }
        }
        if fields.is_empty() {
            lines.push(blank_box_line(inner, frame_style));
        }
    }

    while lines.len() > 2
        && line_width(lines.last().unwrap()) == inner + 2
        && line_width(&lines[lines.len() - 2]) == inner + 2
    {
        let blank = format!("│{}│", " ".repeat(inner));
        let last_text: String = lines
            .last()
            .unwrap()
            .spans
            .iter()
            .map(|span| span.content.to_string())
            .collect();
        let prev_text: String = lines[lines.len() - 2]
            .spans
            .iter()
            .map(|span| span.content.to_string())
            .collect();
        if last_text == blank && prev_text == blank {
            lines.pop();
        } else {
            break;
        }
    }

    lines.push(border_line('└', None, inner, frame_style));
    lines
}

fn combine_boxes(
    left: Vec<Line<'static>>,
    right: Vec<Line<'static>>,
    gap: usize,
) -> Vec<Line<'static>> {
    let left_width = left.first().map(line_width).unwrap_or(0);
    let right_width = right.first().map(line_width).unwrap_or(0);
    let max_lines = left.len().max(right.len());
    let blank_left = Line::from(" ".repeat(left_width));
    let blank_right = Line::from(" ".repeat(right_width));
    let mut out = Vec::new();
    for index in 0..max_lines {
        let left_line = left
            .get(index)
            .cloned()
            .unwrap_or_else(|| blank_left.clone());
        let right_line = right
            .get(index)
            .cloned()
            .unwrap_or_else(|| blank_right.clone());
        let mut spans = left_line.spans;
        spans.push(Span::raw(" ".repeat(gap)));
        spans.extend(right_line.spans);
        out.push(Line::from(spans));
    }
    out
}

fn render_detail_grid(
    rows: Vec<(Vec<Line<'static>>, Option<Vec<Line<'static>>>)>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let row_count = rows.len();
    for (index, (left, right)) in rows.into_iter().enumerate() {
        let row_lines = if let Some(right) = right {
            combine_boxes(left, right, 2)
        } else {
            left
        };
        lines.extend(row_lines);
        if index + 1 != row_count {
            lines.push(Line::from(""));
        }
    }
    lines
}

fn table_border(
    left: char,
    mid: char,
    right: char,
    widths: &[usize],
    style: Style,
) -> Line<'static> {
    let parts = widths
        .iter()
        .map(|width| "─".repeat(width + 2))
        .collect::<Vec<_>>();
    Line::from(Span::styled(
        format!("{}{}{}", left, parts.join(&mid.to_string()), right),
        style,
    ))
}

fn render_wrapped_table(
    title: &str,
    note: Option<&str>,
    headers: &[&str],
    widths: &[usize],
    rows: &[Vec<String>],
    frame_style: Style,
    title_style: Style,
    header_style: Style,
    row_style: Style,
) -> Vec<Line<'static>> {
    let total_inner =
        widths.iter().map(|width| width + 2).sum::<usize>() + widths.len().saturating_sub(1);
    let title_text = truncate_chars(&format!(" {} ", title), total_inner);
    let title_len = title_text.chars().count();
    let mut lines = vec![Line::from(vec![
        Span::styled("┌", frame_style),
        Span::styled(title_text, title_style.add_modifier(Modifier::BOLD)),
        Span::styled(
            "─".repeat(total_inner.saturating_sub(title_len)),
            frame_style,
        ),
        Span::styled("┐", frame_style),
    ])];
    if let Some(note) = note {
        for wrapped in wrap_text(note, total_inner) {
            lines.push(Line::from(vec![
                Span::styled("│", frame_style),
                Span::styled(pad_right(&wrapped, total_inner), header_style),
                Span::styled("│", frame_style),
            ]));
        }
        lines.push(Line::from(Span::styled(
            format!("├{}┤", "─".repeat(total_inner)),
            frame_style,
        )));
    }
    lines.push(table_border('├', '┬', '┤', widths, frame_style));
    let mut header_spans = vec![Span::styled("│", frame_style)];
    for (idx, (header, width)) in headers.iter().zip(widths.iter()).enumerate() {
        header_spans.push(Span::styled(
            format!(" {} ", pad_right(header, *width)),
            header_style.add_modifier(Modifier::BOLD),
        ));
        header_spans.push(Span::styled(
            if idx + 1 == widths.len() {
                "│"
            } else {
                "│"
            },
            frame_style,
        ));
    }
    lines.push(Line::from(header_spans));
    lines.push(table_border('├', '┼', '┤', widths, frame_style));
    if rows.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("│", frame_style),
            Span::styled(pad_right("No rows", total_inner), row_style),
            Span::styled("│", frame_style),
        ]));
    } else {
        for row in rows {
            let wrapped_cells = row
                .iter()
                .zip(widths.iter())
                .map(|(value, width)| wrap_text(value, *width))
                .collect::<Vec<_>>();
            let height = wrapped_cells.iter().map(Vec::len).max().unwrap_or(1);
            for line_index in 0..height {
                let mut spans = vec![Span::styled("│", frame_style)];
                for (idx, (cell_lines, width)) in
                    wrapped_cells.iter().zip(widths.iter()).enumerate()
                {
                    let value = cell_lines.get(line_index).map(String::as_str).unwrap_or("");
                    spans.push(Span::styled(
                        format!(" {} ", pad_right(value, *width)),
                        row_style,
                    ));
                    spans.push(Span::styled(
                        if idx + 1 == widths.len() {
                            "│"
                        } else {
                            "│"
                        },
                        frame_style,
                    ));
                }
                lines.push(Line::from(spans));
            }
            lines.push(table_border('├', '┼', '┤', widths, frame_style));
        }
        lines.pop();
    }
    lines.push(table_border('└', '┴', '┘', widths, frame_style));
    lines
}

fn render_scrollable_detail_modal(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    body_lines: Vec<Line<'static>>,
    footer_hint: &str,
    scroll: usize,
    frame_style: Style,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(2)])
        .split(area);
    let (offset, total_rows, start_line, end_line, visible_rows) =
        scroll_metrics(body_lines.len(), layout[0].height, scroll);
    hit_map.set_page_rows(visible_rows);
    let visible = body_lines
        .iter()
        .skip(offset)
        .take(visible_rows)
        .cloned()
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(visible).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(frame_style)
                .title(Span::styled(
                    title.to_string(),
                    frame_style.add_modifier(Modifier::BOLD),
                )),
        ),
        layout[0],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "{}  Lines {}-{} / {}",
            footer_hint, start_line, end_line, total_rows
        ))
        .style(theme.muted)
        .block(Block::default().borders(Borders::TOP)),
        layout[1],
    );
}

fn render_job_detail_modal(
    frame: &mut Frame,
    detail: &crate::app::JobDetailModal,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let area = centered_rect(86, 84, frame.area());
    frame.render_widget(Clear, area);
    hit_map.push(area, MouseHit::Modal(ModalAction::Ignore));
    if detail.loading {
        render_scrollable_detail_modal(
            frame,
            area,
            "Job Detail",
            vec![Line::from(format!(
                "Loading detail for job {}...",
                detail.job_id
            ))],
            "q Back",
            app.modal_scroll(),
            theme.detail_frame,
            theme,
            hit_map,
        );
        return;
    }

    let body_lines = cached_detail_lines(app.modal_revision(), area.width, |content_width| {
        let detail = match &detail.detail {
            Some(detail) => detail,
            None => {
                return render_detail_grid(vec![(
                    render_kv_box(
                        "Error",
                        &[kv_styled(
                            "Message",
                            detail
                                .error
                                .clone()
                                .unwrap_or_else(|| "Unknown error".to_string()),
                            theme.danger,
                            theme.danger,
                        )],
                        content_width,
                        true,
                        theme.detail_frame,
                        theme.danger,
                        theme.danger,
                    ),
                    None,
                )]);
            }
        };

        let two_col = content_width >= 84;
        let box_width = if two_col {
            (content_width.saturating_sub(2)) / 2
        } else {
            content_width
        };
        let state_text = detail.state.clone().unwrap_or_else(|| "N/A".to_string());
        let state_value_style = state_style(&state_text, theme);
        let basic = render_kv_box(
            "Basic",
            &[
                kv_styled("Job ID", detail.job_id.clone(), theme.accent, theme.accent),
                kv_styled(
                    "Name",
                    detail.name.clone().unwrap_or_else(|| "N/A".to_string()),
                    theme.title,
                    Style::default(),
                ),
                kv_styled(
                    "User",
                    detail.user.clone().unwrap_or_else(|| "N/A".to_string()),
                    theme.accent,
                    theme.accent,
                ),
                kv_styled(
                    "Account",
                    detail.account.clone().unwrap_or_else(|| "N/A".to_string()),
                    theme.muted,
                    Style::default(),
                ),
                kv_styled(
                    "Partition",
                    detail
                        .partition
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.title,
                    theme.title,
                ),
                kv_styled("State", state_text, state_value_style, state_value_style),
            ],
            box_width,
            false,
            theme.detail_frame,
            theme.title,
            theme.accent,
        );
        let resources = render_kv_box(
            "Resources",
            &[
                kv_styled(
                    "Nodes",
                    detail
                        .nodes
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.success,
                    theme.success,
                ),
                kv_styled(
                    "Tasks",
                    detail
                        .n_tasks
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.success,
                    theme.success,
                ),
                kv_styled(
                    "CPUs",
                    detail
                        .cpus
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.success,
                    theme.success,
                ),
                kv_styled(
                    "Memory",
                    detail
                        .memory_mb
                        .map(format_mem_mb)
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.success,
                    theme.success,
                ),
                kv_styled(
                    "GPUs",
                    detail
                        .requested_gpus
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.success,
                    theme.success,
                ),
                kv_styled(
                    "GRES",
                    detail.gres.clone().unwrap_or_else(|| "N/A".to_string()),
                    theme.success,
                    Style::default(),
                ),
            ],
            box_width,
            false,
            theme.detail_frame,
            theme.success,
            theme.success,
        );
        let scheduling = render_kv_box(
            "Scheduling",
            &[
                kv_styled(
                    "Runtime",
                    detail
                        .runtime_raw
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.pending,
                    theme.pending,
                ),
                kv_styled(
                    "Time limit",
                    detail
                        .time_limit_raw
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.pending,
                    theme.pending,
                ),
                kv_styled(
                    "Submit time",
                    detail
                        .submit_time
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.pending,
                    Style::default(),
                ),
                kv_styled(
                    "Start time",
                    detail
                        .start_time
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.pending,
                    Style::default(),
                ),
                kv_styled(
                    "End time",
                    detail.end_time.clone().unwrap_or_else(|| "N/A".to_string()),
                    theme.pending,
                    Style::default(),
                ),
                kv_styled(
                    "Exit code",
                    detail
                        .exit_code
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.pending,
                    Style::default(),
                ),
            ],
            box_width,
            false,
            theme.detail_frame,
            theme.pending,
            theme.pending,
        );
        let placement = render_kv_box(
            "Placement / Reason",
            &[
                kv_styled(
                    "Node list",
                    detail
                        .node_list
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.warning,
                    Style::default(),
                ),
                kv_styled(
                    "Reason",
                    detail.reason.clone().unwrap_or_else(|| "N/A".to_string()),
                    theme.warning,
                    theme.warning,
                ),
                kv_styled(
                    "ReqTRES",
                    detail.req_tres.clone().unwrap_or_else(|| "N/A".to_string()),
                    theme.warning,
                    theme.success,
                ),
                kv_styled(
                    "AllocTRES",
                    detail
                        .alloc_tres
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.warning,
                    theme.success,
                ),
            ],
            box_width,
            false,
            theme.detail_frame,
            theme.warning,
            theme.warning,
        );
        let paths = render_kv_box(
            "Paths",
            &[
                kv_styled(
                    "Workdir",
                    detail.work_dir.clone().unwrap_or_else(|| "N/A".to_string()),
                    theme.accent,
                    Style::default(),
                ),
                kv_styled(
                    "Stdout",
                    detail
                        .stdout_path
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.accent,
                    Style::default(),
                ),
                kv_styled(
                    "Stderr",
                    detail
                        .stderr_path
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                    theme.accent,
                    Style::default(),
                ),
            ],
            box_width,
            false,
            theme.detail_frame,
            theme.accent,
            theme.accent,
        );
        let extra = render_kv_box(
            "Extra",
            &[
                kv_styled(
                    "Command",
                    detail.command.clone().unwrap_or_else(|| "N/A".to_string()),
                    theme.title,
                    Style::default(),
                ),
                kv_styled(
                    "Notes",
                    if detail.source_notes.is_empty() {
                        "none".to_string()
                    } else {
                        detail.source_notes.join("\n")
                    },
                    theme.muted,
                    theme.muted,
                ),
            ],
            box_width,
            true,
            theme.detail_frame,
            theme.muted,
            theme.muted,
        );
        let close_box = render_kv_box(
            "Close",
            &[kv_styled(
                "Mouse / Keyboard",
                "Click outside the panel to close. Keyboard: Esc, b, q, or Enter. Scroll with j/k, arrows, Space, b, g, or G.",
                theme.muted,
                theme.muted,
            )],
            content_width,
            true,
            theme.detail_frame,
            theme.muted,
            theme.muted,
        );

        if two_col {
            render_detail_grid(vec![
                (basic, Some(resources)),
                (scheduling, Some(placement)),
                (paths, Some(extra)),
                (close_box, None),
            ])
        } else {
            render_detail_grid(vec![
                (basic, None),
                (resources, None),
                (scheduling, None),
                (placement, None),
                (paths, None),
                (extra, None),
                (close_box, None),
            ])
        }
    });

    render_scrollable_detail_modal(
        frame,
        area,
        "Job Detail",
        body_lines,
        "j/k move  Space/b page  g/G top/bottom  q Back  Wheel scroll",
        app.modal_scroll(),
        theme.detail_frame,
        theme,
        hit_map,
    );
}

fn render_cancel_confirm_modal(
    frame: &mut Frame,
    preview: &crate::app::CancelPreview,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let area = centered_rect(88, 84, frame.area());
    frame.render_widget(Clear, area);
    hit_map.push(area, MouseHit::Modal(ModalAction::Ignore));
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(2),
            Constraint::Length(3),
        ])
        .split(area);

    let body_lines = cached_detail_lines(app.modal_revision(), layout[0].width, |content_width| {
        let allowed: Vec<_> = preview
            .candidates
            .iter()
            .filter(|candidate| candidate.allowed)
            .collect();
        let denied: Vec<_> = preview
            .candidates
            .iter()
            .filter(|candidate| !candidate.allowed)
            .collect();
        let mut lines = Vec::new();
        lines.extend(render_kv_box(
            "Summary",
            &[
                kv_styled(
                    "Operation",
                    preview.title.clone(),
                    theme.cancel_frame,
                    theme.cancel_frame,
                ),
                kv_styled(
                    "Scope",
                    match preview.scope {
                        crate::app::CancelScope::Single => "single job".to_string(),
                        crate::app::CancelScope::Visible => "visible filtered jobs".to_string(),
                    },
                    theme.warning,
                    Style::default(),
                ),
                kv_styled(
                    "Reviewed jobs",
                    preview.candidates.len().to_string(),
                    theme.warning,
                    Style::default(),
                ),
                kv_styled(
                    "Eligible jobs",
                    allowed.len().to_string(),
                    theme.success,
                    theme.success,
                ),
                kv_styled(
                    "Ineligible jobs",
                    denied.len().to_string(),
                    theme.muted,
                    theme.muted,
                ),
                kv_styled(
                    "Warning",
                    "Dangerous action. Confirming starts scancel immediately for eligible jobs.",
                    theme.danger,
                    theme.danger,
                ),
            ],
            content_width,
            false,
            theme.cancel_frame,
            theme.cancel_frame,
            theme.warning,
        ));
        lines.push(Line::from(""));

        let table_widths = if content_width >= 96 {
            vec![
                8,
                16,
                10,
                10,
                8,
                7,
                content_width.saturating_sub(8 + 16 + 10 + 10 + 8 + 7 + 21),
            ]
        } else {
            vec![
                8,
                12,
                8,
                8,
                8,
                7,
                content_width.saturating_sub(8 + 12 + 8 + 8 + 8 + 7 + 19),
            ]
        };

        let eligible_rows = allowed
            .iter()
            .map(|candidate| {
                vec![
                    candidate.job_id.clone(),
                    candidate.name.clone(),
                    candidate.user.clone(),
                    candidate.partition.clone(),
                    candidate.state.clone(),
                    candidate.nodes.to_string(),
                    format!(
                        "ReqTRES: {}\nAllocTRES: {}\nPlacement: {}\nEligible: yes",
                        candidate
                            .req_tres
                            .clone()
                            .unwrap_or_else(|| "N/A".to_string()),
                        candidate
                            .alloc_tres
                            .clone()
                            .unwrap_or_else(|| "N/A".to_string()),
                        candidate.placement_or_reason,
                    ),
                ]
            })
            .collect::<Vec<_>>();
        lines.extend(
            render_wrapped_table(
                if preview.scope == crate::app::CancelScope::Single {
                    "Target Job"
                } else {
                    "Eligible Jobs"
                },
                Some("These jobs will be cancelled if you confirm."),
                &[
                    "JobID",
                    "Name",
                    "User",
                    "Partition",
                    "State",
                    "Nodes",
                    "Details",
                ],
                &table_widths,
                &eligible_rows,
                theme.cancel_frame,
                theme.warning,
                theme.warning,
                theme.warning,
            )
            .into_iter()
            .map(Line::from),
        );
        if !denied.is_empty() {
            lines.push(Line::from(""));
            let denied_rows = denied
                .iter()
                .map(|candidate| {
                    vec![
                        candidate.job_id.clone(),
                        candidate.name.clone(),
                        candidate.user.clone(),
                        candidate.partition.clone(),
                        candidate.state.clone(),
                        candidate.nodes.to_string(),
                        format!(
                            "Skip reason: {}\nPlacement: {}",
                            candidate
                                .reason
                                .clone()
                                .unwrap_or_else(|| "Not allowed".to_string()),
                            candidate.placement_or_reason,
                        ),
                    ]
                })
                .collect::<Vec<_>>();
            lines.extend(
                render_wrapped_table(
                    "Ineligible / Skipped Jobs",
                    Some("These jobs will not be cancelled."),
                    &[
                        "JobID",
                        "Name",
                        "User",
                        "Partition",
                        "State",
                        "Nodes",
                        "Why not",
                    ],
                    &table_widths,
                    &denied_rows,
                    theme.cancel_frame,
                    theme.muted,
                    theme.muted,
                    theme.muted,
                )
                .into_iter()
                .map(Line::from),
            );
        }
        lines.push(Line::from(""));
        lines.extend(render_kv_box(
            "Action",
            &[
                kv_styled(
                    "Will run",
                    if preview.scope == crate::app::CancelScope::Single {
                        "scancel for the selected job".to_string()
                    } else {
                        "scancel for every eligible job in this preview".to_string()
                    },
                    theme.cancel_frame,
                    theme.warning,
                ),
                kv_styled(
                    "Affected jobs",
                    preview.allowed_count().to_string(),
                    theme.success,
                    theme.success,
                ),
                kv_styled(
                    "Skipped jobs",
                    preview
                        .candidates
                        .len()
                        .saturating_sub(preview.allowed_count())
                        .to_string(),
                    theme.muted,
                    theme.muted,
                ),
            ],
            content_width,
            false,
            theme.cancel_frame,
            theme.warning,
            theme.warning,
        ));
        lines
    });

    let (offset, total_rows, start_line, end_line, visible_rows) =
        scroll_metrics(body_lines.len(), layout[0].height, app.modal_scroll());
    hit_map.set_page_rows(visible_rows);
    let visible = body_lines
        .iter()
        .skip(offset)
        .take(visible_rows)
        .cloned()
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(visible).block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    "Confirm Cancel",
                    theme.cancel_frame.add_modifier(Modifier::BOLD),
                ))
                .border_style(theme.cancel_frame),
        ),
        layout[0],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "j/k move  Space/b page  g/G top/bottom  Enter/y confirm  q/Esc/n back  Lines {}-{} / {}",
            start_line, end_line, total_rows
        ))
        .style(theme.muted)
        .block(Block::default().borders(Borders::TOP)),
        layout[1],
    );

    let buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22),
            Constraint::Length(22),
            Constraint::Min(1),
        ])
        .split(layout[2]);
    let confirm_rect = buttons[0];
    let cancel_rect = buttons[1];
    frame.render_widget(
        Paragraph::new("Confirm [Enter / y]")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(theme.cancel_frame)
                    .title(Span::styled(
                        "Confirm",
                        theme.cancel_frame.add_modifier(Modifier::BOLD),
                    )),
            ),
        confirm_rect,
    );
    frame.render_widget(
        Paragraph::new("Back [q / Esc / n]")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(Span::styled(
                "Back",
                theme.muted.add_modifier(Modifier::BOLD),
            ))),
        cancel_rect,
    );
    hit_map.push(confirm_rect, MouseHit::Modal(ModalAction::Confirm));
    hit_map.push(cancel_rect, MouseHit::Modal(ModalAction::Cancel));
}

fn render_cancel_result_modal(
    frame: &mut Frame,
    report: &crate::app::CancelReport,
    app: &AppState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let area = centered_rect(84, 82, frame.area());
    frame.render_widget(Clear, area);
    hit_map.push(area, MouseHit::Modal(ModalAction::Ignore));
    let failed_count = report
        .results
        .iter()
        .filter(|result| !result.success && !is_skipped_message(&result.message))
        .count();
    let skipped_count = report
        .results
        .iter()
        .filter(|result| !result.success && is_skipped_message(&result.message))
        .count();
    let result_frame = if failed_count > 0 {
        theme.result_failure
    } else if skipped_count > 0 {
        theme.muted
    } else {
        theme.result_success
    };

    let body_lines = cached_detail_lines(app.modal_revision(), area.width, |content_width| {
        let succeeded: Vec<_> = report
            .results
            .iter()
            .filter(|result| result.success)
            .collect();
        let skipped: Vec<_> = report
            .results
            .iter()
            .filter(|result| !result.success && is_skipped_message(&result.message))
            .collect();
        let failed: Vec<_> = report
            .results
            .iter()
            .filter(|result| !result.success && !is_skipped_message(&result.message))
            .collect();
        let mut lines = Vec::new();
        let result_frame = if !failed.is_empty() {
            theme.result_failure
        } else if !skipped.is_empty() {
            theme.muted
        } else {
            theme.result_success
        };
        lines.extend(render_kv_box(
            "Result Summary",
            &[
                kv_styled(
                    "Scope",
                    match report.scope {
                        crate::app::CancelScope::Single => "single job".to_string(),
                        crate::app::CancelScope::Visible => "visible filtered jobs".to_string(),
                    },
                    result_frame,
                    Style::default(),
                ),
                kv_styled(
                    "Total requests",
                    report.results.len().to_string(),
                    theme.title,
                    Style::default(),
                ),
                kv_styled(
                    "Succeeded",
                    succeeded.len().to_string(),
                    theme.result_success,
                    theme.result_success,
                ),
                kv_styled(
                    "Failed",
                    failed.len().to_string(),
                    theme.result_failure,
                    theme.result_failure,
                ),
                kv_styled(
                    "Skipped",
                    skipped.len().to_string(),
                    theme.muted,
                    theme.muted,
                ),
            ],
            content_width,
            false,
            result_frame,
            result_frame,
            result_frame,
        ));
        lines.push(Line::from(""));
        let widths = vec![8, content_width.saturating_sub(8 + 5)];
        if !succeeded.is_empty() {
            let rows = succeeded
                .iter()
                .map(|outcome| vec![outcome.job_id.clone(), outcome.message.clone()])
                .collect::<Vec<_>>();
            lines.extend(
                render_wrapped_table(
                    "Succeeded",
                    Some("These jobs were cancelled successfully."),
                    &["JobID", "Message"],
                    &widths,
                    &rows,
                    theme.result_success,
                    theme.result_success,
                    theme.result_success,
                    theme.result_success,
                )
                .into_iter()
                .map(Line::from),
            );
            lines.push(Line::from(""));
        }
        if !failed.is_empty() {
            let rows = failed
                .iter()
                .map(|outcome| vec![outcome.job_id.clone(), outcome.message.clone()])
                .collect::<Vec<_>>();
            lines.extend(
                render_wrapped_table(
                    "Failed",
                    Some("These jobs were not cancelled because scancel failed."),
                    &["JobID", "Message"],
                    &widths,
                    &rows,
                    theme.result_failure,
                    theme.result_failure,
                    theme.result_failure,
                    theme.result_failure,
                )
                .into_iter()
                .map(Line::from),
            );
            lines.push(Line::from(""));
        }
        if !skipped.is_empty() {
            let rows = skipped
                .iter()
                .map(|outcome| vec![outcome.job_id.clone(), outcome.message.clone()])
                .collect::<Vec<_>>();
            lines.extend(
                render_wrapped_table(
                    "Skipped",
                    Some("These jobs were intentionally left untouched."),
                    &["JobID", "Message"],
                    &widths,
                    &rows,
                    theme.muted,
                    theme.muted,
                    theme.muted,
                    theme.muted,
                )
                .into_iter()
                .map(Line::from),
            );
        }
        lines
    });

    render_scrollable_detail_modal(
        frame,
        area,
        "Cancel Result",
        body_lines,
        "j/k move  Space/b page  g/G top/bottom  q Back  Wheel scroll",
        app.modal_scroll(),
        result_frame,
        theme,
        hit_map,
    );
}

fn register_table_rows(hit_map: &mut UiHitMap, area: Rect, row_count: usize, kind: RowKind) {
    let body_y = area.y.saturating_add(2);
    let width = area.width.saturating_sub(2);
    let visible_rows = area.height.saturating_sub(3) as usize;
    hit_map.set_page_rows(visible_rows.max(1));
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

fn adaptive_segment_width(area_width: u16, preferred: usize, minimum: usize) -> usize {
    let usable = area_width.saturating_sub(6).max(minimum as u16) as usize;
    preferred.min(usable).max(minimum)
}

fn adaptive_text_width(
    values: &[String],
    area_width: u16,
    minimum: usize,
    maximum: usize,
    fallback: usize,
) -> u16 {
    let preferred = values
        .iter()
        .map(|value| value.chars().count())
        .max()
        .unwrap_or(fallback)
        .clamp(minimum, maximum);
    adaptive_segment_width(area_width, preferred, minimum) as u16
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

fn wide_table_title(base: &str, hidden_left: usize, hidden_right: usize) -> String {
    if hidden_left == 0 && hidden_right == 0 {
        base.to_string()
    } else {
        format!(
            "{base}  ← {} hidden  {} hidden →",
            hidden_left, hidden_right
        )
    }
}

fn styled_stacked_bar_line(
    first: u64,
    second: u64,
    capacity: Option<u64>,
    width: usize,
    first_label: &str,
    second_label: &str,
    first_style: Style,
    second_style: Style,
    theme: &Theme,
) -> Line<'static> {
    let capacity = capacity.unwrap_or((first + second).max(1)).max(1);
    let first_width = scaled_bar_width(first, capacity, width);
    let second_width =
        scaled_bar_width(second, capacity, width).min(width.saturating_sub(first_width));
    let blank = width.saturating_sub(first_width + second_width);
    Line::from(vec![
        Span::styled("[".to_string(), theme.muted),
        Span::styled("█".repeat(first_width), first_style),
        Span::styled("█".repeat(second_width), second_style),
        Span::styled("·".repeat(blank), theme.muted),
        Span::styled("]".to_string(), theme.muted),
        Span::raw(format!(
            "  {first_label}: {first}  {second_label}: {second}"
        )),
    ])
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

fn build_node_state_lines(partition: &PartitionOverview, theme: &Theme) -> Vec<Line<'static>> {
    let total = partition.total_nodes.max(1) as u64;
    let mut lines = Vec::new();
    for key in ["idle", "mix", "alloc", "drain", "down"] {
        let count = *partition.node_state_counts.get(key).unwrap_or(&0) as u64;
        let state_style = match key {
            "idle" => theme.success,
            "mix" | "alloc" => theme.running,
            "drain" => theme.warning,
            "down" => theme.danger,
            _ => theme.muted,
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{:<5} ", key), state_style),
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

fn footer_action_label(action: FooterAction, app: &AppState) -> &'static str {
    match action {
        FooterAction::BackOverview => match app.current_page() {
            Page::PartitionDetail | Page::NodeDetail | Page::UserJobs => "q Back",
            _ => "← Overview",
        },
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
