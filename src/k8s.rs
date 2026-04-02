use anyhow::Result;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{Event as KubeEvent, Namespace, Pod, Secret, Service};
use k8s_openapi::api::networking::v1::Ingress;
use kube::{
    api::{Api, ListParams, LogParams, Patch, PatchParams},
    Client,
};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct PodInfo {
    pub name: String,
    pub status: String,
    pub ready: String,
    pub restarts: i32,
    pub age: String,
    pub node: String,
    pub ip: String,
    pub image: String,
    pub containers: Vec<String>, // container names, for exec picker
}

impl PodInfo {
    pub fn status_color(&self) -> ratatui::style::Color {
        match self.status.as_str() {
            "Running" => ratatui::style::Color::Green,
            "Succeeded" => ratatui::style::Color::Blue,
            "Pending" => ratatui::style::Color::Yellow,
            "Failed" => ratatui::style::Color::Red,
            "CrashLoopBackOff" | "Error" | "OOMKilled" => ratatui::style::Color::Red,
            "Terminating" => ratatui::style::Color::Magenta,
            _ => ratatui::style::Color::Gray,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub type_: String,
    pub cluster_ip: String,
    pub external_ip: String,
    pub ports: String,
    pub age: String,
}

#[derive(Debug, Clone)]
pub struct IngressInfo {
    pub name: String,
    pub hosts: String,   // comma-joined host names
    pub paths: String,   // simplified "host/path -> svc:port"
    pub age: String,
}

#[derive(Debug, Clone)]
pub struct SecretInfo {
    pub name: String,
    pub type_: String,
    pub keys: usize,
    pub key_names: Vec<String>,
    pub age: String,
}

pub fn current_context() -> Option<String> {
    let config = kube::config::Kubeconfig::read().ok()?;
    config.current_context
}

pub async fn list_namespaces(client: &Client) -> Result<Vec<String>> {
    let ns_api: Api<Namespace> = Api::all(client.clone());
    let ns_list = ns_api.list(&ListParams::default()).await?;
    let mut names: Vec<String> = ns_list
        .items
        .iter()
        .filter_map(|ns| ns.metadata.name.clone())
        .collect();
    names.sort();
    Ok(names)
}

pub async fn list_pods(client: &Client, namespace: &str) -> Result<Vec<PodInfo>> {
    let pods_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pod_list = pods_api.list(&ListParams::default()).await?;

    let pods = pod_list
        .items
        .iter()
        .map(|pod| {
            let metadata = &pod.metadata;
            let spec = pod.spec.as_ref();
            let status = pod.status.as_ref();

            let name = metadata.name.clone().unwrap_or_default();

            let phase = status
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".into());

            let pod_status = status
                .and_then(|s| s.container_statuses.as_ref())
                .and_then(|cs| {
                    cs.iter().find_map(|c| {
                        c.state.as_ref().and_then(|state| {
                            if let Some(waiting) = &state.waiting {
                                Some(waiting.reason.clone().unwrap_or_else(|| "Waiting".into()))
                            } else if state.terminated.is_some() {
                                Some("Terminated".into())
                            } else {
                                None
                            }
                        })
                    })
                })
                .unwrap_or(phase);

            let pod_status = if metadata.deletion_timestamp.is_some() {
                "Terminating".into()
            } else {
                pod_status
            };

            let (ready_count, total_count) = status
                .and_then(|s| s.container_statuses.as_ref())
                .map(|cs| {
                    let total = cs.len();
                    let ready = cs.iter().filter(|c| c.ready).count();
                    (ready, total)
                })
                .unwrap_or((0, 0));
            let ready = format!("{ready_count}/{total_count}");

            let restarts = status
                .and_then(|s| s.container_statuses.as_ref())
                .map(|cs| cs.iter().map(|c| c.restart_count).sum())
                .unwrap_or(0);

            let age = metadata
                .creation_timestamp
                .as_ref()
                .map(|ts| format_age(ts.0))
                .unwrap_or_else(|| "?".into());

            let node = spec
                .and_then(|s| s.node_name.clone())
                .unwrap_or_else(|| "-".into());

            let ip = status
                .and_then(|s| s.pod_ip.clone())
                .unwrap_or_else(|| "-".into());

            let image = spec
                .and_then(|s| s.containers.first())
                .map(|c| c.image.clone().unwrap_or_default())
                .unwrap_or_default();

            let containers = spec
                .map(|s| s.containers.iter().map(|c| c.name.clone()).collect())
                .unwrap_or_default();

            PodInfo {
                name,
                status: pod_status,
                ready,
                restarts,
                age,
                node,
                ip,
                image,
                containers,
            }
        })
        .collect();

    Ok(pods)
}

pub async fn list_services(client: &Client, namespace: &str) -> Result<Vec<ServiceInfo>> {
    let svc_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let svc_list = svc_api.list(&ListParams::default()).await?;

    let services = svc_list
        .items
        .iter()
        .map(|svc| {
            let meta = &svc.metadata;
            let spec = svc.spec.as_ref();
            let status = svc.status.as_ref();

            let name = meta.name.clone().unwrap_or_default();

            let type_ = spec
                .and_then(|s| s.type_.clone())
                .unwrap_or_else(|| "ClusterIP".into());

            let cluster_ip = spec
                .and_then(|s| s.cluster_ip.clone())
                .unwrap_or_else(|| "-".into());

            let external_ip = status
                .and_then(|s| s.load_balancer.as_ref())
                .and_then(|lb| lb.ingress.as_ref())
                .and_then(|ingress| ingress.first())
                .and_then(|i| i.ip.clone().or_else(|| i.hostname.clone()))
                .unwrap_or_else(|| "<none>".into());

            let ports = spec
                .and_then(|s| s.ports.as_ref())
                .map(|ports| {
                    ports
                        .iter()
                        .map(|p| {
                            let proto = p.protocol.as_deref().unwrap_or("TCP");
                            if let Some(node_port) = p.node_port {
                                format!("{}:{}/{}", p.port, node_port, proto)
                            } else {
                                format!("{}/{}", p.port, proto)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_else(|| "<none>".into());

            let age = meta
                .creation_timestamp
                .as_ref()
                .map(|ts| format_age(ts.0))
                .unwrap_or_else(|| "?".into());

            ServiceInfo {
                name,
                type_,
                cluster_ip,
                external_ip,
                ports,
                age,
            }
        })
        .collect();

    Ok(services)
}

pub async fn get_pod_logs(client: &Client, namespace: &str, name: &str) -> Result<Vec<String>> {
    let pods_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let logs = pods_api
        .logs(
            name,
            &LogParams {
                tail_lines: Some(200),
                ..Default::default()
            },
        )
        .await?;

    Ok(logs.lines().map(String::from).collect())
}

pub async fn describe_pod(client: &Client, namespace: &str, name: &str) -> Result<Vec<String>> {
    let pods_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pod = pods_api.get(name).await?;

    let mut lines = Vec::new();
    let meta = &pod.metadata;
    let spec = pod.spec.as_ref();
    let status = pod.status.as_ref();

    lines.push(format!(
        "Name:         {}",
        meta.name.as_deref().unwrap_or("-")
    ));
    lines.push(format!("Namespace:    {namespace}"));
    lines.push(format!(
        "Node:         {}",
        spec.and_then(|s| s.node_name.as_deref()).unwrap_or("-")
    ));
    lines.push(format!(
        "Status:       {}",
        status
            .and_then(|s| s.phase.as_deref())
            .unwrap_or("Unknown")
    ));
    lines.push(format!(
        "IP:           {}",
        status.and_then(|s| s.pod_ip.as_deref()).unwrap_or("-")
    ));

    if let Some(labels) = &meta.labels {
        lines.push("Labels:".into());
        for (k, v) in labels {
            lines.push(format!("  {k}={v}"));
        }
    }

    if let Some(spec) = spec {
        lines.push(String::new());
        lines.push("Containers:".into());
        for container in &spec.containers {
            lines.push(format!("  {}:", container.name));
            lines.push(format!(
                "    Image:   {}",
                container.image.as_deref().unwrap_or("-")
            ));

            if let Some(ports) = &container.ports {
                for port in ports {
                    lines.push(format!(
                        "    Port:    {}/{}",
                        port.container_port,
                        port.protocol.as_deref().unwrap_or("TCP")
                    ));
                }
            }

            if let Some(resources) = &container.resources {
                if let Some(requests) = &resources.requests {
                    lines.push("    Requests:".into());
                    for (k, v) in requests {
                        lines.push(format!("      {k}: {}", v.0));
                    }
                }
                if let Some(limits) = &resources.limits {
                    lines.push("    Limits:".into());
                    for (k, v) in limits {
                        lines.push(format!("      {k}: {}", v.0));
                    }
                }
            }
        }
    }

    if let Some(status) = status {
        if let Some(conditions) = &status.conditions {
            lines.push(String::new());
            lines.push("Conditions:".into());
            for cond in conditions {
                lines.push(format!(
                    "  {}: {} ({})",
                    cond.type_,
                    cond.status,
                    cond.reason.as_deref().unwrap_or("-")
                ));
            }
        }
    }

    lines.push(String::new());
    lines.push("Events:  <use 'kubectl describe' for full events>".into());

    Ok(lines)
}

/// List TSH Kubernetes clusters via `tsh kube ls`.
pub async fn list_tsh_clusters() -> Vec<String> {
    let output = tokio::process::Command::new("tsh")
        .args(["kube", "ls"])
        .output()
        .await;

    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            parse_tsh_kube_ls(&text)
        }
        Err(_) => vec!["(tsh not found)".into()],
    }
}

fn parse_tsh_kube_ls(output: &str) -> Vec<String> {
    // tsh kube ls output:
    //  Kube Cluster Name   Labels   Selected
    //  ------------------  -------  --------
    //  cluster-prod                 *
    //  cluster-staging
    output
        .lines()
        .skip_while(|l| !l.starts_with("--"))
        .skip(1) // skip the dashes line itself
        .filter_map(|line| {
            let name = line.split_whitespace().next()?;
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

/// Switch to a TSH Kubernetes cluster (`tsh kube login <cluster>`).
pub async fn tsh_kube_login(cluster: &str) -> Result<()> {
    let status = tokio::process::Command::new("tsh")
        .args(["kube", "login", cluster])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("tsh kube login failed for cluster: {cluster}");
    }
    Ok(())
}

/// Stream `tail -1000f <log_path>` from inside a pod via `kubectl exec`.
/// Returns a channel receiver; drop it to stop the background task.
pub async fn stream_exec_tail(namespace: &str, pod: &str, log_path: &str) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel(2000);
    let pod = pod.to_string();
    let namespace = namespace.to_string();
    let log_path = log_path.to_string();

    tokio::spawn(async move {
        let child = tokio::process::Command::new("kubectl")
            .args([
                "exec",
                &pod,
                "-n",
                &namespace,
                "--",
                "tail",
                "-1000f",
                &log_path,
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        match child {
            Ok(mut child) => {
                if let Some(stdout) = child.stdout.take() {
                    use tokio::io::AsyncBufReadExt;
                    let reader = tokio::io::BufReader::new(stdout);
                    let mut lines = reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        if tx.send(line).await.is_err() {
                            break;
                        }
                    }
                }
                let _ = child.wait().await;
            }
            Err(e) => {
                let _ = tx.send(format!("Error starting kubectl exec: {e}")).await;
            }
        }
    });

    rx
}

/// Run `helm list -n <namespace>` and return lines for the viewer.
pub async fn list_helm_releases(namespace: &str) -> Vec<String> {
    let output = tokio::process::Command::new("helm")
        .args(["list", "-n", namespace])
        .output()
        .await;

    match output {
        Ok(out) => {
            if out.stdout.is_empty() {
                vec![format!("No Helm releases in namespace '{namespace}'")]
            } else {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(String::from)
                    .collect()
            }
        }
        Err(e) => vec![format!("Error running helm: {e}")],
    }
}

// ── Deployments ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DeploymentInfo {
    pub name: String,
    pub ready: String,     // "2/3"
    pub up_to_date: i32,
    pub available: i32,
    pub age: String,
    pub image: String,
    pub replicas: i32,
}

pub async fn list_deployments(client: &Client, namespace: &str) -> Result<Vec<DeploymentInfo>> {
    let api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&ListParams::default()).await?;

    let items = list
        .items
        .iter()
        .map(|d| {
            let meta = &d.metadata;
            let spec = d.spec.as_ref();
            let status = d.status.as_ref();

            let name = meta.name.clone().unwrap_or_default();
            let replicas = spec.and_then(|s| s.replicas).unwrap_or(0);
            let ready = status.and_then(|s| s.ready_replicas).unwrap_or(0);
            let up_to_date = status.and_then(|s| s.updated_replicas).unwrap_or(0);
            let available = status.and_then(|s| s.available_replicas).unwrap_or(0);

            let image = spec
                .and_then(|s| s.template.spec.as_ref())
                .and_then(|s| s.containers.first())
                .and_then(|c| c.image.clone())
                .unwrap_or_default();

            let age = meta
                .creation_timestamp
                .as_ref()
                .map(|ts| format_age(ts.0))
                .unwrap_or_else(|| "?".into());

            DeploymentInfo {
                name,
                ready: format!("{ready}/{replicas}"),
                up_to_date,
                available,
                age,
                image,
                replicas,
            }
        })
        .collect();

    Ok(items)
}

pub async fn describe_deployment(
    client: &Client,
    namespace: &str,
    name: &str,
) -> Result<Vec<String>> {
    let api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let d = api.get(name).await?;
    let mut lines = Vec::new();

    let meta = &d.metadata;
    let spec = d.spec.as_ref();
    let status = d.status.as_ref();

    lines.push(format!("Name:        {}", meta.name.as_deref().unwrap_or("-")));
    lines.push(format!("Namespace:   {namespace}"));
    lines.push(format!(
        "Replicas:    {} desired | {} ready | {} available",
        spec.and_then(|s| s.replicas).unwrap_or(0),
        status.and_then(|s| s.ready_replicas).unwrap_or(0),
        status.and_then(|s| s.available_replicas).unwrap_or(0),
    ));
    lines.push(format!(
        "Strategy:    {}",
        spec.and_then(|s| s.strategy.as_ref())
            .and_then(|s| s.type_.as_deref())
            .unwrap_or("-")
    ));

    if let Some(labels) = &meta.labels {
        lines.push("Labels:".into());
        for (k, v) in labels {
            lines.push(format!("  {k}={v}"));
        }
    }

    if let Some(spec) = spec {
        if let Some(selector) = &spec.selector.match_labels {
            lines.push("Selector:".into());
            for (k, v) in selector {
                lines.push(format!("  {k}={v}"));
            }
        }

        lines.push(String::new());
        lines.push("Pod Template:".into());
        if let Some(pod_spec) = &spec.template.spec {
            for c in &pod_spec.containers {
                lines.push(format!("  Container: {}", c.name));
                lines.push(format!("    Image: {}", c.image.as_deref().unwrap_or("-")));
                if let Some(ports) = &c.ports {
                    for p in ports {
                        lines.push(format!(
                            "    Port: {}/{}",
                            p.container_port,
                            p.protocol.as_deref().unwrap_or("TCP")
                        ));
                    }
                }
                if let Some(res) = &c.resources {
                    if let Some(req) = &res.requests {
                        lines.push("    Requests:".into());
                        for (k, v) in req {
                            lines.push(format!("      {k}: {}", v.0));
                        }
                    }
                    if let Some(lim) = &res.limits {
                        lines.push("    Limits:".into());
                        for (k, v) in lim {
                            lines.push(format!("      {k}: {}", v.0));
                        }
                    }
                }
            }
        }
    }

    if let Some(status) = status {
        if let Some(conditions) = &status.conditions {
            lines.push(String::new());
            lines.push("Conditions:".into());
            for c in conditions {
                lines.push(format!(
                    "  {}: {} — {}",
                    c.type_,
                    c.status,
                    c.message.as_deref().unwrap_or("-")
                ));
            }
        }
    }

    Ok(lines)
}

/// Patch spec.replicas to scale a deployment.
pub async fn scale_deployment(
    client: &Client,
    namespace: &str,
    name: &str,
    replicas: i32,
) -> Result<()> {
    let api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let patch = serde_json::json!({ "spec": { "replicas": replicas } });
    api.patch(name, &PatchParams::apply("kubeview"), &Patch::Merge(&patch))
        .await?;
    Ok(())
}

/// Rollout restart via kubectl (adds restartedAt annotation).
pub async fn rollout_restart(namespace: &str, name: &str) -> Result<()> {
    let status = tokio::process::Command::new("kubectl")
        .args(["rollout", "restart", &format!("deployment/{name}"), "-n", namespace])
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("kubectl rollout restart failed for {name}");
    }
    Ok(())
}

// ── Events ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EventInfo {
    pub last_seen: String,
    pub type_: String,   // "Normal" or "Warning"
    pub reason: String,
    pub object: String,  // "Pod/my-pod-xyz"
    pub message: String,
    pub count: i32,
}

pub async fn list_events(client: &Client, namespace: &str) -> Result<Vec<EventInfo>> {
    let api: Api<KubeEvent> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&ListParams::default()).await?;

    let mut events: Vec<(i64, EventInfo)> = list
        .items
        .iter()
        .map(|e| {
            let last_seen = e
                .last_timestamp
                .as_ref()
                .map(|t| format_age(t.0))
                .or_else(|| {
                    e.metadata
                        .creation_timestamp
                        .as_ref()
                        .map(|t| format_age(t.0))
                })
                .unwrap_or_else(|| "?".into());

            let sort_key = e
                .last_timestamp
                .as_ref()
                .map(|t| t.0.timestamp())
                .unwrap_or(0);

            let obj = &e.involved_object;
            let object = format!(
                "{}/{}",
                obj.kind.as_deref().unwrap_or("?"),
                obj.name.as_deref().unwrap_or("?")
            );

            let info = EventInfo {
                last_seen,
                type_: e.type_.clone().unwrap_or_else(|| "Normal".into()),
                reason: e.reason.clone().unwrap_or_default(),
                object,
                message: e.message.clone().unwrap_or_default(),
                count: e.count.unwrap_or(1),
            };

            (sort_key, info)
        })
        .collect();

    // Most recent first
    events.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(events.into_iter().map(|(_, e)| e).collect())
}

// ── YAML viewer ───────────────────────────────────────────────────────────────

/// Spawn a kubectl port-forward and return the child process handle.
pub async fn spawn_port_forward(
    namespace: &str,
    pod: &str,
    local_port: u16,
    remote_port: u16,
) -> anyhow::Result<tokio::process::Child> {
    let child = tokio::process::Command::new("kubectl")
        .args([
            "port-forward",
            pod,
            &format!("{local_port}:{remote_port}"),
            "-n",
            namespace,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(child)
}

/// Fetch raw YAML for any resource via kubectl.
pub async fn get_resource_yaml(kind: &str, name: &str, namespace: &str) -> Vec<String> {
    let output = tokio::process::Command::new("kubectl")
        .args(["get", kind, name, "-n", namespace, "-o", "yaml"])
        .output()
        .await;

    match output {
        Ok(out) => {
            if out.stdout.is_empty() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                vec![format!("Error: {stderr}")]
            } else {
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .map(String::from)
                    .collect()
            }
        }
        Err(e) => vec![format!("Error running kubectl: {e}")],
    }
}

// ── Ingresses ─────────────────────────────────────────────────────────────────

pub async fn list_ingresses(client: &Client, namespace: &str) -> Result<Vec<IngressInfo>> {
    let api: Api<Ingress> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&ListParams::default()).await?;

    let items = list
        .items
        .iter()
        .map(|ing| {
            let meta = &ing.metadata;
            let name = meta.name.clone().unwrap_or_default();
            let age = meta
                .creation_timestamp
                .as_ref()
                .map(|ts| format_age(ts.0))
                .unwrap_or_else(|| "?".into());

            let rules = ing.spec.as_ref().and_then(|s| s.rules.as_ref());

            let hosts = rules
                .map(|rs| {
                    rs.iter()
                        .filter_map(|r| r.host.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_else(|| "*".into());

            let paths = rules
                .map(|rs| {
                    rs.iter()
                        .flat_map(|r| {
                            let host = r.host.as_deref().unwrap_or("*");
                            r.http
                                .as_ref()
                                .map(|h| {
                                    h.paths
                                        .iter()
                                        .map(|p| {
                                            let path = p.path.as_deref().unwrap_or("/");
                                            let svc = p
                                                .backend
                                                .service
                                                .as_ref()
                                                .map(|s| {
                                                    let port = s
                                                        .port
                                                        .as_ref()
                                                        .and_then(|p| p.number)
                                                        .map(|n| n.to_string())
                                                        .unwrap_or_else(|| "?".into());
                                                    format!("{}:{}", s.name, port)
                                                })
                                                .unwrap_or_else(|| "?".into());
                                            format!("{host}{path} → {svc}")
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default()
                        })
                        .collect::<Vec<_>>()
                        .join("  ")
                })
                .unwrap_or_default();

            IngressInfo { name, hosts, paths, age }
        })
        .collect();

    Ok(items)
}

pub async fn describe_ingress(client: &Client, namespace: &str, name: &str) -> Vec<String> {
    let api: Api<Ingress> = Api::namespaced(client.clone(), namespace);
    match api.get(name).await {
        Err(e) => vec![format!("Error: {e}")],
        Ok(ing) => {
            let mut lines = Vec::new();
            let meta = &ing.metadata;

            lines.push(format!("Name:       {}", meta.name.as_deref().unwrap_or("-")));
            lines.push(format!("Namespace:  {namespace}"));
            lines.push(format!(
                "Age:        {}",
                meta.creation_timestamp
                    .as_ref()
                    .map(|t| format_age(t.0))
                    .unwrap_or_else(|| "-".into())
            ));

            if let Some(labels) = &meta.labels {
                if !labels.is_empty() {
                    lines.push("Labels:".into());
                    for (k, v) in labels {
                        lines.push(format!("  {k}={v}"));
                    }
                }
            }

            if let Some(annotations) = &meta.annotations {
                let relevant: Vec<_> = annotations
                    .iter()
                    .filter(|(k, _)| !k.starts_with("kubectl.kubernetes.io"))
                    .collect();
                if !relevant.is_empty() {
                    lines.push("Annotations:".into());
                    for (k, v) in &relevant {
                        lines.push(format!("  {k}: {v}"));
                    }
                }
            }

            if let Some(spec) = &ing.spec {
                if let Some(class) = &spec.ingress_class_name {
                    lines.push(format!("IngressClass: {class}"));
                }

                if let Some(tls_list) = &spec.tls {
                    lines.push(String::new());
                    lines.push("TLS:".into());
                    for tls in tls_list {
                        let secret = tls.secret_name.as_deref().unwrap_or("<no secret>");
                        let hosts = tls
                            .hosts
                            .as_ref()
                            .map(|h| h.join(", "))
                            .unwrap_or_else(|| "*".into());
                        lines.push(format!("  {hosts} → secret/{secret}"));
                    }
                }

                if let Some(rules) = &spec.rules {
                    lines.push(String::new());
                    lines.push("Rules:".into());
                    lines.push(format!("  {:<40} {:<25} {}", "HOST", "PATH", "BACKEND"));
                    lines.push(format!("  {}", "-".repeat(80)));
                    for rule in rules {
                        let host = rule.host.as_deref().unwrap_or("*");
                        if let Some(http) = &rule.http {
                            for path in &http.paths {
                                let p = path.path.as_deref().unwrap_or("/");
                                let backend = path
                                    .backend
                                    .service
                                    .as_ref()
                                    .map(|s| {
                                        let port = s
                                            .port
                                            .as_ref()
                                            .and_then(|p| p.number)
                                            .map(|n| n.to_string())
                                            .unwrap_or_else(|| "?".into());
                                        format!("{}:{}", s.name, port)
                                    })
                                    .unwrap_or_else(|| "?".into());
                                let path_type = path.path_type.as_str();
                                lines.push(format!(
                                    "  {:<40} {:<25} {}  ({})",
                                    host, p, backend, path_type
                                ));
                            }
                        } else {
                            lines.push(format!("  {host}  (no HTTP rules)"));
                        }
                    }
                }

                if let Some(backend) = &spec.default_backend {
                    lines.push(String::new());
                    if let Some(svc) = &backend.service {
                        let port = svc
                            .port
                            .as_ref()
                            .and_then(|p| p.number)
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "?".into());
                        lines.push(format!("Default backend: {}:{}", svc.name, port));
                    }
                }
            }
            lines
        }
    }
}

// ── Secrets ───────────────────────────────────────────────────────────────────

pub async fn list_secrets(client: &Client, namespace: &str) -> Result<Vec<SecretInfo>> {
    let api: Api<Secret> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&ListParams::default()).await?;

    let items = list
        .items
        .iter()
        .map(|s| {
            let meta = &s.metadata;
            let name = meta.name.clone().unwrap_or_default();
            let type_ = s.type_.clone().unwrap_or_else(|| "Opaque".into());
            let key_names: Vec<String> = s
                .data
                .as_ref()
                .map(|d| d.keys().cloned().collect())
                .unwrap_or_default();
            let keys = key_names.len();
            let age = meta
                .creation_timestamp
                .as_ref()
                .map(|ts| format_age(ts.0))
                .unwrap_or_else(|| "?".into());
            SecretInfo { name, type_, keys, key_names, age }
        })
        .collect();

    Ok(items)
}

/// Fetch the decoded value of a single secret key (for pre-filling the edit buffer).
pub async fn get_secret_key_raw(client: &Client, namespace: &str, name: &str, key: &str) -> String {
    let api: Api<Secret> = Api::namespaced(client.clone(), namespace);
    match api.get(name).await {
        Err(_) => String::new(),
        Ok(secret) => secret
            .data
            .as_ref()
            .and_then(|d| d.get(key))
            .and_then(|v| String::from_utf8(v.0.clone()).ok())
            .unwrap_or_default(),
    }
}

/// Update a single key in a secret using `stringData` (k8s handles base64 encoding).
pub async fn update_secret_key(
    client: &Client,
    namespace: &str,
    name: &str,
    key: &str,
    plain_value: &str,
) -> Result<()> {
    let api: Api<Secret> = Api::namespaced(client.clone(), namespace);
    let patch = serde_json::json!({ "stringData": { key: plain_value } });
    api.patch(name, &PatchParams::apply("kubeview"), &Patch::Merge(&patch))
        .await?;
    Ok(())
}

/// Fetch a secret and return decoded key→value lines for the viewer.
pub async fn get_secret_data(client: &Client, namespace: &str, name: &str) -> Vec<String> {
    let api: Api<Secret> = Api::namespaced(client.clone(), namespace);
    match api.get(name).await {
        Err(e) => vec![format!("Error fetching secret: {e}")],
        Ok(secret) => {
            let mut lines = vec![
                format!("Secret: {name}"),
                format!("Namespace: {namespace}"),
                format!("Type: {}", secret.type_.as_deref().unwrap_or("Opaque")),
                String::new(),
                "── Data (base64-decoded) ─────────────────────".into(),
            ];
            if let Some(data) = &secret.data {
                for (key, val) in data {
                    let decoded = String::from_utf8(val.0.clone())
                        .unwrap_or_else(|_| format!("<binary {} bytes>", val.0.len()));
                    lines.push(format!("{key}:"));
                    // Indent multi-line values
                    for l in decoded.lines() {
                        lines.push(format!("  {l}"));
                    }
                    lines.push(String::new());
                }
            } else {
                lines.push("  (no data)".into());
            }
            if let Some(string_data) = &secret.string_data {
                if !string_data.is_empty() {
                    lines.push("── StringData ────────────────────────────────".into());
                    for (key, val) in string_data {
                        lines.push(format!("{key}:"));
                        for l in val.lines() {
                            lines.push(format!("  {l}"));
                        }
                        lines.push(String::new());
                    }
                }
            }
            lines
        }
    }
}

fn format_age(created: chrono::DateTime<chrono::Utc>) -> String {
    let duration = chrono::Utc::now() - created;
    let secs = duration.num_seconds();

    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}
