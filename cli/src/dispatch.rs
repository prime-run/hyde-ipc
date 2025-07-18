use crate::flags::{ResizeCmd, WindowId};
use hyprland::dispatch::{
    Corner, CycleDirection, Direction, Dispatch, DispatchType, FullscreenType, MonitorIdentifier,
    Position, WindowIdentifier, WindowMove, WorkspaceIdentifierWithSpecial,
};
use hyprland::shared::Address;
use std::collections::HashMap;
use std::sync::LazyLock;

type DispatcherBuilder = fn(Vec<String>) -> Result<DispatchType<'static>, String>;

fn parse_window_identifier(identifier: WindowId) -> Result<WindowIdentifier<'static>, String> {
    if let Some(class) = identifier.class {
        let class_static = Box::leak(class.to_string().into_boxed_str());
        Ok(WindowIdentifier::ClassRegularExpression(class_static))
    } else if let Some(title) = identifier.title {
        let title_static = Box::leak(title.to_string().into_boxed_str());
        Ok(WindowIdentifier::Title(title_static))
    } else if let Some(pid) = identifier.pid {
        Ok(WindowIdentifier::ProcessId(pid))
    } else if let Some(addr) = identifier.address {
        Ok(WindowIdentifier::Address(Address::new(addr)))
    } else {
        Err("No window identifier provided".to_string())
    }
}

fn parse_workspace_identifier(
    workspace: &str,
) -> Result<WorkspaceIdentifierWithSpecial<'static>, String> {
    if let Ok(id) = workspace.parse::<i32>() {
        Ok(WorkspaceIdentifierWithSpecial::Id(id))
    } else if let Some(num_str) = workspace.strip_prefix("right:") {
        let num = num_str
            .parse::<i32>()
            .map_err(|_| format!("Invalid number for right: {num_str}"))?;
        Ok(WorkspaceIdentifierWithSpecial::Relative(num))
    } else if let Some(num_str) = workspace.strip_prefix("left:") {
        let num = num_str
            .parse::<i32>()
            .map_err(|_| format!("Invalid number for left: {num_str}"))?;
        Ok(WorkspaceIdentifierWithSpecial::Relative(-num))
    } else if workspace == "previous" {
        Ok(WorkspaceIdentifierWithSpecial::Previous)
    } else if workspace == "empty" {
        Ok(WorkspaceIdentifierWithSpecial::Empty)
    } else if let Some(name) = workspace.strip_prefix("name:") {
        let name_static = Box::leak(name.to_string().into_boxed_str());
        Ok(WorkspaceIdentifierWithSpecial::Name(name_static))
    } else {
        Err(format!("Unknown workspace identifier: {workspace}"))
    }
}

fn parse_direction(direction: &str) -> Result<Direction, String> {
    match direction.to_lowercase().as_str() {
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        _ => Err(format!("Unknown direction: {direction}")),
    }
}

fn parse_window_move(target: &str) -> Result<WindowMove<'static>, String> {
    if let Some(monitor_name) = target.strip_prefix("mon:") {
        let monitor_name_static = Box::leak(
            monitor_name
                .to_string()
                .into_boxed_str(),
        );
        Ok(WindowMove::Monitor(MonitorIdentifier::Name(monitor_name_static)))
    } else if let Ok(monitor_id) = target.parse::<i128>() {
        Ok(WindowMove::Monitor(MonitorIdentifier::Id(monitor_id)))
    } else if target.to_lowercase() == "current" {
        Ok(WindowMove::Monitor(MonitorIdentifier::Current))
    } else if let Ok(rel_num) = target.parse::<i32>() {
        Ok(WindowMove::Monitor(MonitorIdentifier::Relative(rel_num)))
    } else if let Some(dir_str) = target
        .to_lowercase()
        .strip_prefix("dir:")
    {
        let dir = parse_direction(dir_str)?;
        Ok(WindowMove::Direction(dir))
    } else {
        Err(format!("Unknown target for MoveWindow: {target}"))
    }
}

