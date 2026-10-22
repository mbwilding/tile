//! Line-delimited JSON over the `\\.\pipe\tiling.sock` named pipe.
//!
//! A client sends one [`Request`] line and reads [`Response`] lines until the server closes
//! the connection (or forever, for [`Request::Subscribe`]).

use std::io;

use interprocess::local_socket::{
    GenericNamespaced, ListenerOptions, Name, ToNsName,
    tokio::{Listener, Stream, prelude::*},
};
use serde::{Deserialize, Serialize};
use tiling_core::{
    LayoutKind,
    state::{MonitorSnapshot, WindowSnapshot, WorkspaceSnapshot},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
};

pub const SOCKET_NAME: &str = "tiling.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Run a dispatcher, e.g. `workspace 3`.
    Dispatch { command: String },
    Query { what: Query },
    /// Stream [`Event`]s until the client disconnects.
    Subscribe,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Query {
    Monitors,
    Workspaces,
    Windows,
    ActiveWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Error { message: String },
    Monitors { monitors: Vec<MonitorSnapshot> },
    Workspaces { workspaces: Vec<WorkspaceSnapshot> },
    Windows { windows: Vec<WindowSnapshot> },
    ActiveWindow { window: Option<WindowSnapshot> },
    Event { event: Event },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Workspace { id: u8 },
    ActiveWindow { id: Option<isize>, title: Option<String>, exe: Option<String> },
    Layout { layout: LayoutKind },
}

/// A request received by the server, with a channel for the responses.
/// Dropping `reply` closes the client connection.
#[derive(Debug)]
pub struct Incoming {
    pub request: Request,
    pub reply: mpsc::UnboundedSender<Response>,
}

fn socket_name() -> io::Result<Name<'static>> {
    SOCKET_NAME.to_ns_name::<GenericNamespaced>()
}

/// Whether a daemon is already listening.
pub async fn is_running() -> bool {
    match socket_name() {
        Ok(name) => Stream::connect(name).await.is_ok(),
        Err(_) => false,
    }
}

/// The daemon side of the pipe. Must be created inside a Tokio runtime.
pub struct Server(Listener);

impl Server {
    pub fn bind() -> io::Result<Self> {
        ListenerOptions::new().name(socket_name()?).create_tokio().map(Self)
    }

    /// Accepts connections forever, forwarding each request to `tx`.
    pub async fn serve(self, tx: mpsc::UnboundedSender<Incoming>) {
        serve(self.0, tx).await;
    }
}

async fn serve(listener: Listener, tx: mpsc::UnboundedSender<Incoming>) {
    loop {
        let conn = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::warn!("ipc accept failed: {e}");
                continue;
            }
        };
        let tx = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(conn, tx).await {
                tracing::debug!("ipc connection ended: {e}");
            }
        });
    }
}

async fn handle(conn: Stream, tx: mpsc::UnboundedSender<Incoming>) -> io::Result<()> {
    let mut reader = BufReader::new(&conn);
    let mut writer = &conn;
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let (reply, mut replies) = mpsc::unbounded_channel();
    match serde_json::from_str::<Request>(&line) {
        Ok(request) => {
            if tx.send(Incoming { request, reply }).is_err() {
                return Ok(());
            }
        }
        Err(e) => {
            let _ = reply.send(Response::Error { message: format!("bad request: {e}") });
        }
    }
    while let Some(response) = replies.recv().await {
        let mut json = serde_json::to_string(&response).map_err(io::Error::other)?;
        json.push('\n');
        writer.write_all(json.as_bytes()).await?;
    }
    writer.flush().await
}

/// A connection to the daemon.
pub struct Client {
    reader: BufReader<Stream>,
}

impl Client {
    pub async fn send(request: &Request) -> io::Result<Self> {
        let conn = Stream::connect(socket_name()?).await?;
        let mut json = serde_json::to_string(request).map_err(io::Error::other)?;
        json.push('\n');
        (&conn).write_all(json.as_bytes()).await?;
        Ok(Self { reader: BufReader::new(conn) })
    }

    /// The next response, or `None` once the daemon closes the connection.
    pub async fn next(&mut self) -> io::Result<Option<Response>> {
        let mut line = String::new();
        if self.reader.read_line(&mut line).await? == 0 {
            return Ok(None);
        }
        serde_json::from_str(&line).map(Some).map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_json_shape() {
        let json = serde_json::to_string(&Request::Dispatch { command: "workspace 2".into() }).unwrap();
        assert_eq!(json, r#"{"type":"dispatch","command":"workspace 2"}"#);
        let query: Request = serde_json::from_str(r#"{"type":"query","what":"active_window"}"#).unwrap();
        assert!(matches!(query, Request::Query { what: Query::ActiveWindow }));
    }
}
