// 交易所集成抽象层。
// 该模块定义了与各交易所服务器通信、订阅市场数据以及处理连接生命周期的统一接口。
use self::subscription::ExchangeSub;
use crate::{
    MarketStream, SnapshotFetcher,
    instrument::InstrumentData,
    subscriber::{Subscriber, validator::SubscriptionValidator},
    subscription::{Map, SubscriptionKind},
};
use barter_instrument::exchange::ExchangeId;
use barter_integration::{Validator, error::SocketError, protocol::websocket::WsMessage};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{fmt::Debug, time::Duration};
use url::Url;

// Binance 现货与永续合约的连接器实现。
/// `BinanceSpot` & `BinanceFuturesUsd` [`Connector`] and [`StreamSelector`] implementations.
pub mod binance;

// Bitfinex 的连接器实现。
/// `Bitfinex` [`Connector`] and [`StreamSelector`] implementations.
pub mod bitfinex;

// Bitmex 的连接器实现。
/// `Bitmex [`Connector`] and [`StreamSelector`] implementations.
pub mod bitmex;

// Bybit 的连接器实现。
/// `Bybit` ['Connector'] and ['StreamSelector'] implementation
pub mod bybit;

// Coinbase 的连接器实现。
/// `Coinbase` [`Connector`] and [`StreamSelector`] implementations.
pub mod coinbase;

// Gate.io 现货与期货的连接器实现。
/// `GateioSpot`, `GateioFuturesUsd` & `GateioFuturesBtc` [`Connector`] and [`StreamSelector`]
/// implementations.
pub mod gateio;

// Kraken 的连接器实现。
/// `Kraken` [`Connector`] and [`StreamSelector`] implementations.
pub mod kraken;


// OKX 的连接器实现。
/// `Okx` [`Connector`] and [`StreamSelector`] implementations.
pub mod okx;

// 统一定义用于构建 WsMessage 订阅负载的市场与频道组合。
/// Defines the generic [`ExchangeSub`] containing a market and channel combination used by an
/// exchange [`Connector`] to build [`WsMessage`] subscription payloads.
pub mod subscription;

// 默认订阅超时时间（10秒）。
/// Default [`Duration`] the [`Connector::SubValidator`] will wait to receive all success responses to actioned
/// `Subscription` requests.
pub const DEFAULT_SUBSCRIPTION_TIMEOUT: Duration = Duration::from_secs(10);

// 定义与交易所订阅类型 [`SubscriptionKind`] 相关联的 [`MarketStream`] 类型。
//
// ### 注意
// 如果交易所 [`Connector`] 支持特定的 [`SubscriptionKind`]，则必须实现此 Trait。
/// Defines the [`MarketStream`] kind associated with an exchange
/// `Subscription` [`SubscriptionKind`].
///
/// ### Notes
/// Must be implemented by an exchange [`Connector`] if it supports a specific
/// [`SubscriptionKind`].
pub trait StreamSelector<Instrument, Kind>
where
    Self: Connector,
    Instrument: InstrumentData,
    Kind: SubscriptionKind,
{
    type SnapFetcher: SnapshotFetcher<Self, Kind>;
    type Stream: MarketStream<Self, Instrument, Kind>;
}

// 主要的交易所抽象层。定义了如何将 Barter 类型转换为交易所特定的类型，
// 以及如何连接、订阅和与交易所服务器进行交互。
//
// ### 注意
// 集成新的交易所时必须实现此 Trait！
/// Primary exchange abstraction. Defines how to translate Barter types into exchange specific
/// types, as well as connecting, subscribing, and interacting with the exchange server.
///
/// ### Notes
/// This must be implemented for a new exchange integration!
pub trait Connector

