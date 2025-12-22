use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use futures_util::future::{join_all, try_join_all};
use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, collections::HashMap, hash::Hash};
use tokio::{
    sync::oneshot,
    task::{JoinError, JoinHandle},
};

/// Generic `DataStream`.
///
/// Defines how to initialise the `DataStream`, and what the stream contains.
pub trait DataStream<Args> {
    /// Stream::Item type yielded by the stream.
    type Item;

    /// Connection error type if initialisation fails.
    type Error;

    /// Initialise the `DataStream`.
    fn init(
        args: Args,
    ) -> impl Future<Output = Result<impl Stream<Item = Self::Item> + Send, Self::Error>> + Send;
}

/// Configuration arguments for initializing a data stream.
///
/// This struct encapsulates all parameters required to initialize a data stream, including
/// the streaming mode (live or historical), subscriptions, server configuration, and timeout settings.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct DataArgs<Mode, Subs, Config> {
    /// The streaming mode (eg/ [`Live`] or [`Historical`]).
    pub mode: Mode,

    /// Subscriptions defining what data to stream.
    pub subscriptions: Subs,

    /// Configuration required for the `DataStream` source (eg/ credentials, urls, timeouts, etc.).
    pub config: Config,
}

impl<Subs, Config> DataArgs<Live, Subs, Config> {
    /// Construct [`DataArgs`] for a live data stream.
    ///
    /// # Arguments
    /// * `subscriptions` - The subscriptions defining what data to stream
    /// * `config` - Server-specific configuration for the stream connection
    pub fn live(subscriptions: Subs, config: Config) -> Self {
        Self {
            mode: Live,
            subscriptions,
            config,
        }
    }
}

impl<Subs, Config> DataArgs<Historical, Subs, Config> {
    /// Construct [`DataArgs`] for a historical data stream.
    ///
    /// # Arguments
    /// * `historical` - Time range specification for the historical data
    /// * `subscriptions` - The subscriptions defining what data to stream
    /// * `config` - Server-specific configuration for the stream connection
    pub fn historical(historical: Historical, subscriptions: Subs, config: Config) -> Self {
        Self {
            mode: historical,
            subscriptions,
            config,
        }
    }
}

/// [`DataStream`] kind, either [`Live`] or [`Historical`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum StreamKind {
    /// Real-time data stream providing events as they occur.
    Live(Live),

    /// Historical data stream replaying events from a time range.
    Historical(Historical),
}

/// Live [`DataStream`] kind.
///
/// Live `DataStream`s are real-time.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct Live;

/// Historical [`DataStream`] kind.
///
/// Historical `DataStream`s replay past events within a specified time range.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct Historical {
    /// Start timestamp of the historical data range.
    pub start: DateTime<Utc>,

    /// Optional end timestamp of the historical data range.
    ///
    /// If `None`, the stream continues until the present or until all available historical
    /// data is consumed.
    pub end: Option<DateTime<Utc>>,
}

#[tokio::test]
async fn run() {
    let runtime = tokio::runtime::Runtime::handle();

    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let key = "stream-key";
    let stream = futures::stream::pending();

    let streams = DataStreams {
        streams: HashMap::from_iter([(key, stream)]),
    };

    let streams = streams
        .forward(|| {
            let tx = tx.clone();
            move |key, item| tx.send(item)
        })
        .spawn(runtime);

    // Todo: Let's mirror the SystemBuild style w/ init() & init_with_runtime().

    let () = streams.stop_all();
}

pub struct StreamManager<StreamKey, St> {
    pub runtime: tokio::runtime::Handle,
    pub streams: HashMap<StreamKey, St>,
}

// Start with single stream type, then compose
pub struct DataStreams<StreamKey, St> {
    pub streams: HashMap<StreamKey, St>,
}

impl<StreamKey, St> Default for DataStreams<StreamKey, St> {
    fn default() -> Self {
        Self {
            streams: HashMap::default(),
        }
    }
}

impl<StreamKey, St> DataStreams<StreamKey, St> {
    pub fn new(streams: HashMap<StreamKey, St>) -> Self {
        Self { streams }
    }

    pub fn insert(&mut self, key: StreamKey, stream: St) -> Option<St>
    where
        StreamKey: Eq + Hash,
    {
        self.streams.insert(key, stream)
    }

    pub fn remove(&mut self, key: &StreamKey) -> Option<St>
    where
        StreamKey: Eq + Hash + Borrow<StreamKey>,
    {
        self.streams.remove(key)
    }

