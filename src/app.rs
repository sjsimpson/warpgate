use chrono::Utc;
use color_eyre::Result;

use crate::task::Task;
use crate::taskwarrior;

/// Convert "20260506T060000Z" -> "2026-05-06" for form display.
/// Taskwarrior accepts this format back as input.
fn format_tw_date(raw: &Option<String>) -> String {
    match raw {
        Some(d) if d.len() >= 8 => {
            format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8])
        }
        Some(d) => d.clone(),
        None => String::new(),
    }
}

#[derive(Debug, Clone)]
pub struct ProjectSummary {
    pub name: String,
    pub remaining: usize,
    pub completed: usize,
    pub total: usize,
    pub avg_age_days: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pane {
    Tasks,
    Projects,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Help,
    Confirm,
    TaskForm,
    Annotate,
    Denotate,
    Filter,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Report {
    Pending,
    All,
    Completed,
    Overdue,
    Active,
}

pub const ALL_REPORTS: &[Report] = &[
    Report::Pending,
    Report::All,
    Report::Completed,
    Report::Overdue,
    Report::Active,
];

impl Report {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::All => "all",
            Self::Completed => "completed",
            Self::Overdue => "overdue",
            Self::Active => "active",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Pending => Self::All,
            Self::All => Self::Completed,
            Self::Completed => Self::Overdue,
            Self::Overdue => Self::Active,
            Self::Active => Self::Pending,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Pending => Self::Active,
            Self::All => Self::Pending,
            Self::Completed => Self::All,
            Self::Overdue => Self::Completed,
            Self::Active => Self::Overdue,
        }
    }

    pub fn filter_args(self) -> Vec<&'static str> {
        match self {
            Self::Pending => vec!["status:pending"],
            Self::All => vec![],
            Self::Completed => vec!["status:completed"],
            Self::Overdue => vec!["+OVERDUE"],
            Self::Active => vec!["+ACTIVE"],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmOption {
    pub label: String,
    pub key: char,
    pub action: ConfirmActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfirmActionKind {
    DeleteInstance,
    DeleteAll,
    DoneInstance,
    Cancel,
}

const FORM_FIELDS: &[FormField] = &[
    FormField::Description,
    FormField::Project,
    FormField::Priority,
    FormField::Due,
    FormField::Tags,
    FormField::Recur,
    FormField::Until,
    FormField::Wait,
    FormField::Scheduled,
    FormField::Depends,
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormField {
    Description,
    Project,
    Priority,
    Due,
    Tags,
    Recur,
    Until,
    Wait,
    Scheduled,
    Depends,
}

impl FormField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Description => "Description",
            Self::Project => "Project",
            Self::Priority => "Priority (H/M/L)",
            Self::Due => "Due",
            Self::Tags => "Tags (comma separated)",
            Self::Recur => "Recur",
            Self::Until => "Until",
            Self::Wait => "Wait",
            Self::Scheduled => "Scheduled",
            Self::Depends => "Depends (task IDs)",
        }
    }

    /// Returns a section header to display before this field, if it starts a new group
    pub fn section_header(self) -> Option<&'static str> {
        match self {
            Self::Description => Some("Info"),
            Self::Recur => Some("Recurrence"),
            Self::Wait => Some("Scheduling"),
            Self::Depends => Some("Dependencies"),
            _ => None,
        }
    }

