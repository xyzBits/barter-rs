# Barter-Instrument 架构分析

## 概述

`barter-instrument` 是 Barter 交易生态系统的基础模块，定义了交易所、资产和交易工具的核心数据结构。它是其他所有 Barter 模块的依赖基础。

## 核心设计理念

### 1. 类型安全的索引系统 (NewType Pattern)

Rust 通过 NewType 模式提供编译期类型安全，防止参数传递错误：

```rust
// Java 写法：容易混淆参数顺序
fn get_fee(exchange: usize, instrument: usize)

// Rust 写法：编译器强制类型检查
fn get_fee(exchange: ExchangeIndex, instrument: InstrumentIndex)
```

**设计原因**：
- 零成本抽象，运行时就是纯整数
- 编译期防止传参错误
- 自文档化的代码

---

## 模块结构

```
barter-instrument/src/
├── lib.rs           # 入口，定义 Keyed, Underlying, Side
├── exchange.rs      # ExchangeId, ExchangeIndex
├── asset/
│   ├── mod.rs       # Asset, AssetIndex, AssetKind
│   └── name.rs      # AssetNameInternal, AssetNameExchange
├── instrument/
│   ├── mod.rs       # Instrument, InstrumentIndex
│   ├── kind/        # InstrumentKind (Spot/Future/Perpetual/Option)
│   ├── name.rs      # InstrumentNameInternal, InstrumentNameExchange
│   ├── quote.rs     # InstrumentQuoteAsset
│   └── spec.rs      # InstrumentSpec (精度、最小数量等)
└── index/
    ├── mod.rs       # IndexedInstruments
    ├── builder.rs   # IndexedInstrumentsBuilder
    └── error.rs     # IndexError
```

---

## 核心数据结构

### 1. 交易所 (Exchange)

```rust
// 交易所枚举 - 覆盖所有支持的交易所
pub enum ExchangeId {
    BinanceSpot,
    BinanceFuturesUsd,
    Coinbase,
    Okx,
    // ... 40+ 交易所
}

// 交易所索引 - 用于高效的 O(1) 查找
pub struct ExchangeIndex(pub usize);
```

**设计原因**：
- `ExchangeId` 提供类型安全的交易所标识
- 不同产品类型有独立的变体（如 `BinanceSpot` vs `BinanceFuturesUsd`）
- `ExchangeIndex` 用于数组索引，实现 O(1) 查找

### 2. 资产 (Asset)

```rust
pub struct Asset {
    pub name_internal: AssetNameInternal,  // 内部标准化名称
    pub name_exchange: AssetNameExchange,  // 交易所原始名称
}

pub struct ExchangeAsset<Asset> {
    pub exchange: ExchangeId,
    pub asset: Asset,
}

pub struct AssetIndex(pub usize);  // 资产索引
```

**关键类型**：
- `BaseAsset` - 基础资产（如 BTC/USDT 中的 BTC）
- `QuoteAsset` - 计价资产（如 BTC/USDT 中的 USDT）
- `AssetKind` - 资产类型（Crypto/Fiat）

### 3. 交易对底层 (Underlying)

```rust
pub struct Underlying<AssetKey> {
    pub base: AssetKey,   // 基础货币（商品）
    pub quote: AssetKey,  // 计价货币
}
// 例：Underlying { base: "btc", quote: "usdt" }
// 表示"用 USDT 买卖 BTC"这个交易对
```

### 4. 交易工具 (Instrument)

```rust
pub struct Instrument<ExchangeKey, AssetKey> {
    pub exchange: ExchangeKey,
    pub name_internal: InstrumentNameInternal,
    pub name_exchange: InstrumentNameExchange,
    pub underlying: Underlying<AssetKey>,
    pub quote: InstrumentQuoteAsset,
    pub kind: InstrumentKind<AssetKey>,
    pub spec: Option<InstrumentSpec<AssetKey>>,
}
```

**工具类型 (InstrumentKind)**：
| 类型 | 说明 |
|------|------|
| `Spot` | 现货交易 |
| `Future(FutureContract)` | 期货合约（含到期日） |
| `Perpetual(PerpetualContract)` | 永续合约 |
| `Option(OptionContract)` | 期权（含行权价、类型等） |

### 5. 索引集合 (IndexedInstruments)

```rust
pub struct IndexedInstruments {
    exchanges: Vec<Keyed<ExchangeIndex, ExchangeId>>,
    assets: Vec<Keyed<AssetIndex, ExchangeAsset<Asset>>>,
    instruments: Vec<Keyed<InstrumentIndex, Instrument<...>>>,
    // 查找表
    exchange_name_to_index: HashMap<ExchangeId, ExchangeIndex>,
    asset_to_index: HashMap<(ExchangeId, AssetNameInternal), AssetIndex>,
    instrument_to_index: HashMap<(ExchangeId, InstrumentNameInternal), InstrumentIndex>,
}
```

**核心操作**：
| 方法 | 复杂度 | 说明 |
|------|--------|------|
| `find_exchange_index()` | O(1) | 通过 ExchangeId 查找索引 |
| `find_asset_index()` | O(1) | 通过交易所+名称查找资产索引 |
| `find_instrument_index()` | O(1) | 通过交易所+名称查找工具索引 |
| `find_instrument()` | O(1) | 通过索引获取工具详情 |

---

## 数据流

```mermaid
graph LR
    A[配置文件/API] --> B[Instrument列表]
    B --> C[IndexedInstruments::new]
    C --> D[索引化存储]
    D --> E[O(1) 查找]
    
    subgraph 索引化
        D --> D1[ExchangeIndex]
        D --> D2[AssetIndex]  
        D --> D3[InstrumentIndex]
    end
```

## 为什么这样设计

1. **O(1) 查找性能**：使用索引而非字符串作为键，避免哈希计算
2. **缓存友好**：连续内存布局的 Vec 比 HashMap 更缓存友好
3. **类型安全**：编译期防止索引混淆
4. **双重命名**：`name_internal`（标准化）+ `name_exchange`（原始），支持跨交易所统一处理
5. **泛型设计**：`Instrument<ExchangeKey, AssetKey>` 支持索引化前后的不同表示

---

## 依赖关系

```
barter-instrument
    ↓ (被依赖)
├── barter-integration
├── barter-data
├── barter-execution
└── barter (核心引擎)
```

`barter-instrument` 是最底层模块，不依赖其他 Barter 模块。