static DISPATCHERS: LazyLock<HashMap<&'static str, DispatcherBuilder>> = LazyLock::new(|| {
    let mut m: HashMap<&'static str, DispatcherBuilder> = HashMap::new();
    m.insert("exec", |args| {
        let command = args.join(" ");
        let command_static = Box::leak(command.into_boxed_str());
        Ok(DispatchType::Exec(command_static))
    });
    m.insert("killactivewindow", |_args| Ok(DispatchType::KillActiveWindow));
    m.insert("togglefloating", |args| {
        let window_str = args
            .first()
            .map(|s| s.as_str())
            .unwrap_or("");
        let window_id = if window_str.is_empty() {
            None
        } else {
            Some(parse_window_identifier(WindowId {
                class: Some(window_str.to_string()),
                ..Default::default()
            })?)
        };
        Ok(DispatchType::ToggleFloating(window_id))
    });
    m.insert("togglesplit", |_| Ok(DispatchType::ToggleSplit));
    m.insert("toggleopaque", |_| Ok(DispatchType::ToggleOpaque));
    m.insert("movecursortocorner", |args| {
        let corner_str = args
            .first()
            .ok_or("Missing corner argument")?;
        let corner = match corner_str.to_lowercase().as_str() {
            "topleft" => Corner::TopLeft,
            "topright" => Corner::TopRight,
            "bottomleft" => Corner::BottomLeft,
            "bottomright" => Corner::BottomRight,
            _ => return Err(format!("Unknown corner: {corner_str}")),
        };
        Ok(DispatchType::MoveCursorToCorner(corner))
    });
    m.insert("movecursor", |args| {
        if args.len() != 2 {
            return Err("movecursor requires x and y arguments".to_string());
        }
        let x = args[0]
            .parse::<i64>()
            .map_err(|_| "Invalid x value")?;
        let y = args[1]
            .parse::<i64>()
            .map_err(|_| "Invalid y value")?;
        Ok(DispatchType::MoveCursor(x, y))
    });
    m.insert("togglefullscreen", |args| {
        let mode_str = args
            .first()
            .map(|s| s.as_str())
            .unwrap_or("noparam");
        let mode = match mode_str.to_lowercase().as_str() {
            "real" => FullscreenType::Real,
            "maximize" => FullscreenType::Maximize,
            "noparam" => FullscreenType::NoParam,
            _ => return Err(format!("Unknown fullscreen type: {mode_str}")),
        };
        Ok(DispatchType::ToggleFullscreen(mode))
    });
    m.insert("movetoworkspace", |args| {
        let workspace_str = args
            .first()
            .ok_or("Missing workspace argument")?;
        let workspace_id = parse_workspace_identifier(workspace_str)?;
        Ok(DispatchType::MoveToWorkspace(workspace_id, None))
    });
    m.insert("workspace", |args| {
        let workspace_str = args
            .first()
            .ok_or("Missing workspace argument")?;
        let workspace_id = parse_workspace_identifier(workspace_str)?;
        Ok(DispatchType::Workspace(workspace_id))
    });
    m.insert("cyclewindow", |args| {
        let dir_str = args
            .first()
            .map(|s| s.as_str())
            .unwrap_or("next");
        let dir = match dir_str.to_lowercase().as_str() {
            "next" => CycleDirection::Next,
            "previous" => CycleDirection::Previous,
            _ => return Err(format!("Unknown cycle direction: {dir_str}")),
        };
        Ok(DispatchType::CycleWindow(dir))
    });
    m.insert("movefocus", |args| {
        let dir_str = args
            .first()
            .ok_or("Missing direction argument")?;
        let dir = parse_direction(dir_str)?;
        Ok(DispatchType::MoveFocus(dir))
    });
    m.insert("swapwindow", |args| {
        let dir_str = args
            .first()
            .ok_or("Missing direction argument")?;
        let dir = parse_direction(dir_str)?;
        Ok(DispatchType::SwapWindow(dir))
    });
    m.insert("focuswindow", |args| {
        let window_str = args
            .first()
            .ok_or("Missing window identifier")?;
        let window_id = parse_window_identifier(WindowId {
            class: Some(window_str.to_string()),
            ..Default::default()
        })?;
        Ok(DispatchType::FocusWindow(window_id))
    });
    m.insert("movewindow", |args| {
        let target_str = args
            .first()
            .ok_or("Missing target argument")?;
        let window_move = parse_window_move(target_str)?;
        Ok(DispatchType::MoveWindow(window_move))
    });
    m.insert("togglefakefullscreen", |_| Ok(DispatchType::ToggleFakeFullscreen));
    m.insert("togglepseudo", |_| Ok(DispatchType::TogglePseudo));
    m.insert("togglepin", |_| Ok(DispatchType::TogglePin));
    m.insert("centerwindow", |_| Ok(DispatchType::CenterWindow));
    m.insert("bringactivetotop", |_| Ok(DispatchType::BringActiveToTop));
    m.insert("focusurgentorlast", |_| Ok(DispatchType::FocusUrgentOrLast));
    m.insert("focuscurrentorlast", |_| Ok(DispatchType::FocusCurrentOrLast));
    m.insert("forcerendererreload", |_| Ok(DispatchType::ForceRendererReload));
    m.insert("exit", |_| Ok(DispatchType::Exit));
    m.insert("resizeactive", |args| {
        if args.is_empty() {
            return Err("resizeactive requires arguments".to_string());
        }
        let params = if args[0] == "exact" {
            ResizeCmd::Exact {
                width: args
                    .get(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                height: args
                    .get(2)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            }
        } else {
            ResizeCmd::Delta {
                dx: args
                    .first()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                dy: args
                    .get(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            }
        };
        let position = match params {
            ResizeCmd::Delta { dx, dy } => Position::Delta(dx, dy),
            ResizeCmd::Exact { width, height } => Position::Exact(width, height),
        };
        Ok(DispatchType::ResizeActive(position))
    });
    m.insert("resizewindowpixel", |args| {
        if args.is_empty() {
            return Err("resizewindowpixel requires arguments".to_string());
        }
        let (params, window_str) = if args[0] == "exact" {
            (
                ResizeCmd::Exact {
                    width: args
                        .get(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0),
                    height: args
                        .get(2)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0),
                },
                args.get(3)
                    .ok_or("Missing window identifier")?,
            )
        } else {
            (
                ResizeCmd::Delta {
                    dx: args
                        .first()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0),
                    dy: args
                        .get(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0),
                },
                args.get(2)
                    .ok_or("Missing window identifier")?,
            )
        };
        let position = match params {
            ResizeCmd::Delta { dx, dy } => Position::Delta(dx, dy),
            ResizeCmd::Exact { width, height } => Position::Exact(width, height),
        };
        let win_id = parse_window_identifier(WindowId {
            class: Some(window_str.to_string()),
            ..Default::default()
        })?;
        Ok(DispatchType::ResizeWindowPixel(position, win_id))
    });
    m
});