    pub fn docs(self) -> &'static str {
        match self {
            Self::Description => concat!(
                "The main text describing what needs to be done.\n",
                "\n",
                "Can be any free-form text. Keep it concise but\n",
                "descriptive enough to understand the task later.\n",
                "\n",
                "For additional context, use annotations (n key)\n",
                "after creating the task — they act as timestamped\n",
                "notes attached to the task.\n",
                "\n",
                "Examples:\n",
                "  Review Q3 budget proposal\n",
                "  Fix login page CSS on mobile\n",
                "  Buy groceries for dinner party\n",
            ),
            Self::Project => concat!(
                "Group tasks under a project name.\n",
                "\n",
                "Use dot notation to create a hierarchy:\n",
                "  work\n",
                "  work.meetings\n",
                "  work.meetings.standup\n",
                "  home.garden\n",
                "\n",
                "Parent projects automatically include child\n",
                "tasks in filters. Selecting 'work' in the\n",
                "projects pane shows work.meetings too.\n",
                "\n",
                "Project names are case-sensitive. Leave empty\n",
                "for tasks without a project.\n",
                "\n",
                "Tip: keep project names short — you'll see\n",
                "them in the task list column.\n",
            ),
            Self::Priority => concat!(
                "Urgency weight for prioritizing tasks.\n",
                "\n",
                "Values:\n",
                "  H  High     (+6.0 urgency)\n",
                "  M  Medium   (+3.9 urgency)\n",
                "  L  Low      (+1.8 urgency)\n",
                "  (empty)     no priority boost\n",
                "\n",
                "Priority directly affects the urgency score\n",
                "which determines task sort order. A task with\n",
                "priority H will appear above otherwise equal\n",
                "tasks.\n",
                "\n",
                "Only single letter — enter H, M, or L.\n",
            ),
            Self::Due => concat!(
                "The date/time when this task is due.\n",
                "\n",
                "Absolute dates:\n",
                "  2026-06-01          specific date\n",
                "  2026-06-01T14:00    date and time\n",
                "\n",
                "Relative dates:\n",
                "  today, tomorrow, yesterday\n",
                "  monday, tuesday, ..., sunday\n",
                "  3days, 2weeks, 1month\n",
                "\n",
                "Named boundaries:\n",
                "  eod    end of day\n",
                "  sow    start of (work) week\n",
                "  eow    end of (work) week\n",
                "  eoww   end of calendar week\n",
                "  som    start of month\n",
                "  eom    end of month\n",
                "  soq    start of quarter\n",
                "  eoq    end of quarter\n",
                "  soy    start of year\n",
                "  eoy    end of year\n",
                "\n",
                "Due dates affect urgency:\n",
                "  Overdue tasks get a large urgency boost.\n",
                "  Tasks due today/tomorrow get moderate boost.\n",
                "\n",
                "Required when setting Recur.\n",
            ),
            Self::Tags => concat!(
                "Comma-separated labels for categorizing tasks.\n",
                "\n",
                "Examples:\n",
                "  urgent, frontend\n",
                "  bug, p1\n",
                "  meeting, followup\n",
                "\n",
                "Tags are free-form — any word works. You can\n",
                "filter by tags in taskwarrior with +tag.\n",
                "\n",
                "Special built-in tags:\n",
                "  next      elevates urgency (+15.0),\n",
                "            shows task in 'next' report\n",
                "  nocolor   disables color rules for task\n",
                "  nocal     hides task from calendar report\n",
                "  nonag     suppresses nagging on completion\n",
                "\n",
                "Virtual tags (filter-only, can't add here):\n",
                "  ACTIVE, BLOCKED, BLOCKING, OVERDUE,\n",
                "  DUE, TODAY, TOMORROW, WEEK, MONTH,\n",
                "  ANNOTATED, TAGGED, READY\n",
            ),
            Self::Recur => concat!(
                "How often this task repeats.\n",
                "\n",
                "IMPORTANT: Requires 'Due' to be set.\n",
                "\n",
                "Frequencies:\n",
                "  daily        every day\n",
                "  weekdays     Mon through Fri\n",
                "  weekly       every week\n",
                "  biweekly     every 2 weeks\n",
                "  monthly      every month\n",
                "  quarterly    every 3 months\n",
                "  semiannual   every 6 months\n",
                "  yearly       every year\n",
                "\n",
                "Custom intervals:\n",
                "  2days, 3weeks, 2months, etc.\n",
                "\n",
                "How it works:\n",
                "  Taskwarrior creates a template task and\n",
                "  generates instances from it. Completing\n",
                "  an instance does NOT delete the template.\n",
                "  New instances appear automatically.\n",
                "\n",
                "  Deleting the template (parent) stops all\n",
                "  future recurrences.\n",
                "\n",
                "Use 'Until' to set an expiration date.\n",
            ),
            Self::Until => concat!(
                "Stop creating recurring instances after\n",
                "this date.\n",
                "\n",
                "Only meaningful when Recur is set.\n",
                "\n",
                "Accepts the same date formats as Due:\n",
                "  eoy           end of year\n",
                "  2027-01-01    specific date\n",
                "  6months       relative\n",
                "  eoq           end of quarter\n",
                "\n",
                "Without Until, recurring tasks continue\n",
                "indefinitely.\n",
                "\n",
                "Example: a daily standup that ends when\n",
                "the project wraps in December:\n",
                "  Recur: weekdays\n",
                "  Due:   tomorrow\n",
                "  Until: eoy\n",
            ),
            Self::Wait => concat!(
                "Hide this task until the given date.\n",
                "\n",
                "A waiting task:\n",
                "  - Does NOT appear in pending reports\n",
                "  - Does NOT count toward project stats\n",
                "  - Automatically becomes pending on the\n",
                "    wait date\n",
                "\n",
                "Accepts same date formats as Due:\n",
                "  tomorrow, monday, 2026-06-01\n",
                "  3days, 2weeks, som (start of month)\n",
                "\n",
                "Special values:\n",
                "  later     indefinitely (year 9999)\n",
                "  someday   same as later\n",
                "\n",
                "Use case: \"I need to do this but not\n",
                "until next month — hide it until then\n",
                "so it doesn't clutter my list.\"\n",
            ),
            Self::Scheduled => concat!(
                "Date when this task becomes relevant.\n",
                "\n",
                "Unlike Wait:\n",
                "  - Task IS visible in reports\n",
                "  - But marked as 'not yet actionable'\n",
                "  - Excluded from 'ready' report until\n",
                "    the scheduled date arrives\n",
                "\n",
                "Accepts same date formats as Due:\n",
                "  tomorrow, monday, 2026-06-01\n",
                "  som (start of month)\n",
                "\n",
                "Use case: \"I know about this task and\n",
                "want to see it, but I can't start work\n",
                "on it until next week.\"\n",
                "\n",
                "Comparison:\n",
                "  Wait:      completely hidden until date\n",
                "  Scheduled: visible but not actionable\n",
                "  Due:       deadline for completion\n",
            ),
            Self::Depends => concat!(
                "Other tasks that must finish before this\n",
                "one can start.\n",
                "\n",
                "Enter comma-separated task IDs:\n",
                "  5, 12\n",
                "  5,12,23\n",
                "\n",
                "Effects of dependencies:\n",
                "  - This task is marked as BLOCKED\n",
                "  - Blocked tasks get -5.0 urgency\n",
                "  - Blocking tasks get +8.0 urgency\n",
                "  - Blocked tasks hidden from 'ready'\n",
                "    report until deps are completed\n",
                "\n",
                "When a dependency is completed, this\n",
                "task automatically becomes unblocked.\n",
                "\n",
                "View with: task blocked / task blocking\n",
                "\n",
                "Note: use task IDs (numbers), not UUIDs.\n",
                "IDs may change between sessions, so set\n",
                "dependencies right after checking IDs.\n",
            ),
        }
    }
}

