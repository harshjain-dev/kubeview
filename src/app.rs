use anyhow::Result;
use chrono::Utc;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::k8s::{self, DeploymentInfo, EventInfo, IngressInfo, PodInfo, SecretInfo, ServiceInfo};
use crate::theme::ThemeVariant;

/// An active kubectl port-forward subprocess.
pub struct PortForward {
    pub pod: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub active: bool,
    child: tokio::process::Child,
}

impl PortForward {
    pub async fn stop(&mut self) {
        let _ = self.child.kill().await;
        self.active = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    Viewing,
    ClusterPicker,
    Help,
    PathInput,        // service log path prompt
    ScaleInput,       // deployment replica count prompt
    ContainerPicker,  // pick container before kubectl exec
    PortInput,          // local:remote port-forward prompt
    Confirm,            // yes/no confirmation dialog
    SecretKeyPicker,    // pick which key of a secret to edit
    SecretValueInput,   // edit the value of a secret key
}

/// Action to execute when the user confirms a dialog.
#[derive(Debug, Clone)]
pub enum ConfirmAction {
    RolloutRestart { name: String, namespace: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Pods,
    Services,
    Deployments,
    Ingresses,
    Secrets,
    Events,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Pods,
        Tab::Services,
        Tab::Deployments,
        Tab::Ingresses,
        Tab::Secrets,
        Tab::Events,
    ];

    pub fn title(&self) -> &str {
        match self {
            Tab::Pods => "Pods",
            Tab::Services => "Services",
            Tab::Deployments => "Deployments",
            Tab::Ingresses => "Ingresses",
            Tab::Secrets => "Secrets",
            Tab::Events => "Events",
        }
    }
}

pub struct App {
    pub client: kube::Client,
    pub input_mode: InputMode,

    // Navigation
    pub active_tab: usize,

    // Per-tab selection indices
    pub pod_selected: usize,
    pub svc_selected: usize,
    pub deploy_selected: usize,
    pub ingress_selected: usize,
    pub secret_selected: usize,
    pub event_selected: usize,

    // Data
    pub pods: Vec<PodInfo>,
    pub filtered_pods: Vec<PodInfo>,
    pub services: Vec<ServiceInfo>,
    pub deployments: Vec<DeploymentInfo>,
    pub ingresses: Vec<IngressInfo>,
    pub secrets: Vec<SecretInfo>,
    pub events: Vec<EventInfo>,
    pub namespaces: Vec<String>,
    pub current_namespace: String,
    pub current_context: String,

    // Search (pods only for now)
    pub search_query: String,

    // Viewer
    pub viewer_content: Vec<String>,
    pub viewer_title: String,
    pub viewer_scroll: usize,
    pub log_stream: Option<mpsc::Receiver<String>>,

    // TSH cluster picker
    pub tsh_clusters: Vec<String>,
    pub cluster_picker_index: usize,

    // Background refresh channels
    pod_refresh_rx: Option<mpsc::Receiver<Result<Vec<PodInfo>>>>,
    svc_refresh_rx: Option<mpsc::Receiver<Result<Vec<ServiceInfo>>>>,
    deploy_refresh_rx: Option<mpsc::Receiver<Result<Vec<DeploymentInfo>>>>,
    ingress_refresh_rx: Option<mpsc::Receiver<Result<Vec<IngressInfo>>>>,
    secret_refresh_rx: Option<mpsc::Receiver<Result<Vec<SecretInfo>>>>,
    events_refresh_rx: Option<mpsc::Receiver<Result<Vec<EventInfo>>>>,
    tsh_clusters_rx: Option<mpsc::Receiver<Vec<String>>>,

    // Confirmation dialog
    pub confirm_action: Option<ConfirmAction>,
    pub confirm_yes: bool,  // true = YES highlighted

    pub loading: bool,

    // Refresh
    pub last_refresh: Instant,
    pub refresh_interval_secs: u64,

    // Status / prompts
    pub status_message: String,
    pub service_log_path: String,
    pub path_input_buffer: String,
    pub scale_input_buffer: String,
    pub scale_deploy_name: String,
    pub scale_current_replicas: i32,

    // Container picker (for exec)
    pub container_picker_list: Vec<String>,
    pub container_picker_idx: usize,
    pub container_picker_pod: String,  // pod name waiting for exec

    // Port-forward
    pub port_forwards: Vec<PortForward>,
    pub port_input_buffer: String,

    // Secret editing
    pub secret_key_picker_list: Vec<String>,
    pub secret_key_picker_idx: usize,
    pub secret_key_picker_secret: String,
    pub secret_value_input_key: String,
    pub secret_value_input_buffer: String,

    // Theme
    pub theme: ThemeVariant,

