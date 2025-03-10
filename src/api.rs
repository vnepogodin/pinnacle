// SPDX-License-Identifier: GPL-3.0-or-later

//! Pinnacle's configuration API.
//!
//! The API works as follows:
//!
//! - All configuration is done through a Unix socket located at /tmp/pinnacle_socket.
//! - Pinnacle is built with the intent of configuration in Lua (and possibly other languages in
//!   the future). To achieve this, an always running process in the target language needs to be
//!   spawned, known as the *client*. This allows Pinnacle, the *server*, to call into user-defined
//!   state through callback functions.
//! - The client must:
//!     - Connect to the socket,
//!     - send configuration messages through the socket, and finally
//!     - listen to requests for callbacks,
//!   in that order.
//!
//! You may be asking, "what messages am I supposed to send and receive?"
//! Great question!
//!
//! Pinnacle uses [MessagePack](https://msgpack.org/index.html) as the message format.
//! Messages should be serialized into MessagePack according to the [defined structs](msg::Msg).
//!
//! When Pinnacle needs to call a user-defined callback, for example from a keybind setting, it
//! sends a [CallCallback](msg::OutgoingMsg::CallCallback) message to the client. This message
//! contains a callback_id to identify what callback the client needs to run—but wait, where do you get that?
//!
//! The callback_id is created by the client to identify one of its callbacks. You will probably
//! need to store all callbacks in some central data structure along with a way to associate an id with it.
//! This could be an array and its indices or a hashmap and its keys (keep in mind the id needs to
//! be an unsigned 32 bit int).
//!
//! TODO: expand
//!
//! For an example, look at the Lua implementation in the repository.

pub mod handlers;
pub mod msg;

use std::{
    io::{self, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use calloop::RegistrationToken;
use smithay::reexports::calloop::{
    self, channel::Sender, generic::Generic, EventSource, Interest, Mode, PostAction,
};
use sysinfo::{ProcessRefreshKind, RefreshKind, SystemExt};

use self::msg::{Msg, OutgoingMsg};

pub const SOCKET_NAME: &str = "pinnacle_socket";

/// Handle a config process.
///
/// `stream` is the incoming stream where messages will be received,
/// and `sender` sends decoded messages to the main state's handler.
fn handle_client(
    mut stream: UnixStream,
    sender: Sender<Msg>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let mut len_marker_bytes = [0u8; 4];
        if let Err(err) = stream.read_exact(&mut len_marker_bytes) {
            if err.kind() == io::ErrorKind::UnexpectedEof {
                tracing::warn!("stream closed: {}", err);
                stream.shutdown(std::net::Shutdown::Both)?;
                break Ok(());
            }
        };

        let len_marker = u32::from_ne_bytes(len_marker_bytes);
        let mut msg_bytes = vec![0u8; len_marker as usize];

        if let Err(err) = stream.read_exact(msg_bytes.as_mut_slice()) {
            if err.kind() == io::ErrorKind::UnexpectedEof {
                tracing::warn!("stream closed: {}", err);
                stream.shutdown(std::net::Shutdown::Both)?;
                break Ok(());
            }
        };
        let msg: Msg = rmp_serde::from_slice(msg_bytes.as_slice())?; // TODO: handle error

        sender.send(msg)?;
    }
}

/// A socket source for an event loop that will listen for config processes.
pub struct PinnacleSocketSource {
    /// The socket listener
    socket: Generic<UnixListener>,
    /// The sender that will send messages from clients to the main event loop.
    sender: Sender<Msg>,
}

impl PinnacleSocketSource {
    /// Create a loop source that listens for connections to the provided `socket_dir`.
    /// This will also set PINNACLE_SOCKET for use in API implementations.
    pub fn new(sender: Sender<Msg>, socket_dir: &Path) -> anyhow::Result<Self> {
        tracing::debug!("Creating socket source for dir {socket_dir:?}");

        let system = sysinfo::System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new()),
        );

        // Test if you are running multiple instances of Pinnacle
        let multiple_instances = system.processes_by_exact_name("pinnacle").count() > 1;

        // If you are, append a suffix to the socket name
        let socket_name = if multiple_instances {
            let mut suffix: u8 = 1;
            while let Ok(true) = socket_dir
                .join(format!("{SOCKET_NAME}_{suffix}"))
                .try_exists()
            {
                suffix += 1;
            }
            format!("{SOCKET_NAME}_{suffix}")
        } else {
            SOCKET_NAME.to_string()
        };

        let socket_path = socket_dir.join(socket_name);

        // If there are multiple instances, don't touch other sockets
        if multiple_instances {
            if let Ok(exists) = socket_path.try_exists() {
                if exists {
                    std::fs::remove_file(&socket_path)
                        .context(format!("Failed to remove old socket at {socket_path:?}",))?;
                }
            }
        } else {
            // If there aren't, remove them all
            for file in std::fs::read_dir(socket_dir)?
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_name().to_string_lossy().starts_with(SOCKET_NAME))
            {
                tracing::debug!("Removing socket at {:?}", file.path());
                std::fs::remove_file(file.path())
                    .context(format!("Failed to remove old socket at {:?}", file.path()))?;
            }
        }

        let listener = UnixListener::bind(&socket_path)
            .with_context(|| format!("Failed to bind to socket at {socket_path:?}"))?;
        tracing::info!("Bound to socket at {socket_path:?}");

        listener
            .set_nonblocking(true)
            .context("Failed to set socket to nonblocking")?;

        let socket = Generic::new(listener, Interest::READ, Mode::Level);

        std::env::set_var("PINNACLE_SOCKET", socket_path);

        Ok(Self { socket, sender })
    }
}