pub struct TaskForm {
    pub description: String,
    pub project: String,
    pub priority: String,
    pub due: String,
    pub tags: String,
    pub recur: String,
    pub until: String,
    pub wait: String,
    pub scheduled: String,
    pub depends: String,
    pub active_field: FormField,
    pub editing_uuid: Option<String>,
    pub docs_focused: bool,
    pub docs_scroll: u16,
}

impl TaskForm {
    pub fn new_advanced(default_project: Option<&str>) -> Self {
        Self {
            description: String::new(),
            project: default_project.unwrap_or("").to_string(),
            priority: String::new(),
            due: String::new(),
            tags: String::new(),
            recur: String::new(),
            until: String::new(),
            wait: String::new(),
            scheduled: String::new(),
            depends: String::new(),
            active_field: FormField::Description,
            editing_uuid: None,
            docs_focused: false,
            docs_scroll: 0,
        }
    }

    pub fn from_task(task: &Task) -> Self {
        Self {
            description: task.description.clone(),
            project: task.project.clone().unwrap_or_default(),
            priority: task.priority.clone().unwrap_or_default(),
            due: format_tw_date(&task.due),
            tags: task.tags.as_ref().map(|t| t.join(", ")).unwrap_or_default(),
            recur: task.recur.clone().unwrap_or_default(),
            until: format_tw_date(&task.until),
            wait: format_tw_date(&task.wait),
            scheduled: format_tw_date(&task.scheduled),
            depends: task.depends.clone().unwrap_or_default(),
            active_field: FormField::Description,
            editing_uuid: task.uuid.clone(),
            docs_focused: false,
            docs_scroll: 0,
        }
    }

    fn field_list(&self) -> &[FormField] {
        FORM_FIELDS
    }

    pub fn next_field(&mut self) {
        let fields = self.field_list();
        let idx = fields
            .iter()
            .position(|f| *f == self.active_field)
            .unwrap_or(0);
        self.active_field = fields[(idx + 1) % fields.len()];
        self.docs_scroll = 0;
    }

