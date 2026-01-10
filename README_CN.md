# Barter
Barter 是一个用于构建高性能实盘交易、模拟交易和回测系统的 Rust 算法交易生态系统库集合。
* **高速**：采用原生 Rust 编写。最小化内存分配。面向数据的状态管理系统，支持直接索引查找。
* **稳健**：强类型。线程安全。广泛的测试覆盖率。
* **可定制**：即插即用的 Strategy（策略）和 RiskManager（风控管理器）组件，支持大多数交易策略（做市、统计套利、高频交易等）。
* **可扩展**：多线程架构与模块化设计。利用 Tokio 进行 I/O 处理。高效内存数据结构。

**请参阅：[`Barter`]、[`Barter-Data`]、[`Barter-Instrument`]、[`Barter-Execution`] 和 [`Barter-Integration`] 获取每个库的完整文档和示例。**

[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]
[![Discord chat][discord-badge]][discord-url]
[![DeepWiki][deepwiki-badge]][deepwiki-url]

[crates-badge]: https://img.shields.io/crates/v/barter.svg
[crates-url]: https://crates.io/crates/barter

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/barter-rs/barter-rs/blob/develop/LICENSE

[discord-badge]: https://img.shields.io/discord/910237311332151317.svg?logo=discord&style=flat-square
[discord-url]: https://discord.gg/wE7RqhnQMV

[deepwiki-badge]: https://deepwiki.com/badge.svg
[deepwiki-url]: https://deepwiki.com/barter-rs/barter-rs

[`Barter`]: https://crates.io/crates/barter
[`Barter-Instrument`]: https://crates.io/crates/barter-instrument
[`Barter-Data`]: https://crates.io/crates/barter-data
[`Barter-Execution`]: https://crates.io/crates/barter-execution
[`Barter-Integration`]: https://crates.io/crates/barter-integration
[API Documentation]: https://docs.rs/barter/latest/barter/
[Chat]: https://discord.gg/wE7RqhnQMV

## 概述
Barter 是一个用于构建高性能实盘交易、模拟交易和回测系统的 Rust 算法交易生态系统库集合。它由以下几个易于使用、可扩展的 crate 组成：
* **Barter**：具有丰富状态管理系统的算法交易引擎。
* **Barter-Instrument**：交易所、交易工具和资产的数据结构及实用工具。
* **Barter-Data**：从金融交易场所流式获取公开市场数据。可通过 MarketStream 接口轻松扩展。
* **Barter-Execution**：流式获取私有账户数据并执行订单。可通过 ExecutionClient 接口轻松扩展。
* **Barter-Integration**：用于灵活 REST/WebSocket 集成的底层框架。

## 主要特性
- 通过 [`Barter-Data`] 库从金融交易场所流式获取公开市场数据。
- 通过 [`Barter-Execution`] 库流式获取私有账户数据、执行订单（实盘或模拟）。
- 即插即用的 Strategy 和 RiskManager 组件，支持大多数交易策略。
- 回测工具，可高效运行数千个并发回测。
- 灵活的引擎，支持同时在多个交易所执行交易策略。
- 使用模拟的 MarketStream 或 Execution 组件，使回测系统与实盘交易系统几乎相同。
- 集中式缓存友好的状态管理系统，使用索引数据结构实现 O(1) 常数时间查找。
- 强大的订单管理系统 - 可独立使用或与 Barter 配合使用。
- 包含全面绩效指标的交易摘要（盈亏、夏普比率、索提诺比率、回撤等）。
- 支持从外部进程（如 UI、Telegram 等）开启/关闭算法交易，同时继续处理市场/账户数据。
- 从外部进程（如 UI、Telegram 等）发出引擎命令，触发操作（平仓所有头寸、开仓订单、取消订单等）。
- EngineState 副本管理器，处理引擎审计流，支持非热路径监控组件（如 UI、Telegram 等）。

[barter-examples]: https://github.com/barter-rs/barter-rs/tree/develop/barter/examples

## 示例
* 请参阅[此处][barter-examples]获取包含完整导入的可编译示例。
* 请参阅各子 crate 获取更多库示例。

#### 使用实时市场数据和模拟执行进行模拟交易

