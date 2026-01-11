use crate::{
    Keyed,
    asset::name::{AssetNameExchange, AssetNameInternal},
    exchange::ExchangeId,
};
use derive_more::{Constructor, Display};
use serde::{Deserialize, Serialize};

/// Defines the [`AssetNameInternal`] and [`AssetNameExchange`] types, used as `SmolStr`
/// identifiers for an [`Asset`].
pub mod name;

/// 资产的唯一逻辑标识符 (Business ID)。
/// ### 特性：
/// * **类型**: `u64`。
/// * **用途**: 通常用于跨系统交互、数据库持久化或作为全局唯一的哈希 ID。
/// * **稳定性**: 即使系统重启，同一个资产的 `AssetId` 通常保持不变（取决于具体实现）。
/// * **对比**: 区别于 `AssetIndex`，它不代表内存中的位置。
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Display,
)]
pub struct AssetId(pub u64);

/// 资产的内存索引 (Memory Index)。
/// ### 特性：
/// * **类型**: `usize` (封装)。
/// * **用途**: 专门用于在 `IndexedInstruments` 等数据结构中进行 O(1) 复杂度的极速查找。
/// * **机制**: 它实际上是 `Vec<Asset>` 数组的下标（位置）。
/// * **局限性**: 仅在当前运行进程的内存结构中有效，重启后索引顺序可能重新生成。
/// * **性能**: 在高频交易的热路径 (Hot Path) 中，使用 `AssetIndex` 访问数据比使用 `HashMap` 查找 `AssetId` 快得多。
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct AssetIndex(pub usize);

impl AssetIndex {
    pub fn index(&self) -> usize {
        self.0
    }
}

impl std::fmt::Display for AssetIndex {
    /// `Formatter<'_>` 是格式化器引用。
    /// `'_` 表示一个匿名生命周期，确保格式化器在写入操作完成前一直有效。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AssetIndex({})", self.0)
    }
}

/// 表示某个特定交易所中的资产结构体。
/// ### 泛型参数：
/// * `Asset`: 资产的类型（如 `Asset` 结构体，或者 `AssetIndex` 索引）。
/// ### 示例：
/// `ExchangeAsset { exchange: ExchangeId::BinanceSpot, asset: btc_asset }`
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct ExchangeAsset<Asset> {
    /// 交易所 ID（如 Binance）
    pub exchange: ExchangeId, 
    /// 资产标识（如 "BTC"）
    pub asset: Asset,         
}

impl<Asset> ExchangeAsset<Asset> {
    pub fn new<A>(exchange: ExchangeId, asset: A) -> Self
    where
        A: Into<Asset>,
    {
        Self {
            exchange,
            asset: asset.into(),
        }
    }
}

impl<Ass, Asset, T> From<(ExchangeId, Ass, T)> for Keyed<ExchangeAsset<Asset>, T>
where
    Ass: Into<Asset>,
{
    fn from((exchange, asset, value): (ExchangeId, Ass, T)) -> Self {
        Self {
            key: ExchangeAsset::new(exchange, asset),
            value,
        }
    }
}

/// 核心资产结构体。
/// 在交易系统中，区分资产的内部统一名称和交易所端原始名称至关重要。
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct Asset {
    /// 系统内部统一处理使用的名字（通常统一小写，如 "btc"）。
    pub name_internal: AssetNameInternal,
    /// 该资产在交易所官方文档或 API 中使用的原始名字（如 "XBT" 或 "BTC"）。
    pub name_exchange: AssetNameExchange,
}

impl<S> From<S> for Asset
where
    S: Into<AssetNameExchange>,
{
    fn from(value: S) -> Self {
        Self::new_from_exchange(value)
    }
}

impl Asset {
    pub fn new<Internal, Exchange>(name_internal: Internal, name_exchange: Exchange) -> Self
    where
        Internal: Into<AssetNameInternal>,
        Exchange: Into<AssetNameExchange>,
    {
        Self {
            name_internal: name_internal.into(),
            name_exchange: name_exchange.into(),
        }
    }

    pub fn new_from_exchange<S>(name_exchange: S) -> Self
    where
        S: Into<AssetNameExchange>,
    {
        let name_exchange = name_exchange.into();
        Self {
            name_internal: AssetNameInternal::from(name_exchange.name().clone()),
            name_exchange,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Display)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Crypto,
    Fiat,
}

impl From<Asset> for AssetNameInternal {
    fn from(value: Asset) -> Self {
        value.name_internal
    }
}

/// 特殊类型，仅用于类型系统中的“基础资产 (Base Asset)”标记。
/// 例如：交易对 btc_usdt_spot 中，BaseAsset 指的是 btc。
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Display)]
pub struct BaseAsset;

/// 特殊类型，仅用于类型系统中的“计价资产 (Quote Asset)”标记。
/// 例如：交易对 btc_usdt_spot 中，QuoteAsset 指的是 usdt。
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Display)]
pub struct QuoteAsset;