    pub fn prev_field(&mut self) {
        let fields = self.field_list();
        let idx = fields
            .iter()
            .position(|f| *f == self.active_field)
            .unwrap_or(0);
        self.active_field = fields[(idx + fields.len() - 1) % fields.len()];
        self.docs_scroll = 0;
    }

    pub fn get_field(&self, field: FormField) -> &str {
        match field {
            FormField::Description => &self.description,
            FormField::Project => &self.project,
            FormField::Priority => &self.priority,
            FormField::Due => &self.due,
            FormField::Tags => &self.tags,
            FormField::Recur => &self.recur,
            FormField::Until => &self.until,
            FormField::Wait => &self.wait,
            FormField::Scheduled => &self.scheduled,
            FormField::Depends => &self.depends,
        }
    }

    pub fn active_buffer_mut(&mut self) -> &mut String {
        match self.active_field {
            FormField::Description => &mut self.description,
            FormField::Project => &mut self.project,
            FormField::Priority => &mut self.priority,
            FormField::Due => &mut self.due,
            FormField::Tags => &mut self.tags,
            FormField::Recur => &mut self.recur,
            FormField::Until => &mut self.until,
            FormField::Wait => &mut self.wait,
            FormField::Scheduled => &mut self.scheduled,
            FormField::Depends => &mut self.depends,
        }
    }

    pub fn visible_fields(&self) -> Vec<FormField> {
        self.field_list().to_vec()
    }
}

pub struct App {
    pub tasks: Vec<Task>,
    pub projects: Vec<String>,
    pub selected_project: usize,
    pub selected_task: usize,
    pub active_pane: Pane,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub running: bool,
    pub status_msg: String,
    pub confirm_options: Vec<ConfirmOption>,
    pub confirm_msg: String,
    pub confirm_selected: usize,
    pub task_form: Option<TaskForm>,
    pub selected_annotation: usize,
    pub active_report: Report,
    pub filter_text: String,
    pub project_summaries: Vec<ProjectSummary>,
}

impl App {
    pub fn new() -> Result<Self> {
        let tasks = taskwarrior::get_pending_tasks().unwrap_or_default();
        let projects = taskwarrior::get_projects(&tasks);

        let mut app = App {
            tasks,
            projects,
            selected_project: 0,
            selected_task: 0,
            active_pane: Pane::Tasks,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            running: true,
            status_msg: String::new(),
            confirm_options: Vec::new(),
            confirm_msg: String::new(),
            confirm_selected: 0,
            task_form: None,
            selected_annotation: 0,
            active_report: Report::Pending,
            filter_text: String::new(),
            project_summaries: Vec::new(),
        };
        app.update_project_summaries();
        Ok(app)
    }

    pub fn load_tasks(&self) -> Vec<Task> {
        let mut args: Vec<&str> = self.active_report.filter_args();
        let filter_words: Vec<String>;
        if !self.filter_text.is_empty() {
            filter_words = self
                .filter_text
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            for w in &filter_words {
                args.push(w.as_str());
            }
        }
        taskwarrior::get_tasks_with_filter(&args).unwrap_or_default()
    }

    pub fn refresh(&mut self) {
        let prev_project = self.selected_project_name().map(|s| s.to_string());

        self.tasks = self.load_tasks();
        self.projects = taskwarrior::get_projects(&self.tasks);

        // If the selected project no longer exists, fall back to (all)
        if let Some(ref name) = prev_project {
            if let Some(idx) = self.projects.iter().position(|p| p == name) {
                self.selected_project = idx + 1; // +1 because 0 = "(all)"
            } else {
                self.selected_project = 0;
                self.status_msg = format!("All tasks in '{}' completed", name);
            }
        }

        let filtered_len = self.filtered_tasks().len();
        if filtered_len == 0 {
            self.selected_task = 0;
        } else if self.selected_task >= filtered_len {
            self.selected_task = filtered_len - 1;
        }

        self.update_project_summaries();
    }

    fn update_project_summaries(&mut self) {
        // Get all tasks (pending + completed) for accurate stats
        let all_tasks = taskwarrior::get_tasks_with_filter(&[]).unwrap_or_default();
        let now = Utc::now().date_naive();

        let mut project_map: std::collections::BTreeMap<String, (usize, usize, Vec<i64>)> =
            std::collections::BTreeMap::new();

        for task in &all_tasks {
            let proj = task.project.clone().unwrap_or("(none)".to_string());
            let entry = project_map.entry(proj).or_insert((0, 0, Vec::new()));

            if task.status.as_deref() == Some("completed") {
                entry.1 += 1; // completed
            } else if task.status.as_deref() == Some("pending") {
                entry.0 += 1; // remaining

                // Calculate age in days
                if let Some(ref e) = task.entry {
                    if e.len() >= 8 {
                        let year: i32 = e[0..4].parse().unwrap_or(0);
                        let month: u32 = e[4..6].parse().unwrap_or(1);
                        let day: u32 = e[6..8].parse().unwrap_or(1);
                        if let Some(created) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
                            entry.2.push((now - created).num_days());
                        }
                    }
                }
            }
        }

