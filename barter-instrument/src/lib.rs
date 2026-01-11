// 强制禁止不安全代码，确保内存安全
#![forbid(unsafe_code)]
// 启用一系列 lint 警告，以提高代码质量和一致性
#![warn(
    unused,
    clippy::cognitive_complexity,
    unused_crate_dependencies,
    unused_extern_crates,
    clippy::unused_self,
    clippy::useless_let_if_seq,
    missing_debug_implementations,
    rust_2018_idioms,
    rust_2024_compatibility
)]
// 允许某些复杂的类型声明，在复杂的交易逻辑中，类型定义可能会变得非常长
#![allow(clippy::type_complexity, clippy::too_many_arguments, type_alias_bounds)]

//! # Barter-Instrument
//! Barter-Instrument 包含了核心的交易所 (Exchange)、交易工具 (Instrument) 和资产 (Asset) 的数据结构及相关工具。
//! 这是整个 Barter 生态的基础，所有交易相关的定义都源自这里。
//!
//! ## Examples
//! 完整的示例请参考 Barter 核心引擎的 /examples 目录。

use derive_more::Constructor;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// Defines a global [`ExchangeId`](exchange::ExchangeId) enum covering all exchanges.
pub mod exchange;

/// [`Asset`](asset::Asset) related data structures.
///
/// eg/ `AssetKind`, `AssetNameInternal`, etc.
pub mod asset;

/// [`Instrument`](instrument::Instrument) related data structures.
///
/// eg/ `InstrumentKind`, `OptionContract``, etc.
pub mod instrument;

/// Indexed collection of exchanges, assets, and instruments. Provides a builder utility for
/// indexing non-indexed collections.
pub mod index;

/// 一个带有键的值结构体。
/// ### 泛型参数：
/// * `Key`: 标识符类型（如 `InstrumentIndex`，代表交易工具在数组中的位置）。
/// * `Value`: 存储的具体数据类型（如 `Instrument` 结构体）。
/// ### 示例：
/// `Keyed { key: InstrumentIndex(0), value: btc_usdt_instrument }`
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct Keyed<Key, Value> {
    pub key: Key,
    pub value: Value,
}

impl<Key, Value> AsRef<Value> for Keyed<Key, Value> {
    fn as_ref(&self) -> &Value {
        &self.value
    }
}

impl<Key, Value> Display for Keyed<Key, Value>
where
    Key: Display,
    Value: Display,
{
    /// `Formatter<'_>` 中的 `'_` 是匿名生命周期标识。
    /// 它告诉编译器，这个 Formatter 的引用有效期至少要和这个函数调用一样长，
    /// 但我们不关心它的具体生命周期名字。这是 Rust 中非常常见的写法。
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, {}", self.key, self.value)
    }
}

/// 交易工具的底层资产，包含基础资产 (base) 和计价资产 (quote)。
/// ### 示例：
/// `Underlying { base: "BTC", quote: "USDT" }` 代表使用 USDT 计价的 BTC。
/// ### 泛型参数：
/// * `AssetKey`: 资产标识类型。
///   - 实际例子 1: `&'static str` (如 "BTC")
///   - 实际例子 2: `AssetIndex` (系统内部生成的唯一整数索引，提高性能)
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct Underlying<AssetKey> {
    /// 基础资产 (Base Asset)：你要交易的目标实物（如 BTC）。
    pub base: AssetKey,
    /// 计价资产 (Quote Asset)：你用来衡量目标实物价值的货币（如 USDT）。
    pub quote: AssetKey,
}

impl<AssetKey> Underlying<AssetKey> {
    /// 创建一个新的底层资产结构。
    /// ### 泛型参数：
    /// * `A`: 任何可以转换为 `AssetKey` 的类型。
    /// ### 参数：
    /// * `base`: 基础资产，如 "BTC"。
    /// * `quote`: 计价资产，如 "USDT"。
    pub fn new<A>(base: A, quote: A) -> Self
    where
        A: Into<AssetKey>,
    {
        Self {
            base: base.into(),
            quote: quote.into(),
        }
    }
}

/// 交易或仓位的方向：买入 (Buy) 或 卖出 (Sell)。
/// ### derive 说明：
/// * `Ord`, `PartialOrd`: 允许对 Side 枚举进行排序（通常 Buy < Sell）。
/// * `Hash`: 允许将 Side 作为 HashMap 的键。
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Deserialize, Serialize)]
pub enum Side {
    /// 当从 JSON 反序列化时，遇到 "buy", "BUY", "b" 都会被解析为 Buy。
    #[serde(alias = "buy", alias = "BUY", alias = "b")]
    Buy,
    /// 当从 JSON 反序列化时，遇到 "sell", "SELL", "s" 都会被解析为 Sell。
    #[serde(alias = "sell", alias = "SELL", alias = "s")]
    Sell,
}

impl Display for Side {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Side::Buy => "buy",
                Side::Sell => "sell",
            }
        )
    }
}

/// 测试工具模块，提供了一些方便构造资产和交易工具的辅助函数。
pub mod test_utils {
    use crate::{
        Underlying,
        asset::{
            Asset, ExchangeAsset,
            name::{AssetNameExchange, AssetNameInternal},
        },
        exchange::ExchangeId,
        instrument::{
            Instrument,
            kind::InstrumentKind,
            name::{InstrumentNameExchange, InstrumentNameInternal},
            quote::InstrumentQuoteAsset,
        },
    };

    /// 创建一个 `ExchangeAsset`，将交易所 ID 与资产绑定。
    pub fn exchange_asset(exchange: ExchangeId, symbol: &str) -> ExchangeAsset<Asset> {
        ExchangeAsset {
            exchange,
            asset: asset(symbol),
        }
    }

    /// 根据符号字符串创建一个 `Asset`。
    /// 内部名称和交易所名称目前被设置为相同。
    pub fn asset(symbol: &str) -> Asset {
        Asset {
            name_internal: AssetNameInternal::from(symbol),
            name_exchange: AssetNameExchange::from(symbol),
        }
    }

    /// 创建一个通用的现货 (Spot) 交易工具。
    pub fn instrument(
        exchange: ExchangeId,
        base: &str,
        quote: &str,
    ) -> Instrument<ExchangeId, Asset> {
        // 按照常见的 "BASE_QUOTE" 格式生成交易所端的名称
        let name_exchange = InstrumentNameExchange::from(format!("{base}_{quote}"));
        // 根据交易所和名称生成内部唯一的名称
        let name_internal =
            InstrumentNameInternal::new_from_exchange(exchange, name_exchange.clone());
        let base_asset = asset(base);
        let quote_asset = asset(quote);

        Instrument::new(
            exchange,
            name_internal,
            name_exchange,
            Underlying::new(base_asset, quote_asset),
            InstrumentQuoteAsset::UnderlyingQuote,
            InstrumentKind::Spot,
            None,
        )
    }
}
