use std::cmp::Ordering;
use std::collections::{BTreeMap, VecDeque};
use std::io::stdout;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Local;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;

use crate::cli::{HistoryWindow, ResolvedCli};
use crate::collector::{
    Collector,
    command::{CancelFlag, CommandCapture, CommandStatus},
};
use crate::model::{
    ClusterSnapshot, DebugDump, HistoryRecord, JobDetail, JobRecord, MetricMode, NodeDetail,
    NodeRecord, PartitionOverview, UserUsage, aggregate_users, build_snapshot, parse_history,
    parse_history_detail, parse_jobs, parse_scontrol_job, parse_scontrol_node,
};
use crate::ui::{ThemePalette, render};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Overview,
    MyJobs,
    Users,
    AllJobs,
    PartitionDetail,
    NodeDetail,
}

impl Page {
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::MyJobs => "My Jobs",
            Self::Users => "Users",
            Self::AllJobs => "All Jobs",
            Self::PartitionDetail => "Partition Detail",
            Self::NodeDetail => "Node Detail",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputMode {
    Normal,
    Search,
    NodeWhereFilter,
    NodeWhyFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefreshCommand {
    RefreshNow,
    Quit,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowKind {
    Overview,
    MyJobs,
    Users,
    AllJobs,
    PartitionJobs,
    PartitionNodes,
    NodeJobs,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FooterAction {
    BackOverview,
    Refresh,
    Help,
    ToggleMine,
    OpenDetail,
    OpenNode,
    CancelJob,
    BulkCancel,
    ClearFilters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalAction {
    Confirm,
    Cancel,
    Close,
    Ignore,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TrendSeries {
    running: VecDeque<u64>,
    pending: VecDeque<u64>,
}

impl TrendSeries {
    fn push(&mut self, running: u64, pending: u64, max_len: usize) {
        self.running.push_back(running);
        self.pending.push_back(pending);
        while self.running.len() > max_len {
            self.running.pop_front();
        }
        while self.pending.len() > max_len {
            self.pending.pop_front();
        }
    }

    pub(crate) fn running_points(&self) -> Vec<u64> {
        self.running.iter().copied().collect()
    }

    pub(crate) fn pending_points(&self) -> Vec<u64> {
        self.pending.iter().copied().collect()
    }

    pub(crate) fn latest_running(&self) -> u64 {
        self.running.back().copied().unwrap_or(0)
    }

    pub(crate) fn latest_pending(&self) -> u64 {
        self.pending.back().copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MouseHit {
    Tab(Page),
    Row(RowKind, usize),
    OverviewHeader(OverviewColumn),
    JobHeader(JobColumn),
    UserHeader(UserColumn),
    Footer(FooterAction),
    Modal(ModalAction),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct UiHitMap {
    hits: Vec<(Rect, MouseHit)>,
}

impl UiHitMap {
    pub fn push(&mut self, rect: Rect, hit: MouseHit) {
        self.hits.push((rect, hit));
    }

    pub fn hit_test(&self, column: u16, row: u16) -> Option<MouseHit> {
        self.hits.iter().rev().find_map(|(rect, hit)| {
            if column >= rect.x
                && column < rect.x.saturating_add(rect.width)
                && row >= rect.y
                && row < rect.y.saturating_add(rect.height)
            {
                Some(hit.clone())
            } else {
                None
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobFilter {
    All,
    Active,
    Running,
    Pending,
}

impl JobFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::Active,
            Self::Active => Self::Running,
            Self::Running => Self::Pending,
            Self::Pending => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Active => "active",
            Self::Running => "running",
            Self::Pending => "pending",
        }
    }

    fn matches(self, job: &JobRecord) -> bool {
        match self {
            Self::All => true,
            Self::Active => job.active,
            Self::Running => job.running,
            Self::Pending => job.pending,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryFilter {
    All,
    Completed,
    Failed,
    Cancelled,
    Timeout,
    Running,
}

#[allow(dead_code)]
impl HistoryFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::Completed,
            Self::Completed => Self::Failed,
            Self::Failed => Self::Cancelled,
            Self::Cancelled => Self::Timeout,
            Self::Timeout => Self::Running,
            Self::Running => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::Running => "running",
        }
    }

    fn matches(self, history: &HistoryRecord) -> bool {
        let state = history.state.to_ascii_uppercase();
        match self {
            Self::All => true,
            Self::Completed => state.contains("COMPLETED"),
            Self::Failed => state.contains("FAILED") || state.contains("NODE_FAIL"),
            Self::Cancelled => state.contains("CANCELLED"),
            Self::Timeout => state.contains("TIMEOUT"),
            Self::Running => state.contains("RUNNING"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverviewColumn {
    Partition,
    Nodes,
    Pressure,
    MineRunning,
    MinePending,
    OthersRunning,
    OthersPending,
    TotalJobs,
}

impl OverviewColumn {
    fn next(self) -> Self {
        match self {
            Self::Partition => Self::Nodes,
            Self::Nodes => Self::Pressure,
            Self::Pressure => Self::MineRunning,
            Self::MineRunning => Self::MinePending,
            Self::MinePending => Self::OthersRunning,
            Self::OthersRunning => Self::OthersPending,
            Self::OthersPending => Self::TotalJobs,
            Self::TotalJobs => Self::Partition,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Partition => "Partition",
            Self::Nodes => "Nodes",
            Self::Pressure => "Pressure",
            Self::MineRunning => "Mine running",
            Self::MinePending => "Mine pending",
            Self::OthersRunning => "Others running",
            Self::OthersPending => "Others pending",
            Self::TotalJobs => "Total jobs",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobColumn {
    JobId,
    User,
    Partition,
    State,
    Name,
    Runtime,
    TimeLimit,
    Nodes,
    Cpus,
    Priority,
    WhereWhy,
}

impl JobColumn {
    fn next(self) -> Self {
        match self {
            Self::JobId => Self::User,
            Self::User => Self::Partition,
            Self::Partition => Self::State,
            Self::State => Self::Name,
            Self::Name => Self::Runtime,
            Self::Runtime => Self::TimeLimit,
            Self::TimeLimit => Self::Nodes,
            Self::Nodes => Self::Cpus,
            Self::Cpus => Self::Priority,
            Self::Priority => Self::WhereWhy,
            Self::WhereWhy => Self::JobId,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::JobId => "Job ID",
            Self::User => "User",
            Self::Partition => "Partition",
            Self::State => "State",
            Self::Name => "Name",
            Self::Runtime => "Runtime",
            Self::TimeLimit => "Time limit",
            Self::Nodes => "Nodes",
            Self::Cpus => "CPUs",
            Self::Priority => "Priority",
            Self::WhereWhy => "Placement / reason",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserColumn {
    User,
    RunningJobs,
    PendingJobs,
    TotalJobs,
    ResourceFootprint,
    Partitions,
}

impl UserColumn {
    fn next(self) -> Self {
        match self {
            Self::User => Self::RunningJobs,
            Self::RunningJobs => Self::PendingJobs,
            Self::PendingJobs => Self::TotalJobs,
            Self::TotalJobs => Self::ResourceFootprint,
            Self::ResourceFootprint => Self::Partitions,
            Self::Partitions => Self::User,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::User => "User",
            Self::RunningJobs => "Running jobs",
            Self::PendingJobs => "Pending jobs",
            Self::TotalJobs => "Total jobs",
            Self::ResourceFootprint => "Resource footprint",
            Self::Partitions => "Top partitions",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortDirection {
    Desc,
    Asc,
}

impl SortDirection {
    fn toggle(self) -> Self {
        match self {
            Self::Desc => Self::Asc,
            Self::Asc => Self::Desc,
        }
    }

    pub(crate) fn arrow(self) -> &'static str {
        match self {
            Self::Desc => "↓",
            Self::Asc => "↑",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OverviewSortState {
    pub(crate) column: OverviewColumn,
    pub(crate) direction: SortDirection,
}

impl OverviewSortState {
    fn label(self) -> String {
        format!("{} {}", self.column.label(), self.direction.arrow())
    }

    fn cycle(self) -> Self {
        Self {
            column: self.column.next(),
            direction: SortDirection::Desc,
        }
    }

    fn choose(&mut self, column: OverviewColumn) {
        if self.column == column {
            self.direction = self.direction.toggle();
        } else {
            self.column = column;
            self.direction = SortDirection::Desc;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JobSortState {
    pub(crate) column: JobColumn,
    pub(crate) direction: SortDirection,
}

impl JobSortState {
    fn label(self) -> String {
        format!("{} {}", self.column.label(), self.direction.arrow())
    }

    fn cycle(self) -> Self {
        Self {
            column: self.column.next(),
            direction: SortDirection::Desc,
        }
    }

    fn choose(&mut self, column: JobColumn) {
        if self.column == column {
            self.direction = self.direction.toggle();
        } else {
            self.column = column;
            self.direction = SortDirection::Desc;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UserSortState {
    pub(crate) column: UserColumn,
    pub(crate) direction: SortDirection,
}

impl UserSortState {
    fn label(self) -> String {
        format!("{} {}", self.column.label(), self.direction.arrow())
    }

    fn cycle(self) -> Self {
        Self {
            column: self.column.next(),
            direction: SortDirection::Desc,
        }
    }

    fn choose(&mut self, column: UserColumn) {
        if self.column == column {
            self.direction = self.direction.toggle();
        } else {
            self.column = column;
            self.direction = SortDirection::Desc;
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistorySort {
    EndTime,
    Elapsed,
    User,
    Partition,
}

#[allow(dead_code)]
impl HistorySort {
    fn next(self) -> Self {
        match self {
            Self::EndTime => Self::Elapsed,
            Self::Elapsed => Self::User,
            Self::User => Self::Partition,
            Self::Partition => Self::EndTime,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::EndTime => "end",
            Self::Elapsed => "elapsed",
            Self::User => "user",
            Self::Partition => "partition",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct HistoryViewState {
    pub window: HistoryWindow,
    pub sort: HistorySort,
    pub state_filter: HistoryFilter,
    pub records: Vec<HistoryRecord>,
    pub notes: Vec<String>,
    pub last_loaded_at: Option<String>,
    pub loading: bool,
    pub available: bool,
}

impl HistoryViewState {
    fn new(window: HistoryWindow) -> Self {
        Self {
            window,
            sort: HistorySort::EndTime,
            state_filter: HistoryFilter::All,
            records: Vec::new(),
            notes: Vec::new(),
            last_loaded_at: None,
            loading: true,
            available: true,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct JobDetailModal {
    pub job_id: String,
    pub loading: bool,
    pub detail: Option<JobDetail>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CancelCandidate {
    pub job_id: String,
    pub name: String,
    pub user: String,
    pub partition: String,
    pub state: String,
    pub allowed: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancelScope {
    Single,
    Visible,
}

#[derive(Debug, Clone)]
pub(crate) struct CancelPreview {
    pub scope: CancelScope,
    pub title: String,
    pub candidates: Vec<CancelCandidate>,
}

impl CancelPreview {
    pub(crate) fn allowed_job_ids(&self) -> Vec<String> {
        self.candidates
            .iter()
            .filter(|candidate| candidate.allowed)
            .map(|candidate| candidate.job_id.clone())
            .collect()
    }

    pub(crate) fn allowed_count(&self) -> usize {
        self.candidates
            .iter()
            .filter(|candidate| candidate.allowed)
            .count()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CancelOutcome {
    pub job_id: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CancelReport {
    pub scope: CancelScope,
    pub results: Vec<CancelOutcome>,
}

impl CancelReport {
    pub(crate) fn succeeded(&self) -> usize {
        self.results.iter().filter(|result| result.success).count()
    }

    pub(crate) fn failed(&self) -> usize {
        self.results.len().saturating_sub(self.succeeded())
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Modal {
    Help,
    JobDetail(JobDetailModal),
    ConfirmCancel(CancelPreview),
    CancelResult(CancelReport),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
enum AsyncEvent {
    HistoryLoaded {
        window: HistoryWindow,
        records: Vec<HistoryRecord>,
        notes: Vec<String>,
        available: bool,
    },
    JobDetailLoaded {
        job_id: String,
        detail: Option<JobDetail>,
        error: Option<String>,
    },
    NodeLoaded {
        node_name: String,
        detail: Option<NodeDetail>,
        jobs: Vec<JobRecord>,
        notes: Vec<String>,
    },
    CancelFinished(CancelReport),
}

#[derive(Debug, Clone)]
pub(crate) struct NodeDetailState {
    pub node_name: String,
    pub partition: String,
    pub selected_job: usize,
    pub user_filter_index: usize,
    pub state_filter_index: usize,
    pub where_filter: String,
    pub where_draft: String,
    pub why_filter: String,
    pub why_draft: String,
    pub detail: Option<NodeDetail>,
    pub jobs: Vec<JobRecord>,
    pub notes: Vec<String>,
    pub loading: bool,
    pub last_loaded_at: Option<String>,
}

impl NodeDetailState {
    fn new(node_name: String, partition: String) -> Self {
        Self {
            node_name,
            partition,
            selected_job: 0,
            user_filter_index: 0,
            state_filter_index: 0,
            where_filter: String::new(),
            where_draft: String::new(),
            why_filter: String::new(),
            why_draft: String::new(),
            detail: None,
            jobs: Vec::new(),
            notes: Vec::new(),
            loading: true,
            last_loaded_at: None,
        }
    }
}

pub(crate) struct AppState {
    pub settings: ResolvedCli,
    pub snapshot: Option<ClusterSnapshot>,
    pub input_mode: InputMode,
    pub search_query: String,
    pub search_draft: String,
    pub metric_mode: MetricMode,
    pub show_only_mine: bool,
    pub pinned_partition: Option<String>,
    pub modal: Option<Modal>,
    pub hit_map: UiHitMap,
    pub history: HistoryViewState,
    pub selected_overview: usize,
    pub selected_my_jobs: usize,
    pub selected_users: usize,
    pub selected_all_jobs: usize,
    pub selected_partition_jobs: usize,
    pub selected_partition_node: usize,
    pub selected_history: usize,
    current_main_index: usize,
    detail_partition: Option<String>,
    node_detail: Option<NodeDetailState>,
    pub job_horizontal_offset: usize,
    pub(crate) overview_sort: OverviewSortState,
    pub(crate) job_sort: JobSortState,
    pub(crate) user_sort: UserSortState,
    job_filter: JobFilter,
    cluster_trend: TrendSeries,
    my_trend: TrendSeries,
    partition_trends: BTreeMap<String, TrendSeries>,
    refresh_tx: Sender<RefreshCommand>,
    refresh_rx: Receiver<ClusterSnapshot>,
    async_tx: Sender<AsyncEvent>,
    async_rx: Receiver<AsyncEvent>,
    shutdown: CancelFlag,
    quit: bool,
    last_click: Option<(MouseHit, Instant)>,
    status_message: Option<String>,
}

impl AppState {
    fn main_pages(all_jobs_enabled: bool) -> Vec<Page> {
        let mut pages = vec![Page::Overview, Page::MyJobs, Page::Users];
        if all_jobs_enabled {
            pages.push(Page::AllJobs);
        }
        pages
    }

    fn new(
        settings: ResolvedCli,
        refresh_tx: Sender<RefreshCommand>,
        refresh_rx: Receiver<ClusterSnapshot>,
        async_tx: Sender<AsyncEvent>,
        async_rx: Receiver<AsyncEvent>,
        shutdown: CancelFlag,
    ) -> Self {
        let initial_pages = Self::main_pages(settings.all_jobs_enabled);
        let current_main_index = if settings.start_in_all_jobs && settings.all_jobs_enabled {
            initial_pages
                .iter()
                .position(|page| *page == Page::AllJobs)
                .unwrap_or(0)
        } else {
            0
        };
        Self {
            history: HistoryViewState::new(settings.history_window),
            settings,
            snapshot: None,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            search_draft: String::new(),
            metric_mode: MetricMode::Nodes,
            show_only_mine: false,
            pinned_partition: None,
            modal: None,
            hit_map: UiHitMap::default(),
            current_main_index,
            detail_partition: None,
            node_detail: None,
            job_horizontal_offset: 0,
            overview_sort: OverviewSortState {
                column: OverviewColumn::Pressure,
                direction: SortDirection::Desc,
            },
            job_sort: JobSortState {
                column: JobColumn::Runtime,
                direction: SortDirection::Desc,
            },
            user_sort: UserSortState {
                column: UserColumn::RunningJobs,
                direction: SortDirection::Desc,
            },
            job_filter: JobFilter::Active,
            cluster_trend: TrendSeries::default(),
            my_trend: TrendSeries::default(),
            partition_trends: BTreeMap::new(),
            selected_overview: 0,
            selected_my_jobs: 0,
            selected_users: 0,
            selected_all_jobs: 0,
            selected_partition_jobs: 0,
            selected_partition_node: 0,
            selected_history: 0,
            refresh_tx,
            refresh_rx,
            async_tx,
            async_rx,
            shutdown,
            quit: false,
            last_click: None,
            status_message: None,
        }
    }

    pub(crate) fn set_hit_map(&mut self, hit_map: UiHitMap) {
        self.hit_map = hit_map;
    }

    pub(crate) fn current_page(&self) -> Page {
        if self.node_detail.is_some() {
            Page::NodeDetail
        } else if self.detail_partition.is_some() {
            Page::PartitionDetail
        } else {
            self.page_tabs()[self.current_main_index]
        }
    }

    pub(crate) fn page_tabs(&self) -> Vec<Page> {
        Self::main_pages(self.settings.all_jobs_enabled)
    }

    pub(crate) fn current_main_page_index(&self) -> usize {
        self.current_main_index
    }

    pub(crate) fn sort_label(&self) -> String {
        match self.current_page() {
            Page::Overview => self.overview_sort.label(),
            Page::MyJobs | Page::AllJobs | Page::PartitionDetail | Page::NodeDetail => {
                self.job_sort.label()
            }
            Page::Users => self.user_sort.label(),
        }
    }

    pub(crate) fn filter_label(&self) -> String {
        match self.current_page() {
            Page::Users => {
                if self.show_only_mine {
                    "Showing only the current user".to_string()
                } else {
                    "Showing all users with active jobs".to_string()
                }
            }
            Page::NodeDetail => {
                if let Some(node) = &self.node_detail {
                    format!(
                        "User: {} | State: {}",
                        self.current_node_user_filter(node),
                        self.current_node_state_filter(node)
                    )
                } else {
                    "All".to_string()
                }
            }
            _ => self.job_filter.label().to_string(),
        }
    }

    pub(crate) fn now_string(&self) -> String {
        Local::now().format("%T").to_string()
    }

    pub(crate) fn is_stale(&self) -> bool {
        self.snapshot
            .as_ref()
            .map(|snapshot| {
                (Local::now().naive_local()
                    - snapshot.collected_at.with_timezone(&Local).naive_local())
                .num_seconds()
                    > (self.settings.interval.ceil() as i64 * 3).max(5)
            })
            .unwrap_or(false)
    }

    pub(crate) fn status_badge<'a>(
        &self,
        theme: &'a ThemePalette,
    ) -> (&'static str, ratatui::style::Style) {
        match self.snapshot.as_ref() {
            None => ("loading", theme.muted),
            Some(snapshot)
                if !snapshot.source_health.nodes_ok || !snapshot.source_health.jobs_ok =>
            {
                ("error", theme.danger)
            }
            Some(snapshot) if snapshot.degraded => ("partial", theme.warning),
            Some(_) => ("ok", theme.success),
        }
    }

    pub(crate) fn visible_partitions(&self) -> Vec<&PartitionOverview> {
        let mut items: Vec<&PartitionOverview> = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.partitions.iter().collect())
            .unwrap_or_default();

        let query = self.active_global_query().to_ascii_lowercase();
        items.retain(|partition| {
            if let Some(pin) = &self.pinned_partition
                && partition.name != *pin
            {
                return false;
            }
            if query.is_empty() {
                true
            } else {
                partition.name.to_ascii_lowercase().contains(&query)
                    || partition.state.to_ascii_lowercase().contains(&query)
                    || partition
                        .node_state_summary()
                        .to_ascii_lowercase()
                        .contains(&query)
            }
        });

        items.sort_by(|left, right| self.compare_partitions(left, right));
        items
    }

    pub(crate) fn visible_my_jobs(&self) -> Vec<&JobRecord> {
        self.collect_jobs(true, false)
    }

    pub(crate) fn visible_all_jobs(&self) -> Vec<&JobRecord> {
        self.collect_jobs(false, self.show_only_mine)
    }

    pub(crate) fn visible_users(&self) -> Vec<UserUsage> {
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };
        let mut users = aggregate_users(&snapshot.jobs, &self.settings.user);
        let query = self.active_global_query().to_ascii_lowercase();
        users.retain(|usage| {
            if self.show_only_mine && !usage.is_current_user {
                return false;
            }
            if let Some(pin) = &self.pinned_partition
                && !usage.partitions.contains_key(pin)
            {
                return false;
            }
            if query.is_empty() {
                true
            } else {
                usage.user.to_ascii_lowercase().contains(&query)
                    || usage
                        .top_partitions_summary(3)
                        .to_ascii_lowercase()
                        .contains(&query)
            }
        });
        users.sort_by(|left, right| self.compare_users(left, right));
        users
    }

    pub(crate) fn selected_user_usage(&self) -> Option<UserUsage> {
        self.visible_users().get(self.selected_users).cloned()
    }

    pub(crate) fn visible_selected_user_jobs(&self) -> Vec<&JobRecord> {
        let Some(selected_user) = self.selected_user_usage().map(|usage| usage.user) else {
            return Vec::new();
        };
        let mut jobs = self.collect_jobs(false, false);
        jobs.retain(|job| job.user == selected_user);
        jobs
    }

    pub(crate) fn visible_partition_jobs(&self, partition: &str) -> Vec<&JobRecord> {
        let mut jobs = self.collect_jobs(false, self.show_only_mine);
        jobs.retain(|job| job.partitions.iter().any(|value| value == partition));
        jobs
    }

    pub(crate) fn visible_partition_nodes(&self, partition: &str) -> Vec<&NodeRecord> {
        let mut nodes: Vec<&NodeRecord> = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .nodes
                    .iter()
                    .filter(|node| node.partition == partition)
                    .collect()
            })
            .unwrap_or_default();
        nodes.sort_by(|left, right| left.node_name.cmp(&right.node_name));
        nodes
    }

    pub(crate) fn visible_history(&self) -> Vec<&HistoryRecord> {
        let mut rows: Vec<&HistoryRecord> = self.history.records.iter().collect();
        let query = self.active_global_query().to_ascii_lowercase();
        rows.retain(|history| {
            if self.show_only_mine && !history.is_mine {
                return false;
            }
            if let Some(pin) = &self.pinned_partition
                && history.partition.as_deref() != Some(pin.as_str())
            {
                return false;
            }
            if !self.history.state_filter.matches(history) {
                return false;
            }
            if query.is_empty() {
                return true;
            }
            [
                history.job_id.as_str(),
                history.user.as_str(),
                history.name.as_str(),
                history.state.as_str(),
                history.partition.as_deref().unwrap_or(""),
            ]
            .into_iter()
            .any(|value| value.to_ascii_lowercase().contains(&query))
        });
        rows.sort_by(|left, right| self.compare_history(left, right));
        rows
    }

    pub(crate) fn visible_node_jobs(&self) -> Vec<&JobRecord> {
        let Some(node) = &self.node_detail else {
            return Vec::new();
        };

        let query = self.active_global_query().to_ascii_lowercase();
        let where_filter = self.active_node_where_filter().to_ascii_lowercase();
        let why_filter = self.active_node_why_filter().to_ascii_lowercase();
        let user_filter = self.current_node_user_filter(node);
        let state_filter = self.current_node_state_filter(node);

        let mut rows: Vec<&JobRecord> = node.jobs.iter().collect();
        rows.retain(|job| {
            if self.show_only_mine && !job.is_mine {
                return false;
            }
            if user_filter != "All users" && job.user != user_filter {
                return false;
            }
            if state_filter != "All states" && job.state != state_filter {
                return false;
            }
            if !where_filter.is_empty() {
                let haystack = format!("{} {}", job.partition_raw, job.location_or_reason)
                    .to_ascii_lowercase();
                if !haystack.contains(&where_filter) {
                    return false;
                }
            }
            if !why_filter.is_empty()
                && !job
                    .location_or_reason
                    .to_ascii_lowercase()
                    .contains(&why_filter)
            {
                return false;
            }
            if query.is_empty() {
                true
            } else {
                [
                    job.job_id.as_str(),
                    job.user.as_str(),
                    job.partition_raw.as_str(),
                    job.name.as_str(),
                    job.state.as_str(),
                    job.location_or_reason.as_str(),
                ]
                .into_iter()
                .any(|value| value.to_ascii_lowercase().contains(&query))
            }
        });
        rows.sort_by(|left, right| self.compare_jobs(left, right));
        rows
    }

    pub(crate) fn selected_partition_detail(&self) -> Option<&PartitionOverview> {
        let name = self.detail_partition.as_ref()?;
        self.snapshot
            .as_ref()?
            .partitions
            .iter()
            .find(|partition| partition.name == *name)
    }

    pub(crate) fn selected_node_detail(&self) -> Option<&NodeDetailState> {
        self.node_detail.as_ref()
    }

    pub(crate) fn cluster_trend(&self) -> &TrendSeries {
        &self.cluster_trend
    }

    pub(crate) fn my_trend(&self) -> &TrendSeries {
        &self.my_trend
    }

    pub(crate) fn partition_trend(&self, partition: &str) -> Option<&TrendSeries> {
        self.partition_trends.get(partition)
    }

    pub(crate) fn history_window_label(&self) -> &'static str {
        self.history.window.label()
    }

    pub(crate) fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    pub(crate) fn active_global_query(&self) -> &str {
        if self.input_mode == InputMode::Search {
            &self.search_draft
        } else {
            &self.search_query
        }
    }

    pub(crate) fn active_node_where_filter(&self) -> &str {
        match self.node_detail.as_ref() {
            Some(node) if self.input_mode == InputMode::NodeWhereFilter => &node.where_draft,
            Some(node) => &node.where_filter,
            None => "",
        }
    }

    pub(crate) fn active_node_why_filter(&self) -> &str {
        match self.node_detail.as_ref() {
            Some(node) if self.input_mode == InputMode::NodeWhyFilter => &node.why_draft,
            Some(node) => &node.why_filter,
            None => "",
        }
    }

    fn node_users<'a>(&self, node: &'a NodeDetailState) -> Vec<&'a str> {
        let mut users: Vec<&str> = node.jobs.iter().map(|job| job.user.as_str()).collect();
        users.sort_unstable();
        users.dedup();
        users
    }

    fn node_states<'a>(&self, node: &'a NodeDetailState) -> Vec<&'a str> {
        let mut states: Vec<&str> = node.jobs.iter().map(|job| job.state.as_str()).collect();
        states.sort_unstable();
        states.dedup();
        states
    }

    pub(crate) fn current_node_user_filter<'a>(&self, node: &'a NodeDetailState) -> &'a str {
        let users = self.node_users(node);
        if node.user_filter_index == 0 {
            "All users"
        } else {
            users
                .get(node.user_filter_index - 1)
                .copied()
                .unwrap_or("All users")
        }
    }

    pub(crate) fn current_node_state_filter<'a>(&self, node: &'a NodeDetailState) -> &'a str {
        let states = self.node_states(node);
        if node.state_filter_index == 0 {
            "All states"
        } else {
            states
                .get(node.state_filter_index - 1)
                .copied()
                .unwrap_or("All states")
        }
    }

    fn collect_jobs(&self, mine_only_page: bool, global_mine_only: bool) -> Vec<&JobRecord> {
        let mut items: Vec<&JobRecord> = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.jobs.iter().collect())
            .unwrap_or_default();
        let query = self.active_global_query().to_ascii_lowercase();
        items.retain(|job| {
            if mine_only_page && !job.is_mine {
                return false;
            }
            if global_mine_only && !job.is_mine {
                return false;
            }
            if let Some(pin) = &self.pinned_partition
                && !job.partitions.iter().any(|partition| partition == pin)
            {
                return false;
            }
            if !self.job_filter.matches(job) {
                return false;
            }
            if query.is_empty() {
                return true;
            }
            [
                job.job_id.as_str(),
                job.user.as_str(),
                job.partition_raw.as_str(),
                job.name.as_str(),
                job.state.as_str(),
                job.location_or_reason.as_str(),
            ]
            .into_iter()
            .any(|value| value.to_ascii_lowercase().contains(&query))
        });
        items.sort_by(|left, right| self.compare_jobs(left, right));
        items
    }

    fn compare_partitions(&self, left: &PartitionOverview, right: &PartitionOverview) -> Ordering {
        let order = match self.overview_sort.column {
            OverviewColumn::Partition => left.name.cmp(&right.name),
            OverviewColumn::Nodes => left.total_nodes.cmp(&right.total_nodes),
            OverviewColumn::Pressure => left
                .pressure_ratio(self.metric_mode)
                .unwrap_or_default()
                .partial_cmp(&right.pressure_ratio(self.metric_mode).unwrap_or_default())
                .unwrap_or(Ordering::Equal),
            OverviewColumn::MineRunning => left
                .mine
                .running_total(self.metric_mode)
                .cmp(&right.mine.running_total(self.metric_mode)),
            OverviewColumn::MinePending => left
                .mine
                .pending_total(self.metric_mode)
                .cmp(&right.mine.pending_total(self.metric_mode)),
            OverviewColumn::OthersRunning => left
                .others
                .running_total(self.metric_mode)
                .cmp(&right.others.running_total(self.metric_mode)),
            OverviewColumn::OthersPending => left
                .others
                .pending_total(self.metric_mode)
                .cmp(&right.others.pending_total(self.metric_mode)),
            OverviewColumn::TotalJobs => left
                .total_usage()
                .active_total(MetricMode::Jobs)
                .cmp(&right.total_usage().active_total(MetricMode::Jobs)),
        };

        apply_direction(order, self.overview_sort.direction)
            .then_with(|| left.name.cmp(&right.name))
    }

    fn compare_jobs(&self, left: &JobRecord, right: &JobRecord) -> Ordering {
        let order = match self.job_sort.column {
            JobColumn::Runtime => left.runtime_secs.cmp(&right.runtime_secs),
            JobColumn::TimeLimit => left.time_limit_secs.cmp(&right.time_limit_secs),
            JobColumn::Priority => left.priority.cmp(&right.priority),
            JobColumn::Partition => left.partition_raw.cmp(&right.partition_raw),
            JobColumn::State => state_sort_key(&left.state).cmp(&state_sort_key(&right.state)),
            JobColumn::JobId => job_id_key(&left.job_id).cmp(&job_id_key(&right.job_id)),
            JobColumn::User => left.user.cmp(&right.user),
            JobColumn::Name => left.name.cmp(&right.name),
            JobColumn::Nodes => left.nodes.cmp(&right.nodes),
            JobColumn::Cpus => left.cpus.unwrap_or(0).cmp(&right.cpus.unwrap_or(0)),
            JobColumn::WhereWhy => left.location_or_reason.cmp(&right.location_or_reason),
        };

        apply_direction(order, self.job_sort.direction).then_with(|| left.job_id.cmp(&right.job_id))
    }

    fn compare_users(&self, left: &UserUsage, right: &UserUsage) -> Ordering {
        let order = match self.user_sort.column {
            UserColumn::User => left.user.cmp(&right.user),
            UserColumn::RunningJobs => left.jobs.running_jobs.cmp(&right.jobs.running_jobs),
            UserColumn::PendingJobs => left.jobs.pending_jobs.cmp(&right.jobs.pending_jobs),
            UserColumn::TotalJobs => left.total_jobs().cmp(&right.total_jobs()),
            UserColumn::ResourceFootprint => left
                .total_gpus()
                .cmp(&right.total_gpus())
                .then_with(|| left.total_cpus().cmp(&right.total_cpus()))
                .then_with(|| left.total_nodes().cmp(&right.total_nodes())),
            UserColumn::Partitions => left
                .top_partitions_summary(3)
                .cmp(&right.top_partitions_summary(3)),
        };
        apply_direction(order, self.user_sort.direction).then_with(|| left.user.cmp(&right.user))
    }

    fn compare_history(&self, left: &HistoryRecord, right: &HistoryRecord) -> Ordering {
        match self.history.sort {
            HistorySort::EndTime => right
                .end
                .cmp(&left.end)
                .then_with(|| left.job_id.cmp(&right.job_id)),
            HistorySort::Elapsed => right
                .elapsed_secs
                .cmp(&left.elapsed_secs)
                .then_with(|| left.job_id.cmp(&right.job_id)),
            HistorySort::User => left
                .user
                .cmp(&right.user)
                .then_with(|| left.job_id.cmp(&right.job_id)),
            HistorySort::Partition => left
                .partition
                .cmp(&right.partition)
                .then_with(|| left.job_id.cmp(&right.job_id)),
        }
    }

    fn clamp_selections(&mut self) {
        self.selected_overview = self
            .selected_overview
            .min(self.visible_partitions().len().saturating_sub(1));
        self.selected_my_jobs = self
            .selected_my_jobs
            .min(self.visible_my_jobs().len().saturating_sub(1));
        self.selected_users = self
            .selected_users
            .min(self.visible_users().len().saturating_sub(1));
        self.selected_all_jobs = self
            .selected_all_jobs
            .min(self.visible_all_jobs().len().saturating_sub(1));
        self.selected_partition_jobs = self.selected_partition_jobs.min(
            self.visible_partition_jobs(self.detail_partition.as_deref().unwrap_or(""))
                .len()
                .saturating_sub(1),
        );
        self.selected_partition_node = self.selected_partition_node.min(
            self.visible_partition_nodes(self.detail_partition.as_deref().unwrap_or(""))
                .len()
                .saturating_sub(1),
        );
        self.selected_history = self
            .selected_history
            .min(self.visible_history().len().saturating_sub(1));
        let node_job_len = self.visible_node_jobs().len();
        if let Some(node) = &mut self.node_detail {
            node.selected_job = node.selected_job.min(node_job_len.saturating_sub(1));
        }
    }

    fn receive_updates(&mut self) {
        while let Ok(snapshot) = self.refresh_rx.try_recv() {
            self.push_trends(&snapshot);
            self.snapshot = Some(snapshot);
            self.clamp_selections();
        }

        while let Ok(event) = self.async_rx.try_recv() {
            match event {
                AsyncEvent::HistoryLoaded {
                    window,
                    records,
                    notes,
                    available,
                } => {
                    if self.history.window == window {
                        self.history.records = records;
                        self.history.notes = notes;
                        self.history.available = available;
                        self.history.loading = false;
                        self.history.last_loaded_at =
                            Some(Local::now().format("%F %T").to_string());
                        self.clamp_selections();
                    }
                }
                AsyncEvent::JobDetailLoaded {
                    job_id,
                    detail,
                    error,
                } => {
                    if let Some(Modal::JobDetail(modal)) = &mut self.modal
                        && modal.job_id == job_id
                    {
                        modal.loading = false;
                        modal.detail = detail;
                        modal.error = error;
                    }
                }
                AsyncEvent::NodeLoaded {
                    node_name,
                    detail,
                    jobs,
                    notes,
                } => {
                    if let Some(node) = &mut self.node_detail
                        && node.node_name == node_name
                    {
                        node.detail = detail;
                        node.jobs = jobs;
                        node.notes = notes;
                        node.loading = false;
                        node.last_loaded_at = Some(Local::now().format("%F %T").to_string());
                        self.clamp_selections();
                    }
                }
                AsyncEvent::CancelFinished(report) => {
                    self.status_message = Some(format!(
                        "Cancelled {} job(s); {} failed",
                        report.succeeded(),
                        report.failed()
                    ));
                    self.modal = Some(Modal::CancelResult(report));
                    let _ = self.refresh_tx.send(RefreshCommand::RefreshNow);
                }
            }
        }
    }

    fn push_trends(&mut self, snapshot: &ClusterSnapshot) {
        const MAX_TREND_POINTS: usize = 90;

        let running = snapshot.jobs.iter().filter(|job| job.running).count() as u64;
        let pending = snapshot.jobs.iter().filter(|job| job.pending).count() as u64;
        self.cluster_trend.push(running, pending, MAX_TREND_POINTS);

        let my_running = snapshot
            .jobs
            .iter()
            .filter(|job| job.is_mine && job.running)
            .count() as u64;
        let my_pending = snapshot
            .jobs
            .iter()
            .filter(|job| job.is_mine && job.pending)
            .count() as u64;
        self.my_trend.push(my_running, my_pending, MAX_TREND_POINTS);

        for partition in &snapshot.partitions {
            let total = partition.total_usage();
            self.partition_trends
                .entry(partition.name.clone())
                .or_default()
                .push(
                    u64::from(total.running_jobs),
                    u64::from(total.pending_jobs),
                    MAX_TREND_POINTS,
                );
        }
    }

    #[allow(dead_code)]
    fn request_history_refresh(&mut self) {
        self.history.loading = true;
        let tx = self.async_tx.clone();
        let settings = self.settings.clone();
        let window = self.history.window;
        thread::spawn(move || {
            let collector = Collector::new(&settings);
            let raw = collector.collect_history(window, true);
            let mut notes = Vec::new();
            if let Some(note) = raw.capture.short_error() {
                notes.push(note);
            }
            let available = raw.capture.ok() || !raw.capture.stdout.trim().is_empty();
            let (records, parse_notes) = if available {
                parse_history(&raw.capture.stdout, &settings.user)
            } else {
                (Vec::new(), Vec::new())
            };
            notes.extend(parse_notes);
            let _ = tx.send(AsyncEvent::HistoryLoaded {
                window,
                records,
                notes,
                available,
            });
        });
    }

    fn request_job_detail(&mut self, job_id: String) {
        self.modal = Some(Modal::JobDetail(JobDetailModal {
            job_id: job_id.clone(),
            loading: true,
            detail: None,
            error: None,
        }));
        let tx = self.async_tx.clone();
        let settings = self.settings.clone();
        let shutdown = self.shutdown.clone();
        thread::spawn(move || {
            let collector = Collector::with_cancel(&settings, shutdown.clone());
            let raw = collector.collect_job_detail(&job_id);
            if shutdown.is_cancelled() {
                return;
            }
            let detail = merge_job_detail(raw.live, raw.accounting, &settings.user);
            let (detail, error) = match detail {
                Ok(detail) => (Some(detail), None),
                Err(error) => (None, Some(error)),
            };
            let _ = tx.send(AsyncEvent::JobDetailLoaded {
                job_id,
                detail,
                error,
            });
        });
    }

    fn request_node_refresh(&mut self) {
        let Some(node) = &self.node_detail else {
            return;
        };
        let tx = self.async_tx.clone();
        let settings = self.settings.clone();
        let node_name = node.node_name.clone();
        let shutdown = self.shutdown.clone();
        if let Some(node) = &mut self.node_detail {
            node.loading = true;
        }
        thread::spawn(move || {
            let collector = Collector::with_cancel(&settings, shutdown.clone());
            let raw = collector.collect_node_detail(&node_name);
            if shutdown.is_cancelled() {
                return;
            }
            let mut notes = Vec::new();
            if let Some(note) = raw.node.short_error() {
                notes.push(note);
            }
            if let Some(note) = raw.jobs.short_error() {
                notes.push(note);
            }
            let (detail, detail_notes) = if raw.node.ok() || !raw.node.stdout.trim().is_empty() {
                parse_scontrol_node(&raw.node.stdout)
            } else {
                (None, Vec::new())
            };
            notes.extend(detail_notes);
            let (jobs, parse_notes) = if raw.jobs.ok() || !raw.jobs.stdout.trim().is_empty() {
                parse_jobs(&raw.jobs.stdout, &settings.user)
            } else {
                (Vec::new(), Vec::new())
            };
            notes.extend(parse_notes);
            let _ = tx.send(AsyncEvent::NodeLoaded {
                node_name,
                detail,
                jobs,
                notes,
            });
        });
    }

    fn request_cancel(&mut self, preview: CancelPreview) {
        let tx = self.async_tx.clone();
        let settings = self.settings.clone();
        let scope = preview.scope;
        let job_ids = preview.allowed_job_ids();
        let shutdown = self.shutdown.clone();
        thread::spawn(move || {
            let collector = Collector::with_cancel(&settings, shutdown.clone());
            let mut results = Vec::new();
            for job_id in job_ids {
                if shutdown.is_cancelled() {
                    return;
                }
                let capture = collector.cancel_jobs(std::slice::from_ref(&job_id));
                if capture.status == CommandStatus::Cancelled {
                    return;
                }
                results.push(CancelOutcome {
                    job_id,
                    success: capture.ok(),
                    message: cancel_message(&capture),
                });
            }
            let _ = tx.send(AsyncEvent::CancelFinished(CancelReport { scope, results }));
        });
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if self.input_mode != InputMode::Normal {
            self.handle_search_input(key);
            return;
        }

        if let Some(modal) = &self.modal {
            match modal {
                Modal::Help => match key.code {
                    KeyCode::Char('h') | KeyCode::Esc | KeyCode::Enter | KeyCode::Char('b') => {
                        self.modal = None
                    }
                    _ => {}
                },
                Modal::JobDetail(_) | Modal::CancelResult(_) => match key.code {
                    KeyCode::Esc
                    | KeyCode::Enter
                    | KeyCode::Char('b')
                    | KeyCode::Char('q')
                    | KeyCode::Char('i') => self.modal = None,
                    _ => {}
                },
                Modal::ConfirmCancel(preview) => match key.code {
                    KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('b') => self.modal = None,
                    KeyCode::Enter | KeyCode::Char('y') => {
                        let preview = preview.clone();
                        self.modal = None;
                        self.request_cancel(preview);
                    }
                    _ => {}
                },
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('h') => self.modal = Some(Modal::Help),
            KeyCode::Esc | KeyCode::Char('b') => {
                if self.node_detail.is_some() {
                    self.node_detail = None;
                } else if self.detail_partition.is_some() {
                    self.detail_partition = None;
                } else if matches!(
                    self.current_page(),
                    Page::MyJobs | Page::AllJobs | Page::Users
                ) {
                    self.current_main_index = 0;
                }
            }
            KeyCode::Tab => self.next_page(),
            KeyCode::BackTab => self.prev_page(),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Left => {
                if matches!(
                    self.current_page(),
                    Page::MyJobs | Page::AllJobs | Page::PartitionDetail | Page::NodeDetail
                ) {
                    self.job_horizontal_offset = self.job_horizontal_offset.saturating_sub(1);
                }
            }
            KeyCode::Right => {
                if matches!(
                    self.current_page(),
                    Page::MyJobs | Page::AllJobs | Page::PartitionDetail | Page::NodeDetail
                ) {
                    self.job_horizontal_offset = self.job_horizontal_offset.saturating_add(1);
                }
            }
            KeyCode::Char('r') => {
                let _ = self.refresh_tx.send(RefreshCommand::RefreshNow);
                if self.node_detail.is_some() {
                    self.request_node_refresh();
                }
            }
            KeyCode::Char('s') => {
                match self.current_page() {
                    Page::Overview => self.overview_sort = self.overview_sort.cycle(),
                    Page::MyJobs | Page::AllJobs | Page::PartitionDetail | Page::NodeDetail => {
                        self.job_sort = self.job_sort.cycle()
                    }
                    Page::Users => self.user_sort = self.user_sort.cycle(),
                }
                self.clamp_selections();
            }
            KeyCode::Char('/') => {
                self.search_draft = self.search_query.clone();
                self.input_mode = InputMode::Search;
            }
            KeyCode::Char('f') => {
                match self.current_page() {
                    Page::NodeDetail => self.cycle_node_state_filter(),
                    Page::MyJobs | Page::AllJobs | Page::PartitionDetail => {
                        self.job_filter = self.job_filter.next()
                    }
                    Page::Overview | Page::Users => {}
                }
                self.clamp_selections();
            }
            KeyCode::Char('u') => {
                if self.current_page() == Page::NodeDetail {
                    self.cycle_node_user_filter();
                    self.clamp_selections();
                }
            }
            KeyCode::Char('m') => {
                if self.current_page() != Page::MyJobs {
                    self.show_only_mine = !self.show_only_mine;
                    self.clamp_selections();
                }
            }
            KeyCode::Char('g') => self.metric_mode = self.metric_mode.next(),
            KeyCode::Char('p') => {
                let partition = self.focused_partition_name().map(ToOwned::to_owned);
                if self.pinned_partition.as_ref() == partition.as_ref() {
                    self.pinned_partition = None;
                } else {
                    self.pinned_partition = partition;
                }
                self.clamp_selections();
            }
            KeyCode::Char('w') => {
                if self.current_page() == Page::NodeDetail
                    && let Some(node) = &mut self.node_detail
                {
                    node.where_draft = node.where_filter.clone();
                    self.input_mode = InputMode::NodeWhereFilter;
                }
            }
            KeyCode::Char('y') => {
                if self.current_page() == Page::NodeDetail
                    && let Some(node) = &mut self.node_detail
                {
                    node.why_draft = node.why_filter.clone();
                    self.input_mode = InputMode::NodeWhyFilter;
                }
            }
            KeyCode::Char('c') => {
                if self.current_page() == Page::NodeDetail {
                    self.clear_node_filters();
                }
            }
            KeyCode::Char('n') => {
                if self.current_page() == Page::PartitionDetail {
                    self.open_selected_node();
                }
            }
            KeyCode::Char('[') => {
                if self.current_page() == Page::PartitionDetail {
                    self.move_partition_node_selection(-1);
                }
            }
            KeyCode::Char(']') => {
                if self.current_page() == Page::PartitionDetail {
                    self.move_partition_node_selection(1);
                }
            }
            KeyCode::Enter => self.open_primary_detail(),
            KeyCode::Char('i') => self.open_job_detail_from_selection(),
            KeyCode::Char('x') => self.open_single_cancel_preview(),
            KeyCode::Char('X') => self.open_bulk_cancel_preview(),
            _ => {}
        }
    }

    fn handle_search_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                match self.input_mode {
                    InputMode::Search => self.search_draft = self.search_query.clone(),
                    InputMode::NodeWhereFilter => {
                        if let Some(node) = &mut self.node_detail {
                            node.where_draft = node.where_filter.clone();
                        }
                    }
                    InputMode::NodeWhyFilter => {
                        if let Some(node) = &mut self.node_detail {
                            node.why_draft = node.why_filter.clone();
                        }
                    }
                    InputMode::Normal => {}
                }
                self.input_mode = InputMode::Normal;
                self.clamp_selections();
            }
            KeyCode::Enter => {
                match self.input_mode {
                    InputMode::Search => self.search_query = self.search_draft.clone(),
                    InputMode::NodeWhereFilter => {
                        if let Some(node) = &mut self.node_detail {
                            node.where_filter = node.where_draft.clone();
                        }
                    }
                    InputMode::NodeWhyFilter => {
                        if let Some(node) = &mut self.node_detail {
                            node.why_filter = node.why_draft.clone();
                        }
                    }
                    InputMode::Normal => {}
                }
                self.input_mode = InputMode::Normal;
                self.clamp_selections();
            }
            KeyCode::Backspace => {
                match self.input_mode {
                    InputMode::Search => {
                        self.search_draft.pop();
                    }
                    InputMode::NodeWhereFilter => {
                        if let Some(node) = &mut self.node_detail {
                            node.where_draft.pop();
                        }
                    }
                    InputMode::NodeWhyFilter => {
                        if let Some(node) = &mut self.node_detail {
                            node.why_draft.pop();
                        }
                    }
                    InputMode::Normal => {}
                }
                self.clamp_selections();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                match self.input_mode {
                    InputMode::Search => self.search_draft.push(c),
                    InputMode::NodeWhereFilter => {
                        if let Some(node) = &mut self.node_detail {
                            node.where_draft.push(c);
                        }
                    }
                    InputMode::NodeWhyFilter => {
                        if let Some(node) = &mut self.node_detail {
                            node.why_draft.push(c);
                        }
                    }
                    InputMode::Normal => {}
                }
                self.clamp_selections();
            }
            _ => {}
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollDown => self.move_selection(1),
            MouseEventKind::ScrollUp => self.move_selection(-1),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(hit) = self.hit_map.hit_test(mouse.column, mouse.row) {
                    let now = Instant::now();
                    let double_click = self.last_click.as_ref().is_some_and(|(last_hit, time)| {
                        *last_hit == hit && now.duration_since(*time) < Duration::from_millis(450)
                    });
                    self.apply_mouse_hit(hit.clone(), double_click);
                    self.last_click = Some((hit, now));
                }
            }
            _ => {}
        }
    }

    fn apply_mouse_hit(&mut self, hit: MouseHit, activate: bool) {
        match hit {
            MouseHit::Tab(page) => {
                self.node_detail = None;
                self.detail_partition = None;
                if let Some(index) = self
                    .page_tabs()
                    .iter()
                    .position(|candidate| *candidate == page)
                {
                    self.current_main_index = index;
                }
            }
            MouseHit::Row(kind, index) => {
                match kind {
                    RowKind::Overview => self.selected_overview = index,
                    RowKind::MyJobs => self.selected_my_jobs = index,
                    RowKind::Users => self.selected_users = index,
                    RowKind::AllJobs => self.selected_all_jobs = index,
                    RowKind::PartitionJobs => self.selected_partition_jobs = index,
                    RowKind::PartitionNodes => self.selected_partition_node = index,
                    RowKind::NodeJobs => {
                        if let Some(node) = &mut self.node_detail {
                            node.selected_job = index;
                        }
                    }
                    RowKind::History => self.selected_history = index,
                }
                if activate {
                    match kind {
                        RowKind::Overview => self.open_primary_detail(),
                        RowKind::PartitionNodes => self.open_selected_node(),
                        RowKind::MyJobs
                        | RowKind::AllJobs
                        | RowKind::PartitionJobs
                        | RowKind::NodeJobs
                        | RowKind::History => self.open_job_detail_from_selection(),
                        RowKind::Users => {}
                    }
                }
            }
            MouseHit::OverviewHeader(column) => {
                self.overview_sort.choose(column);
                self.clamp_selections();
            }
            MouseHit::JobHeader(column) => {
                self.job_sort.choose(column);
                self.clamp_selections();
            }
            MouseHit::UserHeader(column) => {
                self.user_sort.choose(column);
                self.clamp_selections();
            }
            MouseHit::Footer(action) => self.trigger_footer_action(action),
            MouseHit::Modal(action) => match action {
                ModalAction::Ignore => {}
                ModalAction::Close | ModalAction::Cancel => self.modal = None,
                ModalAction::Confirm => {
                    if let Some(Modal::ConfirmCancel(preview)) = self.modal.clone() {
                        self.modal = None;
                        self.request_cancel(preview);
                    } else {
                        self.modal = None;
                    }
                }
            },
        }
    }

    fn trigger_footer_action(&mut self, action: FooterAction) {
        match action {
            FooterAction::BackOverview => {
                self.node_detail = None;
                self.detail_partition = None;
                self.current_main_index = 0;
            }
            FooterAction::Refresh => {
                let _ = self.refresh_tx.send(RefreshCommand::RefreshNow);
                if self.node_detail.is_some() {
                    self.request_node_refresh();
                }
            }
            FooterAction::Help => self.modal = Some(Modal::Help),
            FooterAction::ToggleMine => {
                if self.current_page() != Page::MyJobs {
                    self.show_only_mine = !self.show_only_mine;
                }
            }
            FooterAction::OpenDetail => self.open_primary_detail(),
            FooterAction::OpenNode => self.open_selected_node(),
            FooterAction::CancelJob => self.open_single_cancel_preview(),
            FooterAction::BulkCancel => self.open_bulk_cancel_preview(),
            FooterAction::ClearFilters => self.clear_node_filters(),
        }
    }

    fn next_page(&mut self) {
        if self.detail_partition.is_some() || self.node_detail.is_some() {
            return;
        }
        let len = self.page_tabs().len();
        self.current_main_index = (self.current_main_index + 1) % len;
    }

    fn prev_page(&mut self) {
        if self.detail_partition.is_some() || self.node_detail.is_some() {
            return;
        }
        let len = self.page_tabs().len();
        self.current_main_index = (self.current_main_index + len - 1) % len;
    }

    fn move_selection(&mut self, delta: isize) {
        let len = match self.current_page() {
            Page::Overview => self.visible_partitions().len(),
            Page::MyJobs => self.visible_my_jobs().len(),
            Page::Users => self.visible_users().len(),
            Page::AllJobs => self.visible_all_jobs().len(),
            Page::PartitionDetail => self
                .visible_partition_jobs(self.detail_partition.as_deref().unwrap_or(""))
                .len(),
            Page::NodeDetail => self.visible_node_jobs().len(),
        };

        if len == 0 {
            return;
        }

        let target = match self.current_page() {
            Page::Overview => &mut self.selected_overview,
            Page::MyJobs => &mut self.selected_my_jobs,
            Page::Users => &mut self.selected_users,
            Page::AllJobs => &mut self.selected_all_jobs,
            Page::PartitionDetail => &mut self.selected_partition_jobs,
            Page::NodeDetail => {
                if let Some(node) = &mut self.node_detail {
                    let next = (node.selected_job as isize + delta)
                        .clamp(0, len.saturating_sub(1) as isize)
                        as usize;
                    node.selected_job = next;
                }
                return;
            }
        };
        let next = (*target as isize + delta).clamp(0, len.saturating_sub(1) as isize) as usize;
        *target = next;
    }

    fn open_primary_detail(&mut self) {
        match self.current_page() {
            Page::Overview => {
                if let Some(partition) = self.focused_partition_name() {
                    self.detail_partition = Some(partition.to_string());
                    self.selected_partition_jobs = 0;
                    self.selected_partition_node = 0;
                }
            }
            Page::Users => {}
            Page::PartitionDetail | Page::MyJobs | Page::AllJobs | Page::NodeDetail => {
                self.open_job_detail_from_selection()
            }
        }
    }

    fn open_job_detail_from_selection(&mut self) {
        if let Some(job_id) = self.focused_job_id().map(ToOwned::to_owned) {
            self.request_job_detail(job_id);
        } else if let Some(job_id) = self.focused_history_id().map(ToOwned::to_owned) {
            self.request_job_detail(job_id);
        }
    }

    fn open_single_cancel_preview(&mut self) {
        let Some(job) = self.focused_job() else {
            self.status_message = Some("no active job selected".to_string());
            return;
        };
        let preview = CancelPreview {
            scope: CancelScope::Single,
            title: format!("Cancel job {}", job.job_id),
            candidates: vec![build_cancel_candidate(job, &self.settings.user)],
        };
        self.modal = Some(Modal::ConfirmCancel(preview));
    }

    fn open_bulk_cancel_preview(&mut self) {
        let jobs: Vec<&JobRecord> = match self.current_page() {
            Page::MyJobs => self.visible_my_jobs(),
            Page::AllJobs => self.visible_all_jobs(),
            Page::PartitionDetail => self
                .detail_partition
                .as_deref()
                .map(|partition| self.visible_partition_jobs(partition))
                .unwrap_or_default(),
            Page::NodeDetail => self.visible_node_jobs(),
            _ => {
                self.status_message =
                    Some("bulk cancel is available only on active job pages".to_string());
                return;
            }
        };

        let candidates = build_bulk_cancel_candidates(&jobs, &self.settings.user);
        if candidates.is_empty() {
            self.status_message = Some("no visible jobs to review for cancel".to_string());
            return;
        }
        self.modal = Some(Modal::ConfirmCancel(CancelPreview {
            scope: CancelScope::Visible,
            title: format!("Cancel {} visible jobs", candidates.len()),
            candidates,
        }));
    }

    fn focused_partition_name(&self) -> Option<&str> {
        match self.current_page() {
            Page::Overview => self
                .visible_partitions()
                .get(self.selected_overview)
                .map(|partition| partition.name.as_str()),
            Page::PartitionDetail => self.detail_partition.as_deref(),
            Page::MyJobs => self
                .visible_my_jobs()
                .get(self.selected_my_jobs)
                .map(|job| job.primary_partition()),
            Page::Users => None,
            Page::AllJobs => self
                .visible_all_jobs()
                .get(self.selected_all_jobs)
                .map(|job| job.primary_partition()),
            Page::NodeDetail => self
                .node_detail
                .as_ref()
                .map(|node| node.partition.as_str()),
        }
    }

    fn focused_job(&self) -> Option<&JobRecord> {
        match self.current_page() {
            Page::MyJobs => self.visible_my_jobs().get(self.selected_my_jobs).copied(),
            Page::Users => None,
            Page::AllJobs => self.visible_all_jobs().get(self.selected_all_jobs).copied(),
            Page::PartitionDetail => self.detail_partition.as_deref().and_then(|partition| {
                self.visible_partition_jobs(partition)
                    .get(self.selected_partition_jobs)
                    .copied()
            }),
            Page::NodeDetail => self
                .visible_node_jobs()
                .get(
                    self.node_detail
                        .as_ref()
                        .map(|node| node.selected_job)
                        .unwrap_or_default(),
                )
                .copied(),
            _ => None,
        }
    }

    fn focused_job_id(&self) -> Option<&str> {
        self.focused_job().map(|job| job.job_id.as_str())
    }

    fn cycle_node_user_filter(&mut self) {
        let Some(node) = &mut self.node_detail else {
            return;
        };
        let mut users: Vec<String> = node.jobs.iter().map(|job| job.user.clone()).collect();
        users.sort();
        users.dedup();
        let len = users.len().saturating_add(1).max(1);
        node.user_filter_index = (node.user_filter_index + 1) % len;
    }

    fn cycle_node_state_filter(&mut self) {
        let Some(node) = &mut self.node_detail else {
            return;
        };
        let mut states: Vec<String> = node.jobs.iter().map(|job| job.state.clone()).collect();
        states.sort();
        states.dedup();
        let len = states.len().saturating_add(1).max(1);
        node.state_filter_index = (node.state_filter_index + 1) % len;
    }

    fn clear_node_filters(&mut self) {
        if let Some(node) = &mut self.node_detail {
            node.user_filter_index = 0;
            node.state_filter_index = 0;
            node.where_filter.clear();
            node.where_draft.clear();
            node.why_filter.clear();
            node.why_draft.clear();
            node.selected_job = 0;
        }
    }

    fn move_partition_node_selection(&mut self, delta: isize) {
        let len = self
            .visible_partition_nodes(self.detail_partition.as_deref().unwrap_or(""))
            .len();
        if len == 0 {
            return;
        }
        let next = (self.selected_partition_node as isize + delta)
            .clamp(0, len.saturating_sub(1) as isize) as usize;
        self.selected_partition_node = next;
    }

    fn open_selected_node(&mut self) {
        let Some(partition) = self.detail_partition.clone() else {
            self.status_message = Some("Open a partition before opening a node".to_string());
            return;
        };
        let Some(node_name) = self
            .visible_partition_nodes(&partition)
            .get(self.selected_partition_node)
            .map(|node| node.node_name.clone())
        else {
            self.status_message = Some("No node is available in this partition".to_string());
            return;
        };
        self.node_detail = Some(NodeDetailState::new(node_name, partition));
        self.request_node_refresh();
    }

    fn focused_history_id(&self) -> Option<&str> {
        None
    }

    pub(crate) fn footer_actions(&self) -> Vec<FooterAction> {
        let mut actions = vec![FooterAction::Refresh, FooterAction::Help];
        if self.current_page() != Page::MyJobs {
            actions.push(FooterAction::ToggleMine);
        }
        match self.current_page() {
            Page::MyJobs | Page::AllJobs | Page::PartitionDetail => {
                actions.insert(0, FooterAction::BackOverview);
                actions.push(FooterAction::OpenDetail);
                actions.push(FooterAction::CancelJob);
                actions.push(FooterAction::BulkCancel);
            }
            Page::NodeDetail => {
                actions.insert(0, FooterAction::BackOverview);
                actions.push(FooterAction::OpenDetail);
                actions.push(FooterAction::CancelJob);
                actions.push(FooterAction::BulkCancel);
                actions.push(FooterAction::ClearFilters);
            }
            Page::Overview => actions.push(FooterAction::OpenDetail),
            Page::Users => {}
        }
        if self.current_page() == Page::PartitionDetail {
            actions.push(FooterAction::OpenNode);
        }
        actions
    }
}

pub fn run_tui(settings: ResolvedCli) -> Result<()> {
    let (refresh_tx, refresh_cmd_rx) = mpsc::channel::<RefreshCommand>();
    let (snapshot_tx, snapshot_rx) = mpsc::channel::<ClusterSnapshot>();
    let (async_tx, async_rx) = mpsc::channel::<AsyncEvent>();
    let shutdown = CancelFlag::default();
    let worker_settings = settings.clone();
    let worker_shutdown = shutdown.clone();
    let worker = thread::spawn(move || {
        refresh_loop(
            worker_settings,
            refresh_cmd_rx,
            snapshot_tx,
            worker_shutdown,
        )
    });

    enable_raw_mode()?;
    let mut handle = stdout();
    execute!(handle, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(handle);
    let mut terminal = Terminal::new(backend)?;

    let mut app = AppState::new(
        settings,
        refresh_tx.clone(),
        snapshot_rx,
        async_tx.clone(),
        async_rx,
        shutdown.clone(),
    );

    loop {
        app.receive_updates();
        let mut hit_map = UiHitMap::default();
        terminal.draw(|frame| {
            hit_map = render(frame, &app);
        })?;
        app.set_hit_map(hit_map);

        if app.quit {
            break;
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
        }
    }

    shutdown.cancel();
    let _ = refresh_tx.send(RefreshCommand::Quit);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    let _ = worker.join();
    Ok(())
}

fn refresh_loop(
    settings: ResolvedCli,
    refresh_rx: Receiver<RefreshCommand>,
    snapshot_tx: Sender<ClusterSnapshot>,
    shutdown: CancelFlag,
) {
    let mut collector = Collector::with_cancel(&settings, shutdown.clone());
    let interval = Duration::from_secs_f64(settings.interval);

    loop {
        if shutdown.is_cancelled() {
            break;
        }
        let raw = collector.collect_raw();
        if shutdown.is_cancelled() {
            break;
        }
        let snapshot = build_snapshot(&raw, &settings.user);
        if snapshot_tx.send(snapshot).is_err() {
            break;
        }

        match refresh_rx.recv_timeout(interval) {
            Ok(RefreshCommand::RefreshNow) => {}
            Ok(RefreshCommand::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn merge_job_detail(
    live_capture: CommandCapture,
    accounting_capture: CommandCapture,
    current_user: &str,
) -> std::result::Result<JobDetail, String> {
    let mut notes = Vec::new();

    if let Some(note) = live_capture.short_error() {
        notes.push(note);
    }
    if let Some(note) = accounting_capture.short_error() {
        notes.push(note);
    }

    let (live, mut live_notes) = if live_capture.ok() || !live_capture.stdout.trim().is_empty() {
        parse_scontrol_job(&live_capture.stdout, current_user)
    } else {
        (None, Vec::new())
    };
    notes.append(&mut live_notes);

    let (acct, mut acct_notes) =
        if accounting_capture.ok() || !accounting_capture.stdout.trim().is_empty() {
            parse_history_detail(&accounting_capture.stdout, current_user)
        } else {
            (None, Vec::new())
        };
    notes.append(&mut acct_notes);

    let mut detail = acct.or(live.clone()).ok_or_else(|| {
        "job detail unavailable: no parsable scontrol or sacct output".to_string()
    })?;

    if let Some(live) = live {
        merge_detail_fields(&mut detail, &live);
    }
    detail.source_notes = notes;
    Ok(detail)
}

fn merge_detail_fields(target: &mut JobDetail, incoming: &JobDetail) {
    macro_rules! merge_field {
        ($field:ident) => {
            if target.$field.is_none() {
                target.$field = incoming.$field.clone();
            }
        };
    }

    merge_field!(name);
    merge_field!(user);
    merge_field!(account);
    merge_field!(partition);
    merge_field!(state);
    merge_field!(reason);
    merge_field!(exit_code);
    merge_field!(runtime_raw);
    merge_field!(time_limit_raw);
    merge_field!(submit_time);
    merge_field!(start_time);
    merge_field!(end_time);
    merge_field!(node_list);
    merge_field!(work_dir);
    merge_field!(command);
    merge_field!(stdout_path);
    merge_field!(stderr_path);
    merge_field!(gres);
    merge_field!(req_tres);
    merge_field!(alloc_tres);

    if target.nodes.is_none() {
        target.nodes = incoming.nodes;
    }
    if target.n_tasks.is_none() {
        target.n_tasks = incoming.n_tasks;
    }
    if target.cpus.is_none() {
        target.cpus = incoming.cpus;
    }
    if target.memory_mb.is_none() {
        target.memory_mb = incoming.memory_mb;
    }
    if target.requested_gpus.is_none() {
        target.requested_gpus = incoming.requested_gpus;
    }
    target.active |= incoming.active;
    target.is_mine |= incoming.is_mine;
}

fn cancel_message(capture: &CommandCapture) -> String {
    if capture.ok() {
        "cancelled".to_string()
    } else if capture.stderr.trim().is_empty() {
        format!("failed ({:?})", capture.status)
    } else {
        capture
            .stderr
            .lines()
            .next()
            .unwrap_or("cancel failed")
            .to_string()
    }
}

fn apply_direction(order: Ordering, direction: SortDirection) -> Ordering {
    match direction {
        SortDirection::Asc => order,
        SortDirection::Desc => order.reverse(),
    }
}

fn job_id_key(job_id: &str) -> (u64, &str) {
    (job_id.parse::<u64>().unwrap_or_default(), job_id)
}

fn state_sort_key(state: &str) -> (u8, &str) {
    let rank = match state {
        "RUNNING" => 0,
        "COMPLETING" => 1,
        "CONFIGURING" => 2,
        "PENDING" => 3,
        "SUSPENDED" => 4,
        "COMPLETED" => 5,
        "FAILED" => 6,
        "TIMEOUT" => 7,
        "CANCELLED" => 8,
        _ => 9,
    };
    (rank, state)
}

pub(crate) fn cancel_denial_reason(job: &JobRecord, current_user: &str) -> Option<&'static str> {
    if !job.active {
        Some("job is no longer active")
    } else if job.user != current_user {
        Some("only your active jobs can be cancelled")
    } else {
        None
    }
}

fn build_cancel_candidate(job: &JobRecord, current_user: &str) -> CancelCandidate {
    CancelCandidate {
        job_id: job.job_id.clone(),
        name: job.name.clone(),
        user: job.user.clone(),
        partition: job.partition_raw.clone(),
        state: job.state.clone(),
        allowed: cancel_denial_reason(job, current_user).is_none(),
        reason: cancel_denial_reason(job, current_user).map(ToOwned::to_owned),
    }
}

pub(crate) fn build_bulk_cancel_candidates(
    jobs: &[&JobRecord],
    current_user: &str,
) -> Vec<CancelCandidate> {
    jobs.iter()
        .map(|job| build_cancel_candidate(job, current_user))
        .collect()
}

pub fn print_debug_dump(dump: &DebugDump) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(dump)?);
    Ok(())
}

pub fn print_once_summary(snapshot: &ClusterSnapshot) {
    println!(
        "sqtop summary | host {} | user {} | sampled {} ms | degraded {}",
        snapshot.hostname, snapshot.current_user, snapshot.sample_duration_ms, snapshot.degraded
    );
    println!("partitions:");
    for partition in &snapshot.partitions {
        println!(
            "  {:<12}  State: {:<8}  Total nodes: {:>3}  Busy nodes: {:>3}  Mine running: {:>3}  Mine pending: {:>3}  Others running: {:>3}  Others pending: {:>3}",
            partition.name,
            partition.state,
            partition.total_nodes,
            partition.used_for_pressure(MetricMode::Nodes),
            partition.mine.running_jobs,
            partition.mine.pending_jobs,
            partition.others.running_jobs,
            partition.others.pending_jobs,
        );
    }
    println!("my active jobs:");
    for job in snapshot
        .jobs
        .iter()
        .filter(|job| job.is_mine && job.active)
        .take(12)
    {
        println!(
            "  {:<8}  Partition: {:<12}  State: {:<10}  Runtime: {:<8}  {}",
            job.job_id, job.partition_raw, job.state, job.runtime_raw, job.name
        );
    }
    if !snapshot.notes.is_empty() {
        println!("notes:");
        for note in &snapshot.notes {
            println!("  - {note}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use chrono::Utc;

    use crate::cli::{HistoryWindow, ResolvedCli, ThemeChoice};
    use crate::collector::command::CancelFlag;
    use crate::model::{Capabilities, ClusterSnapshot, NodeRecord, SourceHealth};

    use crate::model::JobRecord;

    use super::{build_bulk_cancel_candidates, cancel_denial_reason};

    fn sample_job(user: &str, active: bool) -> JobRecord {
        JobRecord {
            job_id: "1".to_string(),
            user: user.to_string(),
            account: Some("demo".to_string()),
            partition_raw: "gpu_l48".to_string(),
            partitions: vec!["gpu_l48".to_string()],
            name: "train".to_string(),
            state: if active { "RUNNING" } else { "COMPLETED" }.to_string(),
            runtime_raw: "1:00:00".to_string(),
            time_limit_raw: "2:00:00".to_string(),
            runtime_secs: Some(3600),
            time_limit_secs: Some(7200),
            nodes: 1,
            cpus: Some(8),
            memory_mb: Some(8000),
            requested_gpus: Some(1),
            gres: Some("gres:gpu:1".to_string()),
            req_tres: Some("cpu=8,mem=8000M,gres/gpu=1".to_string()),
            alloc_tres: Some("cpu=8,mem=8000M,gres/gpu=1".to_string()),
            location_or_reason: "c56b01n01".to_string(),
            submit_time: Some("2026-04-12T12:00:00".to_string()),
            priority: Some(1),
            is_mine: user == "myli",
            active,
            running: active,
            pending: false,
        }
    }

    fn sample_settings() -> ResolvedCli {
        ResolvedCli {
            interval: 2.0,
            user: "myli".to_string(),
            start_in_all_jobs: false,
            all_jobs_enabled: true,
            theme: ThemeChoice::Dark,
            debug_dump: false,
            once: false,
            compact: false,
            no_color: false,
            history_window: HistoryWindow::H24,
            show_advanced_resources: true,
        }
    }

    fn sample_snapshot() -> ClusterSnapshot {
        ClusterSnapshot {
            collected_at: Utc::now(),
            local_time: "2026-04-12 12:00:00".to_string(),
            hostname: "login04".to_string(),
            current_user: "myli".to_string(),
            sample_duration_ms: 12,
            stale_partition_static: false,
            degraded: false,
            notes: Vec::new(),
            capabilities: Capabilities::default(),
            source_health: SourceHealth {
                nodes_ok: true,
                jobs_ok: true,
                partition_static_ok: true,
            },
            nodes: vec![NodeRecord {
                partition: "gpu_l48".to_string(),
                state: "alloc".to_string(),
                node_name: "c56b01n01".to_string(),
                cpus: Some(52),
                memory_mb: Some(515200),
                gpus: Some(8),
                gres: Some("gpu:l40:8".to_string()),
            }],
            partitions: Vec::new(),
            jobs: vec![sample_job("myli", true)],
        }
    }

    #[test]
    fn forbids_cancel_for_non_owned_job() {
        let job = sample_job("other", true);
        assert_eq!(
            cancel_denial_reason(&job, "myli"),
            Some("only your active jobs can be cancelled")
        );
    }

    #[test]
    fn forbids_cancel_for_inactive_job() {
        let job = sample_job("myli", false);
        assert_eq!(
            cancel_denial_reason(&job, "myli"),
            Some("job is no longer active")
        );
    }

    #[test]
    fn bulk_cancel_preview_marks_allowed_subset() {
        let mine = sample_job("myli", true);
        let other = sample_job("other", true);
        let done = sample_job("myli", false);
        let candidates = build_bulk_cancel_candidates(&[&mine, &other, &done], "myli");
        assert_eq!(
            candidates
                .iter()
                .filter(|candidate| candidate.allowed)
                .count(),
            1
        );
        assert_eq!(
            candidates[1].reason.as_deref(),
            Some("only your active jobs can be cancelled")
        );
    }

    #[test]
    fn search_filters_visible_rows_before_enter() {
        let (refresh_tx, _) = mpsc::channel();
        let (_, refresh_rx) = mpsc::channel();
        let (async_tx, _) = mpsc::channel();
        let (_, async_rx) = mpsc::channel();
        let shutdown = CancelFlag::default();
        let mut app = super::AppState::new(
            sample_settings(),
            refresh_tx,
            refresh_rx,
            async_tx,
            async_rx,
            shutdown,
        );
        app.snapshot = Some(sample_snapshot());
        app.current_main_index = app
            .page_tabs()
            .iter()
            .position(|page| *page == super::Page::AllJobs)
            .unwrap();

        assert_eq!(app.visible_all_jobs().len(), 1);
        app.input_mode = super::InputMode::Search;
        app.search_draft = "does-not-match".to_string();
        assert_eq!(app.visible_all_jobs().len(), 0);
    }

    #[test]
    fn visible_users_respects_current_user_toggle() {
        let (refresh_tx, _) = mpsc::channel();
        let (_, refresh_rx) = mpsc::channel();
        let (async_tx, _) = mpsc::channel();
        let (_, async_rx) = mpsc::channel();
        let shutdown = CancelFlag::default();
        let mut app = super::AppState::new(
            sample_settings(),
            refresh_tx,
            refresh_rx,
            async_tx,
            async_rx,
            shutdown,
        );
        let mut snapshot = sample_snapshot();
        snapshot.jobs.push(sample_job("other", true));
        app.snapshot = Some(snapshot);

        assert_eq!(app.visible_users().len(), 2);
        app.show_only_mine = true;
        let visible = app.visible_users();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].user, "myli");
    }
}
