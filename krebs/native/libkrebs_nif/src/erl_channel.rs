use std::sync::mpsc;
use std::sync::mpsc::{Receiver, SendError, SyncSender};
use std::thread::JoinHandle;

use rustler;
use rustler::{Encoder, LocalPid, OwnedEnv};

/// A one-way channel that forwards messages from Rustler to
/// an Erlang process.
///
/// Uses [`std::sync::mpsc`](mpsc) under the hood - message
/// passing within message passing.
pub struct ErlChannel<T: Send + Sync + Encoder + 'static> {
    sender: SyncSender<ErlChannelMessage<T>>,
    recipient_thread: JoinHandle<usize>,
}

enum ErlChannelMessage<T: Send + Sync + Encoder + 'static> {
    Here(T),
    Close,
}

impl<T: Send + Sync + Encoder + 'static> ErlChannel<T> {
    pub const DEFAULT_CAPACITY: usize = 10;

    /// Creates a new [`ErlChannel`] that sends messages to
    /// the given Erlang PID.
    pub fn new(recipient: LocalPid) -> Self {
        Self::with_capacity(recipient, Self::DEFAULT_CAPACITY)
    }

    /// Creates a new [`ErlChannel`] that sends messages to
    /// the given Erlang PID, with the given buffer capacity.
    ///
    /// (The default should work just fine; throughput
    /// ought to be very high.)
    pub fn with_capacity(recipient: LocalPid, capacity: usize) -> Self {
        let (sender, erl_mailbox) = mpsc::sync_channel::<ErlChannelMessage<T>>(capacity);

        Self {
            sender,
            recipient_thread: Self::create_recipient_thread(erl_mailbox, recipient),
        }
    }

    /// Sends the given message to the Erlang process.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`std::sync::mpsc::SyncSender::send`].
    ///
    /// # Panics
    ///
    /// Propagates panics from [`rustler::OwnedEnv::send_and_clear`]
    /// (this should never happen).
    pub fn send(&self, val: T) -> Result<(), SendError<T>> {
        self.sender.send(ErlChannelMessage::Here(val)).map_err(
            |msg: SendError<ErlChannelMessage<T>>| -> SendError<T> {
                if let ErlChannelMessage::Here(inner) = msg.0 {
                    SendError(inner)
                } else {
                    unreachable!("SendError should always wrap the original Here(val) message")
                }
            },
        )
    }

    /// Forces the channel to close if it hasn't already closed,
    /// consuming it in the process.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`std::sync::mpsc::SyncSender::send`].
    ///
    /// # Panics
    ///
    /// Propagates panics from [`std::thread::JoinHandle::join`].
    pub fn force_close(self) -> Result<usize, SendError<()>> {
        let res = self
            .sender
            .send(ErlChannelMessage::Close)
            .map_err(|_| SendError(()));

        let count = self.recipient_thread.join().unwrap();

        std::mem::drop(self.sender);

        res.map(|_| count)
    }

    fn create_recipient_thread(
        erl_mailbox: Receiver<ErlChannelMessage<T>>,
        recipient: LocalPid,
    ) -> JoinHandle<usize> {
        std::thread::spawn(move || {
            let mut msg_env = OwnedEnv::new();
            let mut msg_count: usize = 0;

            loop {
                match erl_mailbox.recv() {
                    Ok(msg_enum) => match msg_enum {
                        ErlChannelMessage::Here(msg) => {
                            let _ = msg_env.send_and_clear(&recipient, |env| msg.encode(env));
                            msg_count += 1;
                        }
                        ErlChannelMessage::Close => {
                            return msg_count;
                        }
                    },
                    Err(_) => {
                        return msg_count;
                    }
                }
            }
        })
    }
}