pub fn build_dispatch_cmd(
    dispatcher: &str,
    args: &[String],
) -> Result<DispatchType<'static>, String> {
    let lower_dispatcher = dispatcher.to_lowercase();
    let args_owned = args
        .iter()
        .map(|s| s.to_string())
        .collect();
    DISPATCHERS
        .get(lower_dispatcher.as_str())
        .ok_or_else(|| format!("Unknown dispatcher: {dispatcher}"))
        .and_then(|builder| builder(args_owned))
}

/// Synchronously execute a dispatcher.
///
/// # Arguments
/// * `dispatcher` - The dispatcher name.
/// * `args` - Arguments for the dispatcher.
pub fn sync_dispatch(dispatcher: &str, args: &[String]) {
    match build_dispatch_cmd(dispatcher, args) {
        Ok(dispatch_type) => {
            if let Err(e) = Dispatch::call(dispatch_type) {
                eprintln!("Error: {e}");
            }
        },
        Err(e) => {
            eprintln!("Error: {e}");
        },
    }
}

/// Asynchronously execute a dispatcher.
///
/// # Arguments
/// * `dispatcher` - The dispatcher name.
/// * `args` - Arguments for the dispatcher.
pub async fn async_dispatch(dispatcher: &str, args: &[String]) {
    match build_dispatch_cmd(dispatcher, args) {
        Ok(dispatch_type) => match Dispatch::call_async(dispatch_type).await {
            Ok(_) => {
                println!("Async dispatch completed successfully");
            },
            Err(e) => {
                eprintln!("Error: {e}");
            },
        },
        Err(e) => {
            eprintln!("Error: {e}");
        },
    }
}