        // Ensure parent projects exist for any dotted child
        let child_keys: Vec<String> = project_map.keys().cloned().collect();
        for key in &child_keys {
            let mut parts: Vec<&str> = key.split('.').collect();
            while parts.len() > 1 {
                parts.pop();
                let parent = parts.join(".");
                project_map.entry(parent).or_insert((0, 0, Vec::new()));
            }
        }

        // Build leaf summaries first
        let leaf_summaries: std::collections::BTreeMap<String, (usize, usize, Vec<i64>)> =
            project_map;

        // Aggregate children into parents
        let all_keys: Vec<String> = leaf_summaries.keys().cloned().collect();
        let mut aggregated: std::collections::BTreeMap<String, (usize, usize, Vec<i64>)> =
            std::collections::BTreeMap::new();

        for key in &all_keys {
            let (remaining, completed, ref ages) = leaf_summaries[key];
            // Add to self
            let entry = aggregated.entry(key.clone()).or_insert((0, 0, Vec::new()));
            entry.0 += remaining;
            entry.1 += completed;
            entry.2.extend(ages.iter());

            // Add to all ancestors
            let mut parts: Vec<&str> = key.split('.').collect();
            while parts.len() > 1 {
                parts.pop();
                let parent = parts.join(".");
                let parent_entry = aggregated.entry(parent).or_insert((0, 0, Vec::new()));
                parent_entry.0 += remaining;
                parent_entry.1 += completed;
                parent_entry.2.extend(ages.iter());
            }
        }

