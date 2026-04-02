mod app;
mod k8s;
mod theme;
mod ui;

use anyhow::Result;
use app::{App, Tab};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
}

#[tokio::main]
async fn main() -> Result<()> {
    // Handle --version / -V before doing anything with the terminal
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("kubeview {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Panic hook: always restore terminal so the shell isn't left in raw mode
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal).await;

    restore_terminal();
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {err:?}");
    }

    Ok(())
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    // Build the kube client with a timeout so a hung TSH credential helper
    // doesn't freeze the terminal. Ctrl+C works as soon as the event loop starts.
    let app_result = tokio::time::timeout(
        Duration::from_secs(15),
        App::new(),
    ).await;

    let mut app = match app_result {
        Ok(Ok(a)) => a,
        Ok(Err(e)) => {
            restore_terminal();
            eprintln!("Failed to connect to Kubernetes: {e}");
            eprintln!("\nIf using Teleport, run:  tsh login && tsh kube login <cluster>");
            return Ok(());
        }
        Err(_) => {
            restore_terminal();
            eprintln!("Timed out connecting to Kubernetes (15s).");
            eprintln!("\nIf using Teleport, run:  tsh login && tsh kube login <cluster>");
            return Ok(());
        }
    };

    if let Err(e) = app.initial_load().await {
        app.status_message = format!("Load error: {e}");
    }

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            if area.width < 40 || area.height < 10 {
                frame.render_widget(
                    ratatui::widgets::Paragraph::new("Terminal too small. Resize to at least 40x10."),
                    area,
                );
                return;
            }
            ui::draw(frame, &app);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    app.tick().await?;
                    continue;
                }

                match app.input_mode {
                    app::InputMode::Normal => {
                        let tab = Tab::ALL[app.active_tab];
                        match key.code {
                            KeyCode::Char('q') => {
                                app.cleanup_port_forwards().await;
                                return Ok(());
                            }
                            KeyCode::Char('c')
                                if key.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                app.cleanup_port_forwards().await;
                                return Ok(());
                            }

                            // Navigation
                            KeyCode::Char('j') | KeyCode::Down => app.next_item(),
                            KeyCode::Char('k') | KeyCode::Up => app.prev_item(),
                            KeyCode::Char('G') => app.jump_bottom(),
                            KeyCode::Char('g') => app.jump_top(),

                            // Namespace / cluster
                            KeyCode::Char('n') => app.next_namespace(),
                            KeyCode::Char('c') => app.open_cluster_picker(),

                            // Search (pods only)
                            KeyCode::Char('/') if tab == Tab::Pods => app.enter_search(),

                            // Viewers
                            KeyCode::Char('l') if tab == Tab::Pods => app.view_logs().await?,
                            KeyCode::Char('y') => app.view_yaml().await,
                            KeyCode::Char('d') => app.describe_selected().await?,
                            KeyCode::Char('H') => app.view_helm_list().await,

                            // Exec into pod (pods tab only) / edit secret key
                            KeyCode::Char('e') if tab == Tab::Pods => app.open_exec(),
                            KeyCode::Char('e') if tab == Tab::Secrets => app.open_secret_edit(),

                            // Port-forward
                            KeyCode::Char('p') if tab == Tab::Pods => app.prompt_port_forward(),
                            KeyCode::Char('P') => app.view_port_forwards(),

                            // Tab-specific actions
                            KeyCode::Char('s') if tab == Tab::Pods => {
                                app.prompt_service_log_path()
                            }
                            KeyCode::Char('s') if tab == Tab::Deployments => app.prompt_scale(),
                            KeyCode::Char('r') if tab == Tab::Deployments => {
                                app.request_rollout_restart()
                            }

                            // Refresh
                            KeyCode::Char('r') => app.schedule_current_tab_refresh(),

                            // Theme cycle
                            KeyCode::Char('T') => app.cycle_theme(),

                            // Secrets tab: Enter or d = decoded view
                            KeyCode::Enter if tab == Tab::Secrets => {
                                app.view_secret_decoded().await
                            }

                            // Help
                            KeyCode::Char('?') => {
                                app.input_mode = app::InputMode::Help;
                            }

                            // Tabs
                            KeyCode::Tab => {
                                app.next_tab();
                                app.schedule_current_tab_refresh();
                            }
                            KeyCode::BackTab => {
                                app.prev_tab();
                                app.schedule_current_tab_refresh();
                            }
                            KeyCode::Char('1') => {
                                app.select_tab(0);
                                app.schedule_current_tab_refresh();
                            }
                            KeyCode::Char('2') => {
                                app.select_tab(1);
                                app.schedule_current_tab_refresh();
                            }
                            KeyCode::Char('3') => {
                                app.select_tab(2);
                                app.schedule_current_tab_refresh();
                            }
                            KeyCode::Char('4') => {
                                app.select_tab(3);
                                app.schedule_current_tab_refresh();
                            }
                            KeyCode::Char('5') => {
                                app.select_tab(4);
                                app.schedule_current_tab_refresh();
                            }
                            KeyCode::Char('6') => {
                                app.select_tab(5);
                                app.schedule_current_tab_refresh();
                            }

                            _ => {}
                        }
                    }

                    app::InputMode::Search => match key.code {
                        KeyCode::Esc | KeyCode::Enter => app.exit_search(),
                        KeyCode::Backspace => {
                            app.search_query.pop();
                            app.apply_filter();
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                            app.apply_filter();
                        }
                        _ => {}
                    },

                    app::InputMode::Viewing => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => app.close_viewer(),
                        KeyCode::Char('j') | KeyCode::Down => app.scroll_viewer_down(),
                        KeyCode::Char('k') | KeyCode::Up => app.scroll_viewer_up(),
                        KeyCode::Char('G') => app.scroll_viewer_bottom(),
                        KeyCode::Char('g') => app.scroll_viewer_top(),
                        _ => {}
                    },

                    app::InputMode::ClusterPicker => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => app.close_cluster_picker(),
                        KeyCode::Char('j') | KeyCode::Down => app.cluster_picker_next(),
                        KeyCode::Char('k') | KeyCode::Up => app.cluster_picker_prev(),
                        KeyCode::Enter => app.confirm_cluster_selection().await?,
                        _ => {}
                    },

                    app::InputMode::ContainerPicker => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.input_mode = app::InputMode::Normal;
                        }
                        KeyCode::Char('j') | KeyCode::Down => app.container_picker_next(),
                        KeyCode::Char('k') | KeyCode::Up => app.container_picker_prev(),
                        KeyCode::Enter => app.confirm_container_for_exec(),
                        _ => {}
                    },

                    app::InputMode::Help => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                            app.input_mode = app::InputMode::Normal;
                        }
                        _ => {}
                    },

                    app::InputMode::PathInput => match key.code {
                        KeyCode::Esc => app.input_mode = app::InputMode::Normal,
                        KeyCode::Enter => app.confirm_service_log_path().await?,
                        KeyCode::Backspace => {
                            app.path_input_buffer.pop();
                        }
                        KeyCode::Char(c) => app.path_input_buffer.push(c),
                        _ => {}
                    },

                    app::InputMode::ScaleInput => match key.code {
                        KeyCode::Esc => app.input_mode = app::InputMode::Normal,
                        KeyCode::Enter => app.confirm_scale().await?,
                        KeyCode::Backspace => {
                            app.scale_input_buffer.pop();
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            app.scale_input_buffer.push(c)
                        }
                        _ => {}
                    },

                    app::InputMode::PortInput => match key.code {
                        KeyCode::Esc => app.input_mode = app::InputMode::Normal,
                        KeyCode::Enter => app.confirm_port_forward().await?,
                        KeyCode::Backspace => {
                            app.port_input_buffer.pop();
                        }
                        KeyCode::Char(c) => app.port_input_buffer.push(c),
                        _ => {}
                    },

                    app::InputMode::SecretKeyPicker => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            app.input_mode = app::InputMode::Normal;
                        }
                        KeyCode::Char('j') | KeyCode::Down => app.secret_key_picker_next(),
                        KeyCode::Char('k') | KeyCode::Up => app.secret_key_picker_prev(),
                        KeyCode::Enter => app.confirm_secret_key_selection().await,
                        _ => {}
                    },

                    app::InputMode::SecretValueInput => match key.code {
                        KeyCode::Esc => app.input_mode = app::InputMode::Normal,
                        KeyCode::Enter => app.confirm_secret_value_edit().await?,
                        KeyCode::Backspace => {
                            app.secret_value_input_buffer.pop();
                        }
                        KeyCode::Char(c) => app.secret_value_input_buffer.push(c),
                        _ => {}
                    },

                    app::InputMode::Confirm => match key.code {
                        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => {
                            app.cancel_confirm()
                        }
                        KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                            app.confirm_yes = !app.confirm_yes;
                        }
                        KeyCode::Enter | KeyCode::Char('y') => {
                            if app.confirm_yes {
                                app.execute_confirm().await?;
                            } else {
                                app.cancel_confirm();
                            }
                        }
                        _ => {}
                    },
                }
            }
        }

        // Handle pending exec: swap terminal, run kubectl exec, restore
        if let Some((pod, ns, container)) = app.pending_exec.take() {
            exec_into_pod(terminal, &pod, &ns, &container).await?;
        }

        app.tick().await?;
    }
}

/// Suspend the TUI, hand terminal to kubectl exec, then restore.
async fn exec_into_pod(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    pod: &str,
    ns: &str,
    container: &str,
) -> Result<()> {
    // Restore terminal to normal state
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    println!("\n── exec: {pod} / {container} ──  (type 'exit' to return)\n");

    // Try bash first, fall back to sh
    let bash = tokio::process::Command::new("kubectl")
        .args(["exec", "-it", pod, "-n", ns, "-c", container, "--", "bash"])
        .status()
        .await;

    if bash.map(|s| !s.success()).unwrap_or(true) {
        let _ = tokio::process::Command::new("kubectl")
            .args(["exec", "-it", pod, "-n", ns, "-c", container, "--", "sh"])
            .status()
            .await;
    }

    // Restore TUI
    enable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;
    terminal.clear()?;

    Ok(())
}
