use std::{
    collections::HashMap,
    io,
    mem::ManuallyDrop,
    sync::atomic::{AtomicU64, Ordering},
    thread,
};

use futures::{SinkExt as _, StreamExt as _};
use interprocess::local_socket::{
    self,
    traits::tokio::{Listener, Stream as _},
    GenericNamespaced, ToNsName,
};
use rand::RngCore as _;
use tasinput_protocol::{
    codec::MessageCodec, HostMessage, HostRequest, UiContent, UiMessage, UiReply, UiRequest,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::{
    codec::{FramedRead, FramedWrite},
    sync::CancellationToken,
};

pub(crate) struct Endpoint {
    io_thread: ManuallyDrop<thread::JoinHandle<()>>,
    cancel: CancellationToken,
    send_queue: mpsc::Sender<(HostRequest, oneshot::Sender<UiReply>)>,
}

pub(crate) struct EndpointWaiting {
    inner: Endpoint,
    socket_id: String,
    io_ready: oneshot::Receiver<io::Result<()>>,
}

impl EndpointWaiting {
    pub(crate) fn wait_ready(self) -> io::Result<Endpoint> {
        self.io_ready.blocking_recv().unwrap()?;
        Ok(self.inner)
    }

    pub fn socket_id(&self) -> &str {
        &self.socket_id
    }
}

impl Endpoint {
    pub(crate) fn new() -> io::Result<EndpointWaiting> {
        let mut os_rng = rand::rngs::OsRng::default();

        // generate a unique ID
        let uuid: u128 = {
            let mut bytes = [0u8; 16];
            os_rng.fill_bytes(&mut bytes);
            u128::from_ne_bytes(bytes)
        };

        let socket_id = format!("tasinput-{:016X}", uuid);

        let socket_name = socket_id.clone()
            .to_ns_name::<GenericNamespaced>()?
            .into_owned();
        let (send_queue, send_queue_out) =
            mpsc::channel::<(HostRequest, oneshot::Sender<UiReply>)>(16);
        let cancel = CancellationToken::new();

        let io_rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let (io_ready_src, io_ready) = oneshot::channel::<io::Result<()>>();

        let io_thread = thread::spawn({
            let cancel = cancel.clone();
            move || {
                io_rt.block_on(async move {
                    let mut io_data =
                        match EndpointLoop::setup(socket_name, cancel, send_queue_out).await {
                            Ok(io_data) => {
                                let _ = io_ready_src.send(Ok(()));
                                io_data
                            }
                            Err(error) => {
                                let _ = io_ready_src.send(Err(error));
                                return;
                            }
                        };

                    io_data.main_loop().await
                })
            }
        });

        Ok(EndpointWaiting {
            inner: Self {
                io_thread: ManuallyDrop::new(io_thread),
                cancel,
                send_queue,
            },
            socket_id,
            io_ready,
        })
    }

    pub(crate) fn send_message(&self, message: HostRequest) -> UiReply {
        let (waiter_src, waiter) = oneshot::channel::<UiReply>();
        self.send_queue
            .blocking_send((message, waiter_src))
            .unwrap();
        waiter.blocking_recv().unwrap()
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.cancel.cancel();
        unsafe { ManuallyDrop::take(&mut self.io_thread) }
            .join()
            .unwrap();
    }
}

struct EndpointLoop {
    // socket data
    recv: FramedRead<local_socket::tokio::RecvHalf, MessageCodec<UiMessage>>,
    send: FramedWrite<local_socket::tokio::SendHalf, MessageCodec<HostMessage>>,
    // shutdown token
    cancel: CancellationToken,
    // request channels
    send_queue: mpsc::Receiver<(HostRequest, oneshot::Sender<UiReply>)>,
    waiters: HashMap<u64, oneshot::Sender<UiReply>>,
    id_counter: AtomicU64,
}

impl EndpointLoop {
    async fn setup(
        socket_name: local_socket::Name<'_>,
        cancel: CancellationToken,
        send_queue: mpsc::Receiver<(HostRequest, oneshot::Sender<UiReply>)>,
    ) -> io::Result<Self> {
        let listener = local_socket::ListenerOptions::new()
            .name(socket_name)
            .create_tokio()?;

        let (recv, send) = listener.accept().await?.split();
        let recv = FramedRead::new(recv, MessageCodec::new());
        let send = FramedWrite::new(send, MessageCodec::new());

        Ok(Self {
            recv,
            send,
            cancel,
            send_queue,
            waiters: HashMap::new(),
            id_counter: AtomicU64::new(0),
        })
    }
    async fn main_loop(&mut self) {
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    return
                },
                msg = self.recv.next() => {
                    self.handle_message(msg.unwrap().unwrap()).await;
                }
                next = self.send_queue.recv() => 'label: {
                    let (msg, waiter) = match next {
                        Some(value) => value,
                        None => break 'label,
                    };
                    let id = self.id_counter.fetch_add(1, Ordering::AcqRel);

                    self.waiters.insert(id, waiter);
                    self
                        .send
                        .send(HostMessage {
                            request_id: id,
                            content: msg.into()
                        })
                        .await
                        .expect("Failed to send to UI process!");
                },
            };
        }
    }

    async fn handle_message(
        &mut self,
        UiMessage {
            request_id,
            content,
        }: UiMessage,
    ) {
        match content {
            UiContent::Request(request) => self.handle_request(request_id, request).await,
            UiContent::Reply(reply) => {
                let sender = self.waiters.remove(&request_id).unwrap();
                let _ = sender.send(reply);
            }
        }
    }

    async fn handle_request(&mut self, _id: u64, request: UiRequest) {
        match request {}
    }
}