    // Pending exec — set by open_exec(), consumed by main.rs to do terminal swap
    pub pending_exec: Option<(String, String, String)>, // (pod, ns, container)
}

impl App {
    pub async fn new() -> Result<Self> {
        let client = kube::Client::try_default().await?;
        let current_context = k8s::current_context().unwrap_or_else(|| "unknown".into());
        let namespaces = k8s::list_namespaces(&client).await.unwrap_or_default();
        let current_namespace = if namespaces.iter().any(|n| n == "default") {
            "default".to_string()
        } else {
            namespaces.first().cloned().unwrap_or_else(|| "default".into())
        };

        Ok(Self {
            client,
            input_mode: InputMode::Normal,
            active_tab: 0,
            pod_selected: 0,
            svc_selected: 0,
            deploy_selected: 0,
            ingress_selected: 0,
            secret_selected: 0,
            event_selected: 0,
            pods: Vec::new(),
            filtered_pods: Vec::new(),
            services: Vec::new(),
            deployments: Vec::new(),
            ingresses: Vec::new(),
            secrets: Vec::new(),
            events: Vec::new(),
            namespaces,
            current_namespace,
            current_context,
            search_query: String::new(),
            viewer_content: Vec::new(),
            viewer_title: String::new(),
            viewer_scroll: 0,
            log_stream: None,
            tsh_clusters: Vec::new(),
            cluster_picker_index: 0,
            pod_refresh_rx: None,
            svc_refresh_rx: None,
            deploy_refresh_rx: None,
            ingress_refresh_rx: None,
            secret_refresh_rx: None,
            events_refresh_rx: None,
            tsh_clusters_rx: None,
            loading: false,
            last_refresh: Instant::now(),
            refresh_interval_secs: 5,
            status_message: String::new(),
            service_log_path: "logs/service.log".to_string(),
            path_input_buffer: String::new(),
            scale_input_buffer: String::new(),
            scale_deploy_name: String::new(),
            scale_current_replicas: 0,
            container_picker_list: Vec::new(),
            container_picker_idx: 0,
            container_picker_pod: String::new(),
            port_forwards: Vec::new(),
            port_input_buffer: "8080:8080".to_string(),
            pending_exec: None,
            theme: ThemeVariant::Default,
            confirm_action: None,
            confirm_yes: false,
            secret_key_picker_list: Vec::new(),
            secret_key_picker_idx: 0,
            secret_key_picker_secret: String::new(),
            secret_value_input_key: String::new(),
            secret_value_input_buffer: String::new(),
        })
    }

    // ── Startup ──────────────────────────────────────────────────────────────

    pub async fn initial_load(&mut self) -> Result<()> {
        self.pods = k8s::list_pods(&self.client, &self.current_namespace).await?;
        self.apply_filter();
        self.last_refresh = Instant::now();
        let now = Utc::now().format("%H:%M:%S");
        self.status_message = format!("Loaded at {now}");
        Ok(())
    }

    // ── Non-blocking background fetches ──────────────────────────────────────

    pub fn schedule_pod_refresh(&mut self) {
        let client = self.client.clone();
        let ns = self.current_namespace.clone();
        let (tx, rx) = mpsc::channel(1);
        self.pod_refresh_rx = Some(rx);
        self.loading = true;
        self.status_message = "Refreshing…".into();
        tokio::spawn(async move {
            let _ = tx.send(k8s::list_pods(&client, &ns).await).await;
        });
    }

    pub fn schedule_svc_refresh(&mut self) {
        let client = self.client.clone();
        let ns = self.current_namespace.clone();
        let (tx, rx) = mpsc::channel(1);
        self.svc_refresh_rx = Some(rx);
        self.loading = true;
        self.status_message = "Refreshing…".into();
        tokio::spawn(async move {
            let _ = tx.send(k8s::list_services(&client, &ns).await).await;
        });
    }

    pub fn schedule_deploy_refresh(&mut self) {
        let client = self.client.clone();
        let ns = self.current_namespace.clone();
        let (tx, rx) = mpsc::channel(1);
        self.deploy_refresh_rx = Some(rx);
        self.loading = true;
        self.status_message = "Refreshing…".into();
        tokio::spawn(async move {
            let _ = tx.send(k8s::list_deployments(&client, &ns).await).await;
        });
    }

    pub fn schedule_events_refresh(&mut self) {
        let client = self.client.clone();
        let ns = self.current_namespace.clone();
        let (tx, rx) = mpsc::channel(1);
        self.events_refresh_rx = Some(rx);
        self.loading = true;
        self.status_message = "Refreshing…".into();
        tokio::spawn(async move {
            let _ = tx.send(k8s::list_events(&client, &ns).await).await;
        });
    }

    pub fn schedule_ingress_refresh(&mut self) {
        let client = self.client.clone();
        let ns = self.current_namespace.clone();
        let (tx, rx) = mpsc::channel(1);
        self.ingress_refresh_rx = Some(rx);
        self.loading = true;
        self.status_message = "Refreshing…".into();
        tokio::spawn(async move {
            let _ = tx.send(k8s::list_ingresses(&client, &ns).await).await;
        });
    }

