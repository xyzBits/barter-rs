use crate::{
    Underlying,
    asset::Asset,
    instrument::{
        kind::{
            InstrumentKind, future::FutureContract, option::OptionContract,
            perpetual::PerpetualContract,
        },
        market_data::{MarketDataInstrument, kind::MarketDataInstrumentKind},
        name::{InstrumentNameExchange, InstrumentNameInternal},
        quote::InstrumentQuoteAsset,
        spec::{InstrumentSpec, InstrumentSpecQuantity, OrderQuantityUnits},
    },
};
use derive_more::{Constructor, Display};
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;

/// Defines an [`Instrument`]s [`InstrumentKind`] (eg/ Spot, Perpetual, etc).
pub mod kind;

/// Defines the [`InstrumentNameExchange`] and [`InstrumentNameExchange`] types, used as
/// `SmolStr` identifiers for an [`Instrument`].
pub mod name;

/// Defines the [`InstrumentSpec`], including specifications for an [`Instrument`]s
/// price, quantity and notional value.
///
/// eg/ `InstrumentSpecPrice.tick_size`, `OrderQuantityUnits`, etc.
pub mod spec;

/// Defines a simplified [`MarketDataInstrument`], with only the necessary data to subscribe to
/// market data feeds.
pub mod market_data;

/// Defines the [`InstrumentQuoteAsset`] (underlying base or quote) for an [`Instrument`].
pub mod quote;

/// 交易工具的唯一 64 位标识符。
/// 用于以内存高效的方式对数据事件（如行情更新）进行索引。
/// ### derive 说明：
/// * `Hash`: 允许将此 ID 作为 `HashMap` 或 `HashSet` 的键，这在缓存交易状态时非常有用。
/// * `Ord`, `PartialOrd`: 允许对 ID 进行排序，从而可以在 BTreeMap 等有序数据结构中使用。
/// * `Display`: 使用 `derive_more` 自动实现显示逻辑。
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Display,
)]
pub struct InstrumentId(pub u64);

/// 交易工具在数组中的索引位置。
/// ### derive 说明：
/// * `Constructor`: 由 `derive_more` 提供，自动生成 `new(usize)` 构造函数。
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct InstrumentIndex(pub usize);

impl InstrumentIndex {
    pub fn index(&self) -> usize {
        self.0
    }
}

impl std::fmt::Display for InstrumentIndex {
    /// `Formatter<'_>` 中的 `'_` 是匿名生命周期。
    /// 在实现 Display trait 时，这意味着 formatter 的引用在函数执行期间是有效的。
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "InstrumentIndex({})", self.0)
    }
}

/// 综合交易工具模型，包含订阅行情数据和生成正确订单所需的所有数据。
/// ### 泛型参数：
/// * `ExchangeKey`: 交易所的唯一标识类型。
///   - 实际例子: `ExchangeId` (如 `ExchangeId::BinanceSpot`)
/// * `AssetKey`: 资产的唯一标识类型。
///   - 实际例子: `AssetIndex` (如 `AssetIndex(0)`)，这是为了在高频计算中使用整数索引代替字符串，提升性能。
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct Instrument<ExchangeKey, AssetKey> {
    /// 该交易工具所属的交易所标识。
    pub exchange: ExchangeKey,
    /// Barter 系统内部的统一名称。
    /// 示例：binance_spot-btc_usdt
    pub name_internal: InstrumentNameInternal,
    /// 交易所官方的产品名称。
    /// 示例：BTCUSDT
    pub name_exchange: InstrumentNameExchange,
    /// 底层资产（包含基础/计价货币对）。
    pub underlying: Underlying<AssetKey>,
    /// 报价资产的结算方式。
    pub quote: InstrumentQuoteAsset,
    /// 交易工具的种类。
    /// 可以通过别名 `instrument_kind` 进行反序列化。
    #[serde(alias = "instrument_kind")]
    pub kind: InstrumentKind<AssetKey>,
    /// 交易规则详情（如价格跳动、最小挂单等），可选。
    pub spec: Option<InstrumentSpec<AssetKey>>,
}

impl<ExchangeKey, AssetKey> Instrument<ExchangeKey, AssetKey> {
    /// Construct a new `Instrument` with the provided data.
    ///
    /// This constructor assumes the [`InstrumentNameInternal`] can be constructed in the default
    /// style via the [`InstrumentNameInternal::new_from_exchange`] constructor.
    pub fn new<NameInternal, NameExchange>(
        exchange: ExchangeKey,
        name_internal: NameInternal,
        name_exchange: NameExchange,
        underlying: Underlying<AssetKey>,
        quote: InstrumentQuoteAsset,
        kind: InstrumentKind<AssetKey>,
        spec: Option<InstrumentSpec<AssetKey>>,
    ) -> Self
    where
        NameInternal: Into<InstrumentNameInternal>,
        NameExchange: Into<InstrumentNameExchange>,
    {
        Self {
            exchange,
            name_internal: name_internal.into(),
            name_exchange: name_exchange.into(),
            quote,
            underlying,
            kind,
            spec,
        }
    }