```rust,no_run
const FILE_PATH_SYSTEM_CONFIG: &str = "barter/examples/config/system_config.json";
const RISK_FREE_RETURN: Decimal = dec!(0.05);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志追踪
    init_logging();

    // 加载系统配置
    let SystemConfig {
        instruments,
        executions,
    } = load_config()?;

    // 构建索引化的交易工具集合
    let instruments = IndexedInstruments::new(instruments);

    // 初始化市场数据流
    let market_stream = init_indexed_multi_exchange_market_stream(
        &instruments,
        &[SubKind::PublicTrades, SubKind::OrderBooksL1],
    )
    .await?;

    // 构建系统参数
    let args = SystemArgs::new(
        &instruments,
        executions,
        LiveClock,
        DefaultStrategy::default(),
        DefaultRiskManager::default(),
        market_stream,
    );

    // 构建并运行完整系统：
    // 请参阅 SystemBuilder 了解所有配置选项
    let mut system = SystemBuilder::new(args)
        // 引擎采用同步模式（迭代器输入）
        .engine_feed_mode(EngineFeedMode::Iterator)

        // 启用审计流（引擎发送审计事件）
        .audit_mode(AuditMode::Enabled)

        // 引擎启动时交易状态设为禁用
        .trading_state(TradingState::Disabled)

        // 构建系统，但暂不启动任务
        .build::<EngineEvent, DefaultGlobalData, DefaultInstrumentMarketData>()?

        // 初始化系统，在当前运行时上启动组件任务
        .init_with_runtime(tokio::runtime::Handle::current())
        .await?;

    // 获取引擎审计接收器的所有权
    let audit_rx = system.audit_rx.take().unwrap();

    // 运行示例异步审计流消费者
    // 注意：您可能需要使用此流来复制 EngineState，或持久化事件等
    //  --> 例如：参见 examples/engine_sync_with_audit_replica_engine_state
    let audit_task = tokio::spawn(async move {
        let mut audit_stream = audit_rx.into_stream();
        while let Some(audit) = audit_stream.next().await {
            debug!(?audit, "AuditStream consumed AuditTick");
            if let EngineAudit::Shutdown(_) = audit.event {
                break;
            }
        }
        audit_stream
    });

    // 启用交易
    system.trading_state(TradingState::Enabled);

    // 让示例运行 5 秒...
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 关闭前，取消订单然后平仓
    system.cancel_orders(InstrumentFilter::None);
    system.close_positions(InstrumentFilter::None);

    // 关闭系统
    let (engine, _shutdown_audit) = system.shutdown().await?;
    let _audit_stream = audit_task.await?;

    // 生成每日交易摘要
    let trading_summary = engine
        .trading_summary_generator(RISK_FREE_RETURN)
        .generate(Daily);

    // 将每日交易摘要打印到终端（也可保存到文件、发送到其他地方等）
    trading_summary.print_summary();

    Ok(())
}

fn load_config() -> Result<SystemConfig, Box<dyn std::error::Error>> {
    let file = File::open(FILE_PATH_SYSTEM_CONFIG)?;
    let reader = BufReader::new(file);
    let config = serde_json::from_reader(reader)?;
    Ok(config)
}
```

## 获取帮助
首先，请查看 [API 文档][API Documentation] 中是否有您问题的答案。如果没有找到，我很乐意通过 [Discord 聊天][Chat] 来帮助您解答问题。

## 支持 Barter 开发
通过成为赞助者（或给我打赏！）来帮助我们提升 Barter 的能力。

您的贡献将使我能够投入更多时间到 Barter 上，加速功能开发和改进。

**如有任何咨询，请发送邮件至 *justastream.code@gmail.com***

### 赞助等级
* 🥇 **赞助者** - 您的姓名、logo 和网站链接将显示在下方。
* 🥈 **支持者** - 您的姓名将被列为支持者。

### 当前赞助者
*您的姓名、logo 和网站链接可以显示在这里*

### 当前支持者
*您的姓名可以显示在这里*

---
**感谢所有赞助者和支持者！🫶**

## 贡献
感谢您帮助开发 Barter 生态系统！如有开发、新功能和未来路线图方面的讨论，请随时通过 Discord [聊天][Chat] 联系我们。

### 许可证
本项目采用 [MIT 许可证]授权。

[MIT 许可证]: https://github.com/barter-rs/barter-rs/blob/develop/LICENSE

### 贡献许可协议

您有意提交的任何用于包含在 Barter 工作区 crate 中的贡献应：
1. 采用 MIT 许可证授权
2. 遵守下述所有免责声明和责任限制
3. 不附带任何额外条款或条件
4. 在理解仅用于教育目的和风险警告适用的前提下提交

提交贡献即表示您证明您有权在这些条款下进行提交。

## 法律免责声明和责任限制

请在使用本软件前仔细阅读本免责声明。访问或使用本软件即表示您承认并同意受本文条款的约束。

1. 教育目的
   本软件和相关文档（"软件"）仅供教育和研究目的使用。本软件不用于、不设计、不测试、不验证或不认证用于商业部署、实盘交易或任何形式的生产使用。

2. 非投资建议
   本软件中包含的任何内容均不构成财务、投资、法律或税务建议。不应依赖本软件的任何方面进行交易决策或财务规划。强烈建议用户就其具体情况咨询合格的专业人士以获取投资指导。

3. 风险承担
   在金融市场进行交易，包括但不限于加密货币、证券、衍生品和其他金融工具，存在重大损失风险。用户承认：
   a) 他们可能损失全部投资；
   b) 过去的业绩不代表未来的结果；
   c) 假设或模拟的业绩结果存在固有的局限性和偏差。

4. 免责声明
   本软件按"原样"提供，不提供任何明示或暗示的担保。在法律允许的最大范围内，作者和版权持有人明确否认所有担保，包括但不限于：
   a) 适销性
   b) 特定用途适用性
   c) 不侵权
   d) 结果的准确性或可靠性
   e) 系统集成
   f) 安静享用权

5. 责任限制
   在任何情况下，作者、版权持有人、贡献者或任何关联方均不对任何直接、间接、偶然、特殊、惩戒性或后果性损害（包括但不限于采购替代商品或服务、使用损失、数据或利润损失；或业务中断）负责，无论该损害是基于合同、严格责任还是侵权（包括疏忽或其他）的任何责任理论而产生，即使已被告知可能发生此类损害。

6. 监管合规
   本软件未在任何金融监管机构注册、获得其背书或批准。用户自行负责：
   a) 确定其使用是否符合适用的法律法规
   b) 获取任何所需的许可证、执照或注册
   c) 履行其管辖区内的任何监管义务

7. 赔偿
   用户同意对作者、版权持有人和任何关联方因用户使用本软件而产生的任何索赔、责任、损害、损失和费用进行赔偿、辩护并使其免受损害。

8. 确认
   使用本软件即表示用户确认他们已阅读本免责声明、理解其内容，并同意受其条款和条件的约束。

对于不允许排除某些担保或责任限制的司法管辖区，上述限制可能不适用。