where
    Self: Clone + Default + Debug + for<'de> Deserialize<'de> + Serialize + Sized,
{
    /// Unique identifier for the exchange server being connected with.
    const ID: ExchangeId;

    /// Type that defines how to translate a Barter `Subscription` into an exchange specific
    /// channel to be subscribed to.
    ///
    /// ### Examples
    /// - [`BinanceChannel("@depth@100ms")`](binance::channel::BinanceChannel)
    /// - [`KrakenChannel("trade")`](kraken::channel::KrakenChannel)
    type Channel: AsRef<str>;

    /// Type that defines how to translate a Barter
    /// `Subscription` into an exchange specific market that
    /// can be subscribed to.
    ///
    /// ### Examples
    /// - [`BinanceMarket("btcusdt")`](binance::market::BinanceMarket)
    /// - [`KrakenMarket("BTC/USDT")`](kraken::market::KrakenMarket)
    type Market: AsRef<str>;

    /// [`Subscriber`] type that establishes a connection with the exchange server, and actions
    /// `Subscription`s over the socket.
    type Subscriber: Subscriber;

    /// [`SubscriptionValidator`] type that listens to responses from the exchange server and
    /// validates if the actioned `Subscription`s were
    /// successful.
    type SubValidator: SubscriptionValidator;

    /// Deserializable type that the [`Self::SubValidator`] expects to receive from the exchange server in
    /// response to the `Subscription` [`Self::requests`]
    /// sent over the [`WebSocket`](barter_integration::protocol::websocket::WebSocket). Implements
    /// [`Validator`] in order to determine if [`Self`]
    /// communicates a successful `Subscription` outcome.
    type SubResponse: Validator + Debug + DeserializeOwned;

    /// Base [`Url`] of the exchange server being connected with.
    fn url() -> Result<Url, SocketError>;

    /// Defines [`PingInterval`] of custom application-level
    /// [`WebSocket`](barter_integration::protocol::websocket::WebSocket) pings for the exchange
    /// server being connected with.
    ///
    /// Defaults to `None`, meaning that no custom pings are sent.
    fn ping_interval() -> Option<PingInterval> {
        None
    }

    /// Defines how to translate a collection of [`ExchangeSub`]s into the [`WsMessage`]
    /// subscription payloads sent to the exchange server.
    fn requests(exchange_subs: Vec<ExchangeSub<Self::Channel, Self::Market>>) -> Vec<WsMessage>;

    /// Number of `Subscription` responses expected from the
    /// exchange server in responses to the requests send. Used to validate all
    /// Subscriptions were accepted.
    fn expected_responses<InstrumentKey>(map: &Map<InstrumentKey>) -> usize {
        map.0.len()
    }

    /// Expected [`Duration`] the [`SubscriptionValidator`] will wait to receive all success
    /// responses to actioned `Subscription` requests.
    fn subscription_timeout() -> Duration {
        DEFAULT_SUBSCRIPTION_TIMEOUT
    }
}

// 当交易所将不同市场数据（如现货和合约）分布在不同服务器上时使用。
// 它允许所有的 [`Connector`] 逻辑保持一致，仅由该 Trait 提供服务器特定信息。
/// Used when an exchange has servers different
/// [`InstrumentKind`](barter_instrument::instrument::kind::InstrumentKind) market data on distinct servers,
/// allowing all the [`Connector`] logic to be identical apart from what this trait provides.
///
/// ### Examples
/// - [`BinanceServerSpot`](binance::spot::BinanceServerSpot)
/// - [`BinanceServerFuturesUsd`](binance::futures::BinanceServerFuturesUsd)
pub trait ExchangeServer: Default + Debug + Clone + Send {
    const ID: ExchangeId;
    fn websocket_url() -> &'static str;
}

// 定义自定义 WebSocket 心跳（Ping）的频率和构建函数。
// 适用于那些需要额外应用层心跳的交易所。
/// Defines the frequency and construction function for custom
/// [`WebSocket`](barter_integration::protocol::websocket::WebSocket) pings - used for exchanges
/// that require additional application-level pings.
#[derive(Debug)]
pub struct PingInterval {
    pub interval: tokio::time::Interval,
    pub ping: fn() -> WsMessage,
}