    /// Construct a new `Spot` `Instrument` with the provided data.
    ///
    /// This constructor assumes the [`InstrumentNameInternal`] can be constructed in the default
    /// style via the [`InstrumentNameInternal::new_from_exchange`] constructor.
    pub fn spot<NameInternal, NameExchange>(
        exchange: ExchangeKey,
        name_internal: NameInternal,
        name_exchange: NameExchange,
        underlying: Underlying<AssetKey>,
        spec: Option<InstrumentSpec<AssetKey>>,
    ) -> Self
    where
        NameInternal: Into<InstrumentNameInternal>,
        NameExchange: Into<InstrumentNameExchange>,
    {
        Self {
            exchange,
            name_internal: name_internal.into(),
            name_exchange: name_exchange.into(),
            quote: InstrumentQuoteAsset::UnderlyingQuote,
            underlying,
            kind: InstrumentKind::Spot,
            spec,
        }
    }

    /// Map this Instruments `ExchangeKey` to a new key.
    pub fn map_exchange_key<NewExchangeKey>(
        self,
        exchange: NewExchangeKey,
    ) -> Instrument<NewExchangeKey, AssetKey> {
        let Instrument {
            exchange: _,
            name_internal,
            name_exchange,
            underlying,
            quote,
            kind,
            spec,
        } = self;

        Instrument {
            exchange,
            name_internal,
            name_exchange,
            underlying,
            quote,
            kind,
            spec,
        }
    }

    /// 将交易工具关联的资产标识符映射为新类型。
    /// ### 泛型参数：
    /// * `FnFindAsset`: 查找闭包类型。
    /// * `NewAssetKey`: 转换后的新标识符类型（如将 `&str` 转为 `AssetIndex`）。
    /// * `Error`: 查找失败时返回的错误类型。
    /// ### 参数：
    /// * `find_asset`: 执行查找逻辑的函数或闭包。
    ///   - 实际例子: `|name| self.index_map.get(name).ok_or(MyError::NotFound)`
    pub fn map_asset_key_with_lookup<FnFindAsset, NewAssetKey, Error>(
        self,
        find_asset: FnFindAsset,
    ) -> Result<Instrument<ExchangeKey, NewAssetKey>, Error>
    where
        FnFindAsset: Fn(&AssetKey) -> Result<NewAssetKey, Error>,
    {
        let Instrument {
            exchange,
            name_internal,
            name_exchange,
            underlying,
            quote,
            kind,
            spec,
        } = self;

        let base_new_key = find_asset(&underlying.base)?;
        let quote_new_key = find_asset(&underlying.quote)?;

        let kind = match kind {
            InstrumentKind::Spot => InstrumentKind::Spot,
            InstrumentKind::Perpetual(contract) => InstrumentKind::Perpetual(PerpetualContract {
                contract_size: contract.contract_size,
                settlement_asset: find_asset(&contract.settlement_asset)?,
            }),
            InstrumentKind::Future(contract) => InstrumentKind::Future(FutureContract {
                contract_size: contract.contract_size,
                settlement_asset: find_asset(&contract.settlement_asset)?,
                expiry: contract.expiry,
            }),
            InstrumentKind::Option(contract) => InstrumentKind::Option(OptionContract {
                contract_size: contract.contract_size,
                settlement_asset: find_asset(&contract.settlement_asset)?,
                kind: contract.kind,
                exercise: contract.exercise,
                expiry: contract.expiry,
                strike: contract.strike,
            }),
        };

        let spec = match spec {
            Some(spec) => {
                let InstrumentSpec {
                    price,
                    quantity:
                        InstrumentSpecQuantity {
                            unit,
                            min,
                            increment,
                        },
                    notional,
                } = spec;

                let unit = match unit {
                    OrderQuantityUnits::Asset(asset) => {
                        OrderQuantityUnits::Asset(find_asset(&asset)?)
                    }
                    OrderQuantityUnits::Contract => OrderQuantityUnits::Contract,
                    OrderQuantityUnits::Quote => OrderQuantityUnits::Quote,
                };

                Some(InstrumentSpec {
                    price,
                    quantity: InstrumentSpecQuantity {
                        unit,
                        min,
                        increment,
                    },
                    notional,
                })
            }
            None => None,
        };

        Ok(Instrument {
            exchange,
            name_internal,
            name_exchange,
            underlying: Underlying::new(base_new_key, quote_new_key),
            quote,
            kind,
            spec,
        })
    }
}

impl<ExchangeKey> From<&Instrument<ExchangeKey, Asset>> for MarketDataInstrument {
    fn from(value: &Instrument<ExchangeKey, Asset>) -> Self {
        Self {
            base: value.underlying.base.name_internal.clone(),
            quote: value.underlying.quote.name_internal.clone(),
            kind: MarketDataInstrumentKind::from(&value.kind),
        }
    }
}