        self.project_summaries = aggregated
            .into_iter()
            .map(|(name, (remaining, completed, ages))| {
                let total = remaining + completed;
                let avg_age_days = if ages.is_empty() {
                    0.0
                } else {
                    ages.iter().sum::<i64>() as f64 / ages.len() as f64
                };
                ProjectSummary {
                    name,
                    remaining,
                    completed,
                    total,
                    avg_age_days,
                }
            })
            .collect();
    }

    pub fn selected_project_summary(&self) -> Option<&ProjectSummary> {
        match self.selected_project_name() {
            Some(name) => self.project_summaries.iter().find(|s| s.name == name),
            None => None,
        }
    }

    pub fn all_projects_summary(&self) -> (usize, usize) {
        let remaining: usize = self.project_summaries.iter().map(|s| s.remaining).sum();
        let completed: usize = self.project_summaries.iter().map(|s| s.completed).sum();
        (remaining, completed)
    }

    pub fn filtered_tasks(&self) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = if self.selected_project == 0 {
            self.tasks.iter().collect()
        } else {
            let project = &self.projects[self.selected_project - 1];
            self.tasks
                .iter()
                .filter(|t| {
                    t.project
                        .as_ref()
                        .map(|p| p == project || p.starts_with(&format!("{}.", project)))
                        .unwrap_or(false)
                })
                .collect()
        };

        // Live search filter — matches against description, project, tags
        let search = if self.input_mode == InputMode::Filter {
            &self.input_buffer
        } else {
            &self.filter_text
        };
        if !search.is_empty() {
            let needle = search.to_lowercase();
            tasks.retain(|t| {
                t.description.to_lowercase().contains(&needle)
                    || t.project
                        .as_ref()
                        .map(|p| p.to_lowercase().contains(&needle))
                        .unwrap_or(false)
                    || t.tags
                        .as_ref()
                        .map(|tags| tags.iter().any(|tag| tag.to_lowercase().contains(&needle)))
                        .unwrap_or(false)
            });
        }

        tasks.sort_by(|a, b| {
            let proj_a = a.project.as_deref().unwrap_or("");
            let proj_b = b.project.as_deref().unwrap_or("");
            proj_a.cmp(proj_b).then_with(|| match (&a.due, &b.due) {
                (Some(da), Some(db)) => da.cmp(db),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
        });

        tasks
    }

    pub fn selected_project_name(&self) -> Option<&str> {
        if self.selected_project == 0 {
            None
        } else {
            Some(&self.projects[self.selected_project - 1])
        }
    }

    pub fn project_list_items(&self) -> Vec<String> {
        let mut items = vec!["(all)".to_string()];
        items.extend(self.projects.iter().cloned());
        items
    }

    pub fn move_up(&mut self) {
        match self.active_pane {
            Pane::Projects => {
                if self.selected_project > 0 {
                    self.selected_project -= 1;
                    self.selected_task = 0;
                }
            }
            Pane::Tasks | Pane::Detail => {
                if self.selected_task > 0 {
                    self.selected_task -= 1;
                }
            }
        }
    }

    pub fn move_down(&mut self) {
        match self.active_pane {
            Pane::Projects => {
                if self.selected_project < self.projects.len() {
                    self.selected_project += 1;
                    self.selected_task = 0;
                }
            }
            Pane::Tasks | Pane::Detail => {
                let len = self.filtered_tasks().len();
                if len > 0 && self.selected_task < len - 1 {
                    self.selected_task += 1;
                }
            }
        }
    }

    pub fn next_pane(&mut self) {
        self.active_pane = match self.active_pane {
            Pane::Tasks => Pane::Projects,
            Pane::Projects => Pane::Tasks,
            Pane::Detail => Pane::Tasks,
        };
    }

    pub fn prev_pane(&mut self) {
        self.active_pane = match self.active_pane {
            Pane::Tasks => Pane::Projects,
            Pane::Projects => Pane::Tasks,
            Pane::Detail => Pane::Tasks,
        };
    }

    pub fn selected_task_detail(&self) -> Option<&Task> {
        let filtered = self.filtered_tasks();
        filtered.get(self.selected_task).copied()
    }

    // Confirmation flow
    pub fn request_delete(&mut self) {
        let filtered = self.filtered_tasks();
        let task = match filtered.get(self.selected_task) {
            Some(t) => (*t).clone(),
            None => return,
        };
        drop(filtered);

        self.confirm_msg = format!("Delete '{}'", task.description);

        if task.is_recurring_instance() {
            self.confirm_options = vec![
                ConfirmOption {
                    label: "This instance only".into(),
                    key: 't',
                    action: ConfirmActionKind::DeleteInstance,
                },
                ConfirmOption {
                    label: "All future instances".into(),
                    key: 'a',
                    action: ConfirmActionKind::DeleteAll,
                },
                ConfirmOption {
                    label: "Cancel".into(),
                    key: 'c',
                    action: ConfirmActionKind::Cancel,
                },
            ];
        } else {
            self.confirm_options = vec![
                ConfirmOption {
                    label: "Yes".into(),
                    key: 'y',
                    action: ConfirmActionKind::DeleteInstance,
                },
                ConfirmOption {
                    label: "No".into(),
                    key: 'n',
                    action: ConfirmActionKind::Cancel,
                },
            ];
        }

        self.confirm_selected = 0;
        self.input_mode = InputMode::Confirm;
    }

    pub fn request_done(&mut self) {
        let filtered = self.filtered_tasks();
        let task = match filtered.get(self.selected_task) {
            Some(t) => (*t).clone(),
            None => return,
        };
        drop(filtered);

        if task.status.as_deref() == Some("completed") {
            self.status_msg = "Task already completed".to_string();
            return;
        }
        if task.status.as_deref() == Some("deleted") {
            self.status_msg = "Task is deleted".to_string();
            return;
        }

        self.confirm_msg = format!("Complete '{}'", task.description);

        if task.is_recurring_instance() {
            self.confirm_options = vec![
                ConfirmOption {
                    label: "This instance".into(),
                    key: 'y',
                    action: ConfirmActionKind::DoneInstance,
                },
                ConfirmOption {
                    label: "Cancel".into(),
                    key: 'c',
                    action: ConfirmActionKind::Cancel,
                },
            ];
        } else {
            self.confirm_options = vec![
                ConfirmOption {
                    label: "Yes".into(),
                    key: 'y',
                    action: ConfirmActionKind::DoneInstance,
                },
                ConfirmOption {
                    label: "No".into(),
                    key: 'n',
                    action: ConfirmActionKind::Cancel,
                },
            ];
        }

        self.confirm_selected = 0;
        self.input_mode = InputMode::Confirm;
    }

    pub fn confirm_execute(&mut self) {
        if self.confirm_selected >= self.confirm_options.len() {
            self.confirm_dismiss();
            return;
        }

        let action = self.confirm_options[self.confirm_selected].action;

        if action == ConfirmActionKind::Cancel {
            self.confirm_dismiss();
            return;
        }

        let filtered = self.filtered_tasks();
        if let Some(task) = filtered.get(self.selected_task) {
            match action {
                ConfirmActionKind::DeleteInstance => {
                    if let Some(ref uuid) = task.uuid {
                        let _ = taskwarrior::delete_task(uuid);
                        self.status_msg = format!("Deleted: {}", task.description);
                    }
                }
                ConfirmActionKind::DeleteAll => {
                    // Delete the parent template to stop all future instances
                    if let Some(ref parent) = task.parent {
                        let _ = taskwarrior::delete_task(parent);
                        self.status_msg = format!("Deleted all: {}", task.description);
                    }
                }
                ConfirmActionKind::DoneInstance => {
                    if let Some(ref uuid) = task.uuid {
                        let _ = taskwarrior::complete_task(uuid);
                        self.status_msg = format!("Completed: {}", task.description);
                    }
                }
                ConfirmActionKind::Cancel => {}
            }
            self.refresh();
        }
        self.confirm_dismiss();
    }

    pub fn confirm_by_key(&mut self, c: char) {
        if let Some(idx) = self.confirm_options.iter().position(|o| o.key == c) {
            self.confirm_selected = idx;
            self.confirm_execute();
        }
    }

    pub fn confirm_dismiss(&mut self) {
        self.confirm_options.clear();
        self.input_mode = InputMode::Normal;
    }

    pub fn confirm_move_up(&mut self) {
        if self.confirm_selected > 0 {
            self.confirm_selected -= 1;
        }
    }

    pub fn confirm_move_down(&mut self) {
        if self.confirm_selected + 1 < self.confirm_options.len() {
            self.confirm_selected += 1;
        }
    }

    // Task form
    pub fn open_add_form(&mut self) {
        let proj = self.selected_project_name().map(|s| s.to_string());
        self.task_form = Some(TaskForm::new_advanced(proj.as_deref()));
        self.input_mode = InputMode::TaskForm;
    }

    pub fn open_modify_form(&mut self) {
        let filtered = self.filtered_tasks();
        if let Some(task) = filtered.get(self.selected_task) {
            self.task_form = Some(TaskForm::from_task(task));
            self.input_mode = InputMode::TaskForm;
        }
    }

    pub fn submit_task_form(&mut self) {
        if let Some(ref form) = self.task_form {
            if form.description.is_empty() {
                self.status_msg = "Description required".to_string();
                self.task_form = None;
                self.input_mode = InputMode::Normal;
                return;
            }

            let tags: Vec<String> = form
                .tags
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();

            if let Some(ref uuid) = form.editing_uuid {
                let mut mods = Vec::new();
                mods.push(form.description.clone());
                mods.push(format!("project:{}", form.project));
                mods.push(format!("priority:{}", form.priority));
                mods.push(format!("due:{}", form.due));
                if !form.recur.is_empty() {
                    mods.push(format!("recur:{}", form.recur));
                }
                if !form.until.is_empty() {
                    mods.push(format!("until:{}", form.until));
                }
                if !form.wait.is_empty() {
                    mods.push(format!("wait:{}", form.wait));
                }
                if !form.scheduled.is_empty() {
                    mods.push(format!("scheduled:{}", form.scheduled));
                }
                if !form.depends.is_empty() {
                    mods.push(format!("depends:{}", form.depends));
                }
                for tag in &tags {
                    mods.push(format!("+{}", tag));
                }
                let _ = taskwarrior::modify_task(uuid, &mods);
                self.status_msg = format!("Modified: {}", form.description);
            } else {
                let _ = taskwarrior::add_task_full(
                    &form.description,
                    &form.project,
                    &form.priority,
                    &form.due,
                    &tags,
                    &form.recur,
                    &form.until,
                    &form.wait,
                    &form.scheduled,
                    &form.depends,
                );
                self.status_msg = format!("Added: {}", form.description);
            }
            self.refresh();
        }
        self.task_form = None;
        self.input_mode = InputMode::Normal;
    }

    // Annotate
    pub fn open_annotate(&mut self) {
        if self.selected_task_detail().is_some() {
            self.input_buffer.clear();
            self.input_mode = InputMode::Annotate;
        }
    }

    pub fn submit_annotation(&mut self) {
        if !self.input_buffer.is_empty() {
            let filtered = self.filtered_tasks();
            if let Some(task) = filtered.get(self.selected_task) {
                if let Some(ref uuid) = task.uuid {
                    let _ = taskwarrior::annotate_task(uuid, &self.input_buffer);
                    self.status_msg = format!("Annotated: {}", task.description);
                    self.refresh();
                }
            }
        }
        self.input_buffer.clear();
        self.input_mode = InputMode::Normal;
    }

    // Denotate
    pub fn open_denotate(&mut self) {
        let filtered = self.filtered_tasks();
        if let Some(task) = filtered.get(self.selected_task) {
            if let Some(ref anns) = task.annotations {
                if !anns.is_empty() {
                    self.selected_annotation = 0;
                    self.input_mode = InputMode::Denotate;
                    return;
                }
            }
        }
        self.status_msg = "No annotations to remove".to_string();
    }

    pub fn denotate_move_up(&mut self) {
        if self.selected_annotation > 0 {
            self.selected_annotation -= 1;
        }
    }

    pub fn denotate_move_down(&mut self) {
        let filtered = self.filtered_tasks();
        if let Some(task) = filtered.get(self.selected_task) {
            if let Some(ref anns) = task.annotations {
                if self.selected_annotation < anns.len() - 1 {
                    self.selected_annotation += 1;
                }
            }
        }
    }

    pub fn submit_denotate(&mut self) {
        let filtered = self.filtered_tasks();
        if let Some(task) = filtered.get(self.selected_task) {
            if let Some(ref anns) = task.annotations {
                if let Some(ann) = anns.get(self.selected_annotation) {
                    if let (Some(uuid), Some(desc)) = (&task.uuid, &ann.description) {
                        let _ = taskwarrior::denotate_task(uuid, desc);
                        self.status_msg = format!("Removed annotation: {}", desc);
                        self.refresh();
                    }
                }
            }
        }
        self.input_mode = InputMode::Normal;
    }

    // Duplicate
    pub fn duplicate_selected(&mut self) {
        let filtered = self.filtered_tasks();
        if let Some(task) = filtered.get(self.selected_task) {
            if let Some(ref uuid) = task.uuid {
                let _ = taskwarrior::duplicate_task(uuid);
                self.status_msg = format!("Duplicated: {}", task.description);
                self.refresh();
            }
        }
    }

    // Undo
    pub fn undo(&mut self) {
        let _ = taskwarrior::undo();
        self.status_msg = "Undo".to_string();
        self.refresh();
    }

    // Report switching
    pub fn set_report(&mut self, report: Report) {
        self.active_report = report;
        self.selected_task = 0;
        self.refresh();
    }

    pub fn next_report(&mut self) {
        self.set_report(self.active_report.next());
    }

    pub fn prev_report(&mut self) {
        self.set_report(self.active_report.prev());
    }

    // Filter
    pub fn open_filter(&mut self) {
        self.input_buffer = self.filter_text.clone();
        self.input_mode = InputMode::Filter;
    }

    /// Called on every keystroke in filter mode to keep selection in bounds
    pub fn filter_changed(&mut self) {
        let len = self.filtered_tasks().len();
        if len == 0 {
            self.selected_task = 0;
        } else if self.selected_task >= len {
            self.selected_task = len - 1;
        }
    }

    pub fn submit_filter(&mut self) {
        self.filter_text = self.input_buffer.clone();
        self.input_buffer.clear();
        self.input_mode = InputMode::Normal;
        if self.filter_text.is_empty() {
            self.status_msg = "Filter cleared".to_string();
        } else {
            self.status_msg = format!("Filter: {}", self.filter_text);
        }
    }

    pub fn clear_filter(&mut self) {
        self.filter_text.clear();
        self.input_buffer.clear();
        self.input_mode = InputMode::Normal;
        self.selected_task = 0;
        self.status_msg = "Filter cleared".to_string();
    }

    pub fn toggle_start_selected(&mut self) {
        let filtered = self.filtered_tasks();
        if let Some(task) = filtered.get(self.selected_task) {
            if task.status.as_deref() != Some("pending") {
                self.status_msg = "Can only start/stop pending tasks".to_string();
                return;
            }
            if let Some(ref uuid) = task.uuid {
                if task.is_started() {
                    let _ = taskwarrior::stop_task(uuid);
                    self.status_msg = format!("Stopped: {}", task.description);
                } else {
                    let _ = taskwarrior::start_task(uuid);
                    self.status_msg = format!("Started: {}", task.description);
                }
                self.refresh();
            }
        }
    }
}
