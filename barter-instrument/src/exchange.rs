use derive_more::{Constructor, Display};
use serde::{Deserialize, Serialize};

/// 交易所索引，是对 `usize` 的封装。
/// ### 为什么要这么做？
/// 1. **类型安全**：防止将一个普通的整数（如 `instrument_index`）误传给需要 `exchange_index` 的函数。
///    例如：`fn get_fee(exchange: ExchangeIndex, instrument: InstrumentIndex)` 即使底层都是 `usize`，
///    传错顺序编译器也会提示错误。
/// 2. **零成本抽象**：它是 Rust 的 Newtype 模式，在运行时它就是个纯 `usize`，没有任何对象分配开销。
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct ExchangeIndex(pub usize);

impl ExchangeIndex {
    pub fn index(&self) -> usize {
        self.0
    }
}

impl std::fmt::Display for ExchangeIndex {
    /// `Formatter<'_>` 中的 `'_` 是匿名生命周期。
    /// 它表示 Formatter 的生命周期与 fmt 函数调用的生命周期绑定，开发者无需显式命名。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExchangeIndex({})", self.0)
    }
}

/// 交易所的唯一标识符枚举。
/// ### 示例：
/// `ExchangeId::BinanceSpot`, `ExchangeId::Okx`
/// ### derive 说明：
/// * `Display`: 来自 `derive_more` 插件，自动将枚举名转换为字符串（如 "BinanceSpot"）。
/// * `snake_case`: 在序列化为 JSON 时，会将 `BinanceSpot` 转换为 `binance_spot`。
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Display,
)]
#[serde(rename = "execution", rename_all = "snake_case")]
pub enum ExchangeId {
    Other,
    Simulated,
    Mock,
    BinanceFuturesCoin,
    BinanceFuturesUsd,
    BinanceOptions,
    BinancePortfolioMargin,
    BinanceSpot,
    BinanceUs,
    Bitazza,
    Bitfinex,
    Bitflyer,
    Bitget,
    Bitmart,
    BitmartFuturesUsd,
    Bitmex,
    Bitso,
    Bitstamp,
    Bitvavo,
    Bithumb,
    BybitPerpetualsUsd,
    BybitSpot,
    Cexio,
    Coinbase,
    CoinbaseInternational,
    Cryptocom,
    Deribit,
    GateioFuturesBtc,
    GateioFuturesUsd,
    GateioOptions,
    GateioPerpetualsBtc,
    GateioPerpetualsUsd,
    GateioSpot,
    Gemini,
    Hitbtc,
    // 即使是老牌交易所火币 (Huobi) 更名为 HTX，这里也通过 alias 兼容了 "huobi" 的写法。
    #[serde(alias = "huobi")]
    Htx,
    Kraken,
    Kucoin,
    Liquid,
    Mexc,
    Okx,
    Poloniex,
}

impl ExchangeId {
    /// 返回交易所 ID 的字符串表示形式。
    /// 返回的是 `'static str`，因为它指向程序的只读存储区，在整个程序生命周期内都有效。
    pub fn as_str(&self) -> &'static str {
        match self {
            ExchangeId::Other => "other",
            ExchangeId::Simulated => "simulated",
            ExchangeId::Mock => "mock",
            ExchangeId::BinanceFuturesCoin => "binance_futures_coin",
            ExchangeId::BinanceFuturesUsd => "binance_futures_usd",
            ExchangeId::BinanceOptions => "binance_options",
            ExchangeId::BinancePortfolioMargin => "binance_portfolio_margin",
            ExchangeId::BinanceSpot => "binance_spot",
            ExchangeId::BinanceUs => "binance_us",
            ExchangeId::Bitazza => "bitazza",
            ExchangeId::Bitfinex => "bitfinex",
            ExchangeId::Bitflyer => "bitflyer",
            ExchangeId::Bitget => "bitget",
            ExchangeId::Bitmart => "bitmart",
            ExchangeId::BitmartFuturesUsd => "bitmart_futures_usd",
            ExchangeId::Bitmex => "bitmex",
            ExchangeId::Bitso => "bitso",
            ExchangeId::Bitstamp => "bitstamp",
            ExchangeId::Bitvavo => "bitvavo",
            ExchangeId::Bithumb => "bithumb",
            ExchangeId::BybitPerpetualsUsd => "bybit_perpetuals_usd",
            ExchangeId::BybitSpot => "bybit_spot",
            ExchangeId::Cexio => "cexio",
            ExchangeId::Coinbase => "coinbase",
            ExchangeId::CoinbaseInternational => "coinbase_international",
            ExchangeId::Cryptocom => "cryptocom",
            ExchangeId::Deribit => "deribit",
            ExchangeId::GateioFuturesBtc => "gateio_futures_btc",
            ExchangeId::GateioFuturesUsd => "gateio_futures_usd",
            ExchangeId::GateioOptions => "gateio_options",
            ExchangeId::GateioPerpetualsBtc => "gateio_perpetuals_btc",
            ExchangeId::GateioPerpetualsUsd => "gateio_perpetuals_usd",
            ExchangeId::GateioSpot => "gateio_spot",
            ExchangeId::Gemini => "gemini",
            ExchangeId::Hitbtc => "hitbtc",
            ExchangeId::Htx => "htx", // 这里 HTX 是火币的新名字
            ExchangeId::Kraken => "kraken",
            ExchangeId::Kucoin => "kucoin",
            ExchangeId::Liquid => "liquid",
            ExchangeId::Mexc => "mexc",
            ExchangeId::Okx => "okx",
            ExchangeId::Poloniex => "poloniex",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_de_exchange_id() {
        // 验证 "htx" 能正确反序列化
        assert_eq!(
            serde_json::from_str::<ExchangeId>(r#""htx""#).unwrap(),
            ExchangeId::Htx
        );
        // 验证别名 "huobi" 也能正确反序列化为 Htx 变体
        assert_eq!(
            serde_json::from_str::<ExchangeId>(r#""huobi""#).unwrap(),
            ExchangeId::Htx
        );
    }
}
