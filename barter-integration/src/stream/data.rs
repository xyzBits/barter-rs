use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::{sync::oneshot, task::JoinHandle};

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
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let forward = move |item| tx.send(item).map_err(|_| ());

    let key = "stream-key";
    let stream = futures::stream::pending();

    let streams = DataStreams {
        streams: HashMap::from_iter([(key, stream)]),
    };

    let mut streams = streams.forward(forward);

    let () = streams.stop_all();
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

impl<StreamKey, St> DataStreams<StreamKey, St>
where
    StreamKey: Eq + std::hash::Hash + Send,
{
    pub fn new(streams: HashMap<StreamKey, St>) -> Self {
        Self { streams }
    }

    pub fn insert(&mut self, key: StreamKey, stream: St) -> Option<St> {
        self.streams.insert(key, stream)
    }

    pub fn remove(&mut self, key: &StreamKey) -> Option<St>
    where
        StreamKey: std::borrow::Borrow<StreamKey>,
    {
        self.streams.remove(key)
    }

    pub fn forward<FnForward>(self, mut forward: FnForward) -> DataStreams<StreamKey, StreamHandle>
    where
        St: Stream + Unpin + Send,
        FnForward: FnMut(St::Item) -> Result<(), ()> + Send + 'static,
    {
        let Self { streams } = self;

        let streams = streams
            .into_iter()
            .map(|(key, mut stream)| {
                let (shutdown_tx, shutdown_rx) = oneshot::channel();

                let forward_future = stream
                    .take_until(shutdown_rx)
                    .map(|item| forward(item))
                    .take_while(|result: &Result<(), ()>| std::future::ready(result.is_ok()))
                    .for_each(|result| {
                        // todo!()
                        std::future::ready(())
                    });

                let handle = StreamHandle {
                    handle: tokio::spawn(forward_future),
                    shutdown: shutdown_tx,
                };

                (key, handle)
            })
            .collect();

        DataStreams { streams }
    }

    pub fn forward_async<FnForward>(
        self,
        forward: FnForward,
    ) -> DataStreams<StreamKey, StreamHandle>
    where
        St: Stream + Unpin + Send,
        FnForward: AsyncFnMut(St::Item) -> Result<(), ()> + Clone + Send + 'static,
    {
        let Self { streams } = self;

        let streams = streams
            .into_iter()
            .map(|(key, mut stream)| {
                let forward = forward.clone();
                let (shutdown_tx, shutdown_rx) = oneshot::channel();

                let forward_future = stream
                    .take_until(shutdown_rx)
                    .then(|item| forward(item))
                    .take_while(|result: &Result<(), ()>| std::future::ready(result.is_ok()))
                    .for_each(|result| {
                        // todo!()
                        std::future::ready(())
                    });

                let handle = StreamHandle {
                    handle: tokio::spawn(forward_future),
                    shutdown: shutdown_tx,
                };

                (key, handle)
            })
            .collect();

        DataStreams { streams }
    }
}

struct StreamHandle {
    handle: JoinHandle<()>,
    shutdown: oneshot::Sender<()>,
}

pub enum DataStreamsError<StreamKey> {
    StreamNotFound(StreamKey),
}

impl<StreamKey> DataStreams<StreamKey, StreamHandle> {
    pub fn stop(&mut self, key: &StreamKey) -> Result<(), DataStreamsError<StreamKey>>
    where
        StreamKey: Clone,
    {
        let Some(stream) = self.streams.remove(key) else {
            return Err(DataStreamsError::StreamNotFound(key.clone()));
        };

        let StreamHandle { handle, shutdown } = stream;

        // Send shutdown signal (ignore error if receiver already dropped)
        let _ = shutdown.send(());

        // Abort the spawned task
        handle.abort();

        Ok(())
    }

    pub fn stop_all(mut self) {
        for (_, stream_handle) in self.streams {
            // Send shutdown signal
            let _ = stream_handle.shutdown.send(());

            // Abort the task
            stream_handle.handle.abort();
        }
    }
}

// Some abstract to flow through information to outer Stream?

// pub struct DataStreamFlattener;
//
// impl<StreamKey> FromIterator<DataStreams> for DataStreamFlattener {
//
// }

pub enum UpsertResult<Stream> {
    Insert,
    Updated((Option<Stream>)),
}

// Todo: ensure compatible with ReconnectingStreams (maybe layer over DataStream Fn(Args) -> impl Stream
// impl<StreamKey, Stream> DataStreams<StreamKey, Stream> {
//     pub async fn upsert<Args>(
//         &mut self,
//         keyed_args: impl IntoIterator<Item = Keyed<StreamKey, Args>>,
//     ) -> Result<Option<Stream>, Stream::Error>
//     where
//         Stream: DataStream<Args>,
//     {
//         let futures = keyed_args
//             .into_iter()
//
//
//
//         let entry = self
//             .streams
//             .insert()
//             .entry(key);
//
//         match Stream::init(args).await {
//             Ok(stream) => stream,
//             Err() => {}
//         }
//
//         let stream = Stream::init(args).await?;
//
//
//
//
//
//
//
//
//     }
// }
//
// pub struct DataStreamsBuilder<StreamKey, Args> {
//     pub streams: FnvHashMap<StreamKey, Args>,
// }
//
// impl<StreamKey, Args> DataStreamsBuilder<StreamKey, Args> {
//     pub fn add(
//         mut self,
//         key: StreamKey,
//         args: Args,
//     ) -> Self
//     {
//         if let Some(replaced) = self.streams.insert(key, args) {
//             debug!(
//                 %key,
//                 old = %replaced,
//                 new = %args,
//                 "DataStreamsBuilder replaced StreamKey Args"
//             );
//         }
//
//         self
//     }
// }
