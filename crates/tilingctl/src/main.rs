use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};
use tiling_ipc::{Client, Event, Query, Request, Response};

/// Control a running `tiling` window manager.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a dispatcher, e.g. `tilingctl dispatch workspace 3`.
    Dispatch {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Print information about the window manager.
    Query {
        what: What,
        /// Print raw JSON.
        #[arg(long, short)]
        json: bool,
    },
    /// Print an event line (JSON) whenever the workspace, active window or layout changes.
    Subscribe,
    /// Reload the config file.
    Reload,
}

#[derive(Clone, Copy, ValueEnum)]
enum What {
    Monitors,
    Workspaces,
    Windows,
    Activewindow,
}

impl From<What> for Query {
    fn from(what: What) -> Self {
        match what {
            What::Monitors => Self::Monitors,
            What::Workspaces => Self::Workspaces,
            What::Windows => Self::Windows,
            What::Activewindow => Self::ActiveWindow,
        }
    }
}

fn print_human(response: &Response) {
    match response {
        Response::Ok => {}
        Response::Error { message } => eprintln!("error: {message}"),
        Response::Monitors { monitors } => {
            for m in monitors {
                let focus = if m.focused { " (focused)" } else { "" };
                println!(
                    "monitor {}: {}x{} at {},{} workspace {}{focus}",
                    m.index, m.rect.w, m.rect.h, m.rect.x, m.rect.y, m.workspace
                );
            }
        }
        Response::Workspaces { workspaces } => {
            for w in workspaces {
                let monitor = w.monitor.map_or_else(|| "-".into(), |m| m.to_string());
                let flags = match (w.visible, w.focused) {
                    (_, true) => " (focused)",
                    (true, false) => " (visible)",
                    _ => "",
                };
                println!("workspace {}: monitor {monitor}, {} windows, {}{flags}", w.id, w.windows, w.layout);
            }
        }
        Response::Windows { windows } => {
            for w in windows {
                let mut flags = Vec::new();
                for (on, name) in [(w.focused, "focused"), (w.floating, "floating"), (w.fullscreen, "fullscreen"), (w.hidden, "hidden")] {
                    if on {
                        flags.push(name);
                    }
                }
                println!("{:#x} [ws {}] {} — {} ({}) {}", w.id, w.workspace, w.exe, w.title, w.class, flags.join(","));
            }
        }
        Response::ActiveWindow { window } => match window {
            Some(w) => println!("{:#x} [ws {}] {} — {}", w.id, w.workspace, w.exe, w.title),
            None => println!("no active window"),
        },
        Response::Event { event } => print_event(event),
    }
}

fn print_event(event: &Event) {
    println!("{}", serde_json::to_string(event).unwrap_or_default());
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let (request, json) = match args.command {
        Cmd::Dispatch { command } => (Request::Dispatch { command: command.join(" ") }, false),
        Cmd::Query { what, json } => (Request::Query { what: what.into() }, json),
        Cmd::Subscribe => (Request::Subscribe, true),
        Cmd::Reload => (Request::Dispatch { command: "reload".into() }, false),
    };
    let mut client = Client::send(&request).await.context("is tiling running?")?;
    let mut failed = false;
    while let Some(response) = client.next().await? {
        failed |= matches!(response, Response::Error { .. });
        match (&response, json) {
            (Response::Event { event }, _) => print_event(event),
            (Response::Ok, _) => {}
            (_, true) => println!("{}", serde_json::to_string_pretty(&response)?),
            (_, false) => print_human(&response),
        }
    }
    if failed {
        bail!("request failed");
    }
    Ok(())
}