    pub fn forward<FnForward, Err>(
        self,
        forward: impl Fn() -> FnForward,
    ) -> DataStreams<StreamKey, StreamHandle<impl Future>>
    where
        StreamKey: Clone + Eq + Hash + Send + 'static,
        St: Stream + Unpin + Send + 'static,
        FnForward: FnMut(&StreamKey, St::Item) -> Result<(), Err> + Send + 'static,
        Err: Send,
    {
        let Self { streams } = self;

        let streams = streams
            .into_iter()
            .map(|(key, stream)| {
                let stream_key = key.clone();
                let mut forward = forward();
                let (shutdown_tx, shutdown_rx) = oneshot::channel();

                let forward_future = stream
                    .take_until(shutdown_rx)
                    .map(move |item| forward(&stream_key, item))
                    .take_while(|result: &Result<(), Err>| std::future::ready(result.is_ok()))
                    .for_each(|_result| std::future::ready(()));

                let handle = StreamHandle {
                    future: forward_future,
                    shutdown: shutdown_tx,
                };

                (key, handle)
            })
            .collect();

        DataStreams { streams }
    }

    pub fn forward_async<FnForward, ForwardFut, Err>(
        self,
        forward: impl Fn() -> FnForward,
    ) -> DataStreams<StreamKey, StreamHandle<impl Future>>
    where
        StreamKey: Clone + Eq + Hash + Send + 'static,
        St: Stream + Unpin + Send + 'static,
        St::Item: Send,
        FnForward: FnMut(&StreamKey, St::Item) -> ForwardFut + Send + 'static,
        ForwardFut: Future<Output = Result<(), Err>> + Send + 'static,
        Err: Send,
    {
        let Self { streams } = self;

        let streams = streams
            .into_iter()
            .map(|(key, stream)| {
                let stream_key = key.clone();
                let mut forward = forward();
                let (shutdown_tx, shutdown_rx) = oneshot::channel();

                let forward_future = stream
                    .take_until(shutdown_rx)
                    .then(move |item| forward(&stream_key, item))
                    .take_while(|result: &Result<(), Err>| std::future::ready(result.is_ok()))
                    .for_each(|_result| std::future::ready(()));

                let handle = StreamHandle {
                    future: forward_future,
                    shutdown: shutdown_tx,
                };

                (key, handle)
            })
            .collect();

        DataStreams { streams }
    }
}

impl<StreamKey, StreamFuture> DataStreams<StreamKey, StreamHandle<StreamFuture>> {
    pub fn spawn(
        self,
        rt: tokio::runtime::Runtime,
    ) -> DataStreams<StreamKey, StreamHandle<JoinHandle<()>>>
    where
        StreamKey: Eq + Hash,
        StreamFuture: Future<Output = ()> + Send,
        StreamFuture::Output: Send,
    {
        let streams = self
            .streams
            .into_iter()
            .map(|(key, handle)| {
                let StreamHandle { future, shutdown } = handle;

                let handle = StreamHandle {
                    future: rt.spawn(future),
                    shutdown,
                };

                (key, handle)
            })
            .collect();

        DataStreams { streams }
    }
}

impl<StreamKey> DataStreams<StreamKey, StreamHandle<JoinHandle<()>>> {
    pub fn stop(
        &mut self,
        key: &StreamKey,
    ) -> Result<impl Future<Output = Result<(), JoinError>>, DataStreamsError<StreamKey>>
    where
        StreamKey: Clone + Eq + Hash,
    {
        let Some(stream) = self.streams.remove(key) else {
            return Err(DataStreamsError::StreamNotFound(key.clone()));
        };

        let StreamHandle { future, shutdown } = stream;

        // Send shutdown signal (ignore error if receiver already dropped)
        let _ = shutdown.send(());

        // Return JoinHandle<()> Future
        Ok(future)
    }

    pub async fn stop_all(self) -> Result<(), JoinError> {
        let handles = self
            .streams
            .into_values()
            .map(|StreamHandle { future, shutdown }| {
                // Send shutdown signal
                let _ = shutdown.send(());

                future
            });

        try_join_all(handles).await.map(|_| ())
    }
}

pub struct StreamHandle<Future = JoinHandle<()>> {
    pub future: Future,
    pub shutdown: oneshot::Sender<()>,
}

pub enum DataStreamsError<StreamKey> {
    StreamNotFound(StreamKey),
}