/// Send a message to a client.
pub fn send_to_client(
    stream: &mut UnixStream,
    msg: &OutgoingMsg,
) -> Result<(), rmp_serde::encode::Error> {
    tracing::trace!("Sending {msg:?}");

    let msg = rmp_serde::to_vec_named(msg)?;
    let msg_len = msg.len() as u32;
    let bytes = msg_len.to_ne_bytes();

    if let Err(err) = stream.write_all(&bytes) {
        if err.kind() == io::ErrorKind::BrokenPipe {
            // TODO: notify user that config daemon is ded
            return Ok(()); // TODO:
        }
    }

    if let Err(err) = stream.write_all(msg.as_slice()) {
        if err.kind() == io::ErrorKind::BrokenPipe {
            // TODO: something
            return Ok(()); // TODO:
        }
    }

    Ok(())
}

impl EventSource for PinnacleSocketSource {
    type Event = UnixStream;

    type Metadata = ();

    type Ret = ();

    type Error = io::Error;

    fn process_events<F>(
        &mut self,
        readiness: calloop::Readiness,
        token: calloop::Token,
        mut callback: F,
    ) -> Result<calloop::PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        self.socket
            .process_events(readiness, token, |_readiness, listener| {
                while let Ok((stream, _sock_addr)) = listener.accept() {
                    let sender = self.sender.clone();
                    let callback_stream = stream.try_clone()?;

                    callback(callback_stream, &mut ());

                    // Handle the client in another thread as to not block the main one.
                    //
                    // No idea if this is even needed or if it's premature optimization.
                    std::thread::spawn(move || {
                        if let Err(err) = handle_client(stream, sender) {
                            tracing::error!("handle_client errored: {err}");
                        }
                    });
                }

                Ok(PostAction::Continue)
            })
    }

    fn register(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.socket.register(poll, token_factory)
    }

    fn reregister(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.socket.reregister(poll, token_factory)
    }

    fn unregister(&mut self, poll: &mut calloop::Poll) -> calloop::Result<()> {
        self.socket.unregister(poll)
    }
}

pub struct ApiState {
    // TODO: this may not need to be in an arc mutex because of the move to async
    /// The stream API messages are being sent through.
    pub stream: Option<Arc<Mutex<UnixStream>>>,
    /// A token used to remove the socket source from the event loop on config restart.
    pub socket_token: Option<RegistrationToken>,
    /// The sending channel used to send API messages received from the socket source to a handler.
    pub tx_channel: Sender<Msg>,
    /// A channel used to ping the future in the event loop to drop and kill the child.
    pub kill_channel: Option<async_channel::Sender<()>>,
    /// A receiving channel that will cause the config process to be dropped and thus killed.
    pub future_channel: Option<async_channel::Receiver<()>>,
}