    pub fn schedule_secret_refresh(&mut self) {
        let client = self.client.clone();
        let ns = self.current_namespace.clone();
        let (tx, rx) = mpsc::channel(1);
        self.secret_refresh_rx = Some(rx);
        self.loading = true;
        self.status_message = "Refreshing…".into();
        tokio::spawn(async move {
            let _ = tx.send(k8s::list_secrets(&client, &ns).await).await;
        });
    }

    pub fn schedule_current_tab_refresh(&mut self) {
        match Tab::ALL[self.active_tab] {
            Tab::Pods => self.schedule_pod_refresh(),
            Tab::Services => self.schedule_svc_refresh(),
            Tab::Deployments => self.schedule_deploy_refresh(),
            Tab::Ingresses => self.schedule_ingress_refresh(),
            Tab::Secrets => self.schedule_secret_refresh(),
            Tab::Events => self.schedule_events_refresh(),
        }
    }

    // ── Filter ───────────────────────────────────────────────────────────────

    pub fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_pods = self.pods.clone();
        } else {
            let matcher = SkimMatcherV2::default();
            self.filtered_pods = self
                .pods
                .iter()
                .filter(|p| matcher.fuzzy_match(&p.name, &self.search_query).is_some())
                .cloned()
                .collect();
        }
        if self.pod_selected >= self.filtered_pods.len() && !self.filtered_pods.is_empty() {
            self.pod_selected = self.filtered_pods.len() - 1;
        }
    }

    // ── Tab-aware navigation ─────────────────────────────────────────────────

    pub fn next_item(&mut self) {
        match Tab::ALL[self.active_tab] {
            Tab::Pods => {
                if !self.filtered_pods.is_empty() {
                    self.pod_selected =
                        (self.pod_selected + 1).min(self.filtered_pods.len() - 1);
                }
            }
            Tab::Services => {
                if !self.services.is_empty() {
                    self.svc_selected = (self.svc_selected + 1).min(self.services.len() - 1);
                }
            }
            Tab::Deployments => {
                if !self.deployments.is_empty() {
                    self.deploy_selected =
                        (self.deploy_selected + 1).min(self.deployments.len() - 1);
                }
            }
            Tab::Ingresses => {
                if !self.ingresses.is_empty() {
                    self.ingress_selected =
                        (self.ingress_selected + 1).min(self.ingresses.len() - 1);
                }
            }
            Tab::Secrets => {
                if !self.secrets.is_empty() {
                    self.secret_selected =
                        (self.secret_selected + 1).min(self.secrets.len() - 1);
                }
            }
            Tab::Events => {
                if !self.events.is_empty() {
                    self.event_selected =
                        (self.event_selected + 1).min(self.events.len() - 1);
                }
            }
        }
    }

    pub fn prev_item(&mut self) {
        match Tab::ALL[self.active_tab] {
            Tab::Pods => self.pod_selected = self.pod_selected.saturating_sub(1),
            Tab::Services => self.svc_selected = self.svc_selected.saturating_sub(1),
            Tab::Deployments => self.deploy_selected = self.deploy_selected.saturating_sub(1),
            Tab::Ingresses => self.ingress_selected = self.ingress_selected.saturating_sub(1),
            Tab::Secrets => self.secret_selected = self.secret_selected.saturating_sub(1),
            Tab::Events => self.event_selected = self.event_selected.saturating_sub(1),
        }
    }

    pub fn jump_top(&mut self) {
        match Tab::ALL[self.active_tab] {
            Tab::Pods => self.pod_selected = 0,
            Tab::Services => self.svc_selected = 0,
            Tab::Deployments => self.deploy_selected = 0,
            Tab::Ingresses => self.ingress_selected = 0,
            Tab::Secrets => self.secret_selected = 0,
            Tab::Events => self.event_selected = 0,
        }
    }

    pub fn jump_bottom(&mut self) {
        match Tab::ALL[self.active_tab] {
            Tab::Pods => {
                if !self.filtered_pods.is_empty() {
                    self.pod_selected = self.filtered_pods.len() - 1;
                }
            }
            Tab::Services => {
                if !self.services.is_empty() {
                    self.svc_selected = self.services.len() - 1;
                }
            }
            Tab::Deployments => {
                if !self.deployments.is_empty() {
                    self.deploy_selected = self.deployments.len() - 1;
                }
            }
            Tab::Ingresses => {
                if !self.ingresses.is_empty() {
                    self.ingress_selected = self.ingresses.len() - 1;
                }
            }
            Tab::Secrets => {
                if !self.secrets.is_empty() {
                    self.secret_selected = self.secrets.len() - 1;
                }
            }
            Tab::Events => {
                if !self.events.is_empty() {
                    self.event_selected = self.events.len() - 1;
                }
            }
        }
    }

    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % Tab::ALL.len();
    }

    pub fn prev_tab(&mut self) {
        if self.active_tab == 0 {
            self.active_tab = Tab::ALL.len() - 1;
        } else {
            self.active_tab -= 1;
        }
    }

    pub fn select_tab(&mut self, idx: usize) {
        if idx < Tab::ALL.len() {
            self.active_tab = idx;
        }
    }

    pub fn next_namespace(&mut self) {
        if self.namespaces.is_empty() {
            return;
        }
        let idx = self
            .namespaces
            .iter()
            .position(|n| n == &self.current_namespace)
            .unwrap_or(0);
        self.current_namespace = self.namespaces[(idx + 1) % self.namespaces.len()].clone();
        self.pod_selected = 0;
        self.svc_selected = 0;
        self.deploy_selected = 0;
        self.ingress_selected = 0;
        self.secret_selected = 0;
        self.event_selected = 0;
        self.schedule_current_tab_refresh();
    }

    pub fn enter_search(&mut self) {
        self.input_mode = InputMode::Search;
        self.search_query.clear();
    }

    pub fn exit_search(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    pub fn selected_pod(&self) -> Option<&PodInfo> {
        self.filtered_pods.get(self.pod_selected)
    }

    pub fn selected_deployment(&self) -> Option<&DeploymentInfo> {
        self.deployments.get(self.deploy_selected)
    }

    pub fn selected_service(&self) -> Option<&ServiceInfo> {
        self.services.get(self.svc_selected)
    }

    pub fn selected_ingress(&self) -> Option<&IngressInfo> {
        self.ingresses.get(self.ingress_selected)
    }

    pub fn selected_secret(&self) -> Option<&SecretInfo> {
        self.secrets.get(self.secret_selected)
    }

    // ── Viewers ──────────────────────────────────────────────────────────────

    pub async fn view_logs(&mut self) -> Result<()> {
        if let Some(pod) = self.selected_pod() {
            let name = pod.name.clone();
            let ns = self.current_namespace.clone();
            self.viewer_title = format!("Logs: {name}");
            self.viewer_content = k8s::get_pod_logs(&self.client, &ns, &name)
                .await
                .unwrap_or_else(|e| vec![format!("Error: {e}")]);
            self.viewer_scroll = self.viewer_content.len().saturating_sub(1);
            self.log_stream = None;
            self.input_mode = InputMode::Viewing;
        }
        Ok(())
    }

    pub fn prompt_service_log_path(&mut self) {
        if self.selected_pod().is_some() {
            self.path_input_buffer = self.service_log_path.clone();
            self.input_mode = InputMode::PathInput;
        }
    }

    pub async fn confirm_service_log_path(&mut self) -> Result<()> {
        let path = self.path_input_buffer.trim().to_string();
        if path.is_empty() {
            self.input_mode = InputMode::Normal;
            return Ok(());
        }
        self.service_log_path = path.clone();
        if let Some(pod) = self.selected_pod() {
            let name = pod.name.clone();
            let ns = self.current_namespace.clone();
            self.viewer_title = format!("Service Logs [live]: {name}");
            self.viewer_content = vec![format!("Tailing {path} in pod {name} …"), String::new()];
            self.viewer_scroll = 0;
            self.log_stream = Some(k8s::stream_exec_tail(&ns, &name, &path).await);
            self.input_mode = InputMode::Viewing;
        }
        Ok(())
    }

    pub async fn describe_selected(&mut self) -> Result<()> {
        match Tab::ALL[self.active_tab] {
            Tab::Pods => {
                if let Some(pod) = self.selected_pod() {
                    let name = pod.name.clone();
                    let ns = self.current_namespace.clone();
                    self.viewer_title = format!("Describe Pod: {name}");
                    self.viewer_content = k8s::describe_pod(&self.client, &ns, &name)
                        .await
                        .unwrap_or_else(|e| vec![format!("Error: {e}")]);
                    self.viewer_scroll = 0;
                    self.log_stream = None;
                    self.input_mode = InputMode::Viewing;
                }
            }
            Tab::Deployments => {
                if let Some(d) = self.selected_deployment() {
                    let name = d.name.clone();
                    let ns = self.current_namespace.clone();
                    self.viewer_title = format!("Describe Deployment: {name}");
                    self.viewer_content = k8s::describe_deployment(&self.client, &ns, &name)
                        .await
                        .unwrap_or_else(|e| vec![format!("Error: {e}")]);
                    self.viewer_scroll = 0;
                    self.log_stream = None;
                    self.input_mode = InputMode::Viewing;
                }
            }
            Tab::Services => {
                if let Some(svc) = self.selected_service() {
                    let name = svc.name.clone();
                    let ns = self.current_namespace.clone();
                    self.viewer_title = format!("YAML: {name}");
                    self.viewer_content = k8s::get_resource_yaml("service", &name, &ns).await;
                    self.viewer_scroll = 0;
                    self.log_stream = None;
                    self.input_mode = InputMode::Viewing;
                }
            }
            Tab::Ingresses => {
                if let Some(ing) = self.selected_ingress() {
                    let name = ing.name.clone();
                    let ns = self.current_namespace.clone();
                    self.viewer_title = format!("Ingress: {name}");
                    self.viewer_content = k8s::describe_ingress(&self.client, &ns, &name).await;
                    self.viewer_scroll = 0;
                    self.log_stream = None;
                    self.input_mode = InputMode::Viewing;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn view_yaml(&mut self) {
        let (kind, name) = match Tab::ALL[self.active_tab] {
            Tab::Pods => {
                let name = self.selected_pod().map(|p| p.name.clone()).unwrap_or_default();
                ("pod", name)
            }
            Tab::Services => {
                let name = self.selected_service().map(|s| s.name.clone()).unwrap_or_default();
                ("service", name)
            }
            Tab::Deployments => {
                let name = self.selected_deployment().map(|d| d.name.clone()).unwrap_or_default();
                ("deployment", name)
            }
            Tab::Ingresses => {
                let name = self.selected_ingress().map(|i| i.name.clone()).unwrap_or_default();
                ("ingress", name)
            }
            Tab::Secrets => {
                // y = raw YAML (base64-encoded values as stored in k8s)
                if let Some(secret) = self.selected_secret() {
                    let name = secret.name.clone();
                    let ns = self.current_namespace.clone();
                    self.viewer_title = format!("YAML (raw): {name}");
                    self.viewer_content = k8s::get_resource_yaml("secret", &name, &ns).await;
                    self.viewer_scroll = 0;
                    self.log_stream = None;
                    self.input_mode = InputMode::Viewing;
                }
                return;
            }
            _ => return,
        };
        if name.is_empty() {
            return;
        }
        let ns = self.current_namespace.clone();
        self.viewer_title = format!("YAML: {kind}/{name}");
        self.viewer_content = k8s::get_resource_yaml(kind, &name, &ns).await;
        self.viewer_scroll = 0;
        self.log_stream = None;
        self.input_mode = InputMode::Viewing;
    }

    /// Show decoded (human-readable) secret values — bound to Enter/d on Secrets tab.
    pub async fn view_secret_decoded(&mut self) {
        if let Some(secret) = self.selected_secret() {
            let name = secret.name.clone();
            let ns = self.current_namespace.clone();
            self.viewer_title = format!("Secret (decoded): {name}");
            self.viewer_content = k8s::get_secret_data(&self.client, &ns, &name).await;
            self.viewer_scroll = 0;
            self.log_stream = None;
            self.input_mode = InputMode::Viewing;
        }
    }

    pub fn cycle_theme(&mut self) {
        self.theme = self.theme.next();
        self.status_message = format!("Theme: {}", self.theme.name());
    }

    pub async fn view_helm_list(&mut self) {
        let ns = self.current_namespace.clone();
        self.viewer_title = format!("Helm releases: {ns}");
        self.viewer_content = k8s::list_helm_releases(&ns).await;
        self.viewer_scroll = 0;
        self.log_stream = None;
        self.input_mode = InputMode::Viewing;
    }

    pub fn close_viewer(&mut self) {
        self.input_mode = InputMode::Normal;
        self.viewer_content.clear();
        self.log_stream = None;
    }

    pub fn scroll_viewer_down(&mut self) {
        if !self.viewer_content.is_empty() {
            self.viewer_scroll =
                (self.viewer_scroll + 1).min(self.viewer_content.len().saturating_sub(1));
        }
    }

    pub fn scroll_viewer_up(&mut self) {
        self.viewer_scroll = self.viewer_scroll.saturating_sub(1);
    }

    pub fn scroll_viewer_bottom(&mut self) {
        self.viewer_scroll = self.viewer_content.len().saturating_sub(1);
    }

    pub fn scroll_viewer_top(&mut self) {
        self.viewer_scroll = 0;
    }

    // ── Deployment actions ───────────────────────────────────────────────────

    pub fn prompt_scale(&mut self) {
        if let Some(d) = self.deployments.get(self.deploy_selected) {
            let name = d.name.clone();
            let replicas = d.replicas;
            self.scale_deploy_name = name;
            self.scale_current_replicas = replicas;
            self.scale_input_buffer = replicas.to_string();
            self.input_mode = InputMode::ScaleInput;
        }
    }

    pub async fn confirm_scale(&mut self) -> Result<()> {
        let replicas: i32 = match self.scale_input_buffer.trim().parse() {
            Ok(n) => n,
            Err(_) => {
                self.status_message = "Invalid replica count".into();
                self.input_mode = InputMode::Normal;
                return Ok(());
            }
        };
        let name = self.scale_deploy_name.clone();
        let ns = self.current_namespace.clone();
        self.input_mode = InputMode::Normal;
        self.status_message = format!("Scaling {name} to {replicas}…");
        match k8s::scale_deployment(&self.client, &ns, &name, replicas).await {
            Ok(()) => {
                self.status_message = format!("Scaled {name} to {replicas} replicas");
                self.schedule_deploy_refresh();
            }
            Err(e) => self.status_message = format!("Scale failed: {e}"),
        }
        Ok(())
    }

    /// Ask for confirmation before rollout restart.
    pub fn request_rollout_restart(&mut self) {
        if let Some(d) = self.selected_deployment() {
            let name = d.name.clone();
            let ns = self.current_namespace.clone();
            self.confirm_action = Some(ConfirmAction::RolloutRestart { name, namespace: ns });
            self.confirm_yes = false;
            self.input_mode = InputMode::Confirm;
        }
    }

    pub async fn execute_confirm(&mut self) -> Result<()> {
        if let Some(action) = self.confirm_action.take() {
            self.input_mode = InputMode::Normal;
            match action {
                ConfirmAction::RolloutRestart { name, namespace } => {
                    self.status_message = format!("Restarting {name}…");
                    match k8s::rollout_restart(&namespace, &name).await {
                        Ok(()) => {
                            self.status_message = format!("Rollout restart triggered for {name}");
                            self.schedule_deploy_refresh();
                        }
                        Err(e) => self.status_message = format!("Restart failed: {e}"),
                    }
                }
            }
        }
        Ok(())
    }

    pub fn cancel_confirm(&mut self) {
        self.confirm_action = None;
        self.input_mode = InputMode::Normal;
        self.status_message = "Cancelled.".into();
    }

    // ── Secret editing ───────────────────────────────────────────────────────

    /// Open the key picker for the currently selected secret.
    pub fn open_secret_edit(&mut self) {
        if let Some(secret) = self.selected_secret() {
            if secret.key_names.is_empty() {
                self.status_message = "Secret has no data keys to edit.".into();
                return;
            }
            let mut keys = secret.key_names.clone();
            keys.sort();
            self.secret_key_picker_secret = secret.name.clone();
            self.secret_key_picker_list = keys;
            self.secret_key_picker_idx = 0;
            self.input_mode = InputMode::SecretKeyPicker;
        }
    }

    pub fn secret_key_picker_next(&mut self) {
        if !self.secret_key_picker_list.is_empty() {
            self.secret_key_picker_idx =
                (self.secret_key_picker_idx + 1).min(self.secret_key_picker_list.len() - 1);
        }
    }

    pub fn secret_key_picker_prev(&mut self) {
        self.secret_key_picker_idx = self.secret_key_picker_idx.saturating_sub(1);
    }

    /// Called when user picks a key — fetch current value and open value editor.
    pub async fn confirm_secret_key_selection(&mut self) {
        let key = match self.secret_key_picker_list.get(self.secret_key_picker_idx) {
            Some(k) => k.clone(),
            None => return,
        };
        let secret_name = self.secret_key_picker_secret.clone();
        let ns = self.current_namespace.clone();
        let current_value = k8s::get_secret_key_raw(&self.client, &ns, &secret_name, &key).await;
        self.secret_value_input_key = key;
        self.secret_value_input_buffer = current_value;
        self.input_mode = InputMode::SecretValueInput;
    }

    /// Apply the edited value back to the secret.
    pub async fn confirm_secret_value_edit(&mut self) -> Result<()> {
        let key = self.secret_value_input_key.clone();
        let value = self.secret_value_input_buffer.clone();
        let name = self.secret_key_picker_secret.clone();
        let ns = self.current_namespace.clone();
        self.input_mode = InputMode::Normal;
        self.status_message = format!("Updating {name}/{key}…");
        match k8s::update_secret_key(&self.client, &ns, &name, &key, &value).await {
            Ok(()) => {
                self.status_message = format!("Updated {name}/{key}");
                self.schedule_secret_refresh();
            }
            Err(e) => self.status_message = format!("Update failed: {e}"),
        }
        Ok(())
    }

    // ── Exec into pod ────────────────────────────────────────────────────────

    /// Either opens the container picker (multi-container) or sets pending_exec directly.
    pub fn open_exec(&mut self) {
        let pod = match self.selected_pod() {
            Some(p) => p.clone(),
            None => return,
        };
        let ns = self.current_namespace.clone();

        if pod.containers.len() <= 1 {
            let container = pod.containers.first().cloned().unwrap_or_default();
            self.pending_exec = Some((pod.name, ns, container));
        } else {
            self.container_picker_pod = pod.name.clone();
            self.container_picker_list = pod.containers.clone();
            self.container_picker_idx = 0;
            self.input_mode = InputMode::ContainerPicker;
        }
    }

    pub fn container_picker_next(&mut self) {
        if !self.container_picker_list.is_empty() {
            self.container_picker_idx =
                (self.container_picker_idx + 1).min(self.container_picker_list.len() - 1);
        }
    }

    pub fn container_picker_prev(&mut self) {
        self.container_picker_idx = self.container_picker_idx.saturating_sub(1);
    }

    pub fn confirm_container_for_exec(&mut self) {
        let container = self
            .container_picker_list
            .get(self.container_picker_idx)
            .cloned()
            .unwrap_or_default();
        let pod = self.container_picker_pod.clone();
        let ns = self.current_namespace.clone();
        self.input_mode = InputMode::Normal;
        self.pending_exec = Some((pod, ns, container));
    }

    // ── Port-forward ─────────────────────────────────────────────────────────

    pub fn prompt_port_forward(&mut self) {
        if self.selected_pod().is_some() {
            self.input_mode = InputMode::PortInput;
        }
    }

    pub async fn confirm_port_forward(&mut self) -> Result<()> {
        let pod = match self.selected_pod() {
            Some(p) => p.name.clone(),
            None => {
                self.input_mode = InputMode::Normal;
                return Ok(());
            }
        };
        let ns = self.current_namespace.clone();

        let (local_port, remote_port) = match parse_port_pair(&self.port_input_buffer) {
            Some(pair) => pair,
            None => {
                self.status_message = format!("Invalid port '{}' — use local:remote (e.g. 8080:8080)", self.port_input_buffer);
                self.input_mode = InputMode::Normal;
                return Ok(());
            }
        };

        self.input_mode = InputMode::Normal;
        self.status_message = format!("Port-forwarding {local_port}→{pod}:{remote_port}…");

        match k8s::spawn_port_forward(&ns, &pod, local_port, remote_port).await {
            Ok(child) => {
                self.port_forwards.push(PortForward {
                    pod: pod.clone(),
                    local_port,
                    remote_port,
                    active: true,
                    child,
                });
                self.status_message =
                    format!("Port-forward active: localhost:{local_port} → {pod}:{remote_port}");
            }
            Err(e) => self.status_message = format!("Port-forward failed: {e}"),
        }
        Ok(())
    }

    pub async fn cleanup_port_forwards(&mut self) {
        for pf in &mut self.port_forwards {
            pf.stop().await;
        }
    }

    pub fn view_port_forwards(&mut self) {
        let lines: Vec<String> = if self.port_forwards.is_empty() {
            vec!["No active port-forwards.".into(), String::new(), "Use 'p' on a pod to start one.".into()]
        } else {
            let mut out = vec![format!("{:<6} {:<30} {}", "LOCAL", "POD", "STATUS")];
            out.push("-".repeat(50));
            for (i, pf) in self.port_forwards.iter().enumerate() {
                let status = if pf.active { "active" } else { "stopped" };
                out.push(format!("[{}] :{:<5} → {}:{} ({})", i + 1, pf.local_port, pf.pod, pf.remote_port, status));
            }
            out.push(String::new());
            out.push("Press Esc to close. (Kill individual forwards with 'p' then re-open)".into());
            out
        };
        self.viewer_title = "Port Forwards".into();
        self.viewer_content = lines;
        self.viewer_scroll = 0;
        self.log_stream = None;
        self.input_mode = InputMode::Viewing;
    }

    // ── TSH cluster picker ───────────────────────────────────────────────────

    pub fn open_cluster_picker(&mut self) {
        self.tsh_clusters = vec!["Loading…".into()];
        self.cluster_picker_index = 0;
        self.input_mode = InputMode::ClusterPicker;
        let (tx, rx) = mpsc::channel(1);
        self.tsh_clusters_rx = Some(rx);
        tokio::spawn(async move {
            let _ = tx.send(k8s::list_tsh_clusters().await).await;
        });
    }

    pub fn cluster_picker_next(&mut self) {
        if !self.tsh_clusters.is_empty() {
            self.cluster_picker_index =
                (self.cluster_picker_index + 1).min(self.tsh_clusters.len() - 1);
        }
    }

    pub fn cluster_picker_prev(&mut self) {
        self.cluster_picker_index = self.cluster_picker_index.saturating_sub(1);
    }

    pub fn close_cluster_picker(&mut self) {
        self.tsh_clusters_rx = None;
        self.input_mode = InputMode::Normal;
    }

    pub async fn confirm_cluster_selection(&mut self) -> Result<()> {
        let cluster = match self.tsh_clusters.get(self.cluster_picker_index) {
            Some(c) if !c.starts_with('(') && c != "Loading…" => c.clone(),
            _ => {
                self.input_mode = InputMode::Normal;
                return Ok(());
            }
        };
        self.status_message = format!("Logging into {cluster}…");
        self.input_mode = InputMode::Normal;
        self.tsh_clusters_rx = None;
        match k8s::tsh_kube_login(&cluster).await {
            Ok(()) => {
                self.client = kube::Client::try_default().await?;
                self.current_context = k8s::current_context().unwrap_or_else(|| "unknown".into());
                self.namespaces = k8s::list_namespaces(&self.client).await.unwrap_or_default();
                // Reset all selections
                self.pod_selected = 0;
                self.svc_selected = 0;
                self.deploy_selected = 0;
                self.ingress_selected = 0;
                self.secret_selected = 0;
                self.event_selected = 0;
                self.status_message = format!("Switched to {cluster} — loading…");
                self.schedule_current_tab_refresh();
            }
            Err(e) => self.status_message = format!("Login failed: {e}"),
        }
        Ok(())
    }

    // ── Tick ─────────────────────────────────────────────────────────────────

    pub async fn tick(&mut self) -> Result<()> {
        // Drain live log stream
        if let Some(rx) = &mut self.log_stream {
            let mut appended = false;
            while let Ok(line) = rx.try_recv() {
                self.viewer_content.push(line);
                appended = true;
            }
            if appended {
                self.viewer_scroll = self.viewer_content.len().saturating_sub(1);
            }
        }

        // Drain background results
        if let Some(rx) = &mut self.pod_refresh_rx {
            if let Ok(result) = rx.try_recv() {
                self.pod_refresh_rx = None;
                self.loading = false;
                match result {
                    Ok(pods) => {
                        self.pods = pods;
                        self.apply_filter();
                        self.last_refresh = Instant::now();
                        self.status_message =
                            format!("Refreshed at {}", Utc::now().format("%H:%M:%S"));
                    }
                    Err(e) => self.status_message = format!("Error: {e}"),
                }
            }
        }

        if let Some(rx) = &mut self.svc_refresh_rx {
            if let Ok(result) = rx.try_recv() {
                self.svc_refresh_rx = None;
                self.loading = false;
                match result {
                    Ok(svcs) => {
                        self.services = svcs;
                        self.last_refresh = Instant::now();
                        self.status_message =
                            format!("Refreshed at {}", Utc::now().format("%H:%M:%S"));
                    }
                    Err(e) => self.status_message = format!("Error: {e}"),
                }
            }
        }

        if let Some(rx) = &mut self.deploy_refresh_rx {
            if let Ok(result) = rx.try_recv() {
                self.deploy_refresh_rx = None;
                self.loading = false;
                match result {
                    Ok(deps) => {
                        self.deployments = deps;
                        self.last_refresh = Instant::now();
                        self.status_message =
                            format!("Refreshed at {}", Utc::now().format("%H:%M:%S"));
                    }
                    Err(e) => self.status_message = format!("Error: {e}"),
                }
            }
        }

        if let Some(rx) = &mut self.ingress_refresh_rx {
            if let Ok(result) = rx.try_recv() {
                self.ingress_refresh_rx = None;
                self.loading = false;
                match result {
                    Ok(ings) => {
                        self.ingresses = ings;
                        self.last_refresh = Instant::now();
                        self.status_message =
                            format!("Refreshed at {}", Utc::now().format("%H:%M:%S"));
                    }
                    Err(e) => self.status_message = format!("Error: {e}"),
                }
            }
        }

        if let Some(rx) = &mut self.secret_refresh_rx {
            if let Ok(result) = rx.try_recv() {
                self.secret_refresh_rx = None;
                self.loading = false;
                match result {
                    Ok(secs) => {
                        self.secrets = secs;
                        self.last_refresh = Instant::now();
                        self.status_message =
                            format!("Refreshed at {}", Utc::now().format("%H:%M:%S"));
                    }
                    Err(e) => self.status_message = format!("Error: {e}"),
                }
            }
        }

        if let Some(rx) = &mut self.events_refresh_rx {
            if let Ok(result) = rx.try_recv() {
                self.events_refresh_rx = None;
                self.loading = false;
                match result {
                    Ok(evts) => {
                        self.events = evts;
                        self.last_refresh = Instant::now();
                        self.status_message =
                            format!("Refreshed at {}", Utc::now().format("%H:%M:%S"));
                    }
                    Err(e) => self.status_message = format!("Error: {e}"),
                }
            }
        }

        if let Some(rx) = &mut self.tsh_clusters_rx {
            if let Ok(clusters) = rx.try_recv() {
                self.tsh_clusters_rx = None;
                self.tsh_clusters = if clusters.is_empty() {
                    vec!["(no clusters found)".into()]
                } else {
                    clusters
                };
            }
        }

        // Auto-refresh
        if !self.loading && self.last_refresh.elapsed().as_secs() >= self.refresh_interval_secs {
            self.last_refresh = Instant::now();
            self.schedule_current_tab_refresh();
        }

        Ok(())
    }
}

fn parse_port_pair(s: &str) -> Option<(u16, u16)> {
    let parts: Vec<&str> = s.trim().splitn(2, ':').collect();
    match parts.as_slice() {
        [local, remote] => {
            let l = local.parse().ok()?;
            let r = remote.parse().ok()?;
            Some((l, r))
        }
        [port] => {
            let p = port.parse().ok()?;
            Some((p, p))
        }
        _ => None,
    }
}
