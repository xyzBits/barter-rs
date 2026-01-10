//! # 实时市场数据、模拟执行与审计流示例 (Live Market Data, Mock Execution & Audit)
//!
//! 该示例演示了如何整合实时市场数据、模拟执行（Mock Execution）以及启用审计流。
//! 重点在于如何同步启动系统、通过异步任务消费审计流，以及在系统关闭后生成业绩总结。

use barter::{
    engine::{
        clock::LiveClock,
        state::{
            global::DefaultGlobalData,
            instrument::{data::DefaultInstrumentMarketData, filter::InstrumentFilter},
            trading::TradingState,
        },
    },
    logging::init_logging,
    risk::DefaultRiskManager,
    statistic::time::Daily,
    strategy::DefaultStrategy,
    system::{
        builder::{AuditMode, EngineFeedMode, SystemArgs, SystemBuilder},
        config::SystemConfig,
    },
};
use barter_data::{
    streams::builder::dynamic::indexed::init_indexed_multi_exchange_market_stream,
    subscription::SubKind,
};
use barter_instrument::index::IndexedInstruments;
use barter_integration::Terminal;
use futures::StreamExt;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::{fs::File, io::BufReader, time::Duration};
use tracing::debug;

const FILE_PATH_SYSTEM_CONFIG: &str = "barter/examples/config/system_config.json";
const RISK_FREE_RETURN: Decimal = dec!(0.05);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 初始化日志追踪
    init_logging();

    // 2. 加载系统配置
    let SystemConfig {
        instruments,
        executions,
    } = load_config()?;

    // 3. 构建索引化的交易工具 (IndexedInstruments)
    let instruments = IndexedInstruments::new(instruments);

    // 4. 初始化实时多交易所市场数据流
    let market_stream = init_indexed_multi_exchange_market_stream(
        &instruments,
        &[SubKind::PublicTrades, SubKind::OrderBooksL1],
    )
    .await?;

    // 5. 构造系统参数 (SystemArgs)
    let args = SystemArgs::new(
        &instruments,
        executions,
        LiveClock,
        DefaultStrategy::default(),
        DefaultRiskManager::default(),
        market_stream,
        DefaultGlobalData::default(),
        |_| DefaultInstrumentMarketData::default(),
    );

    // 6. 使用 SystemBuilder 构建系统：
    // 关于所有配置选项，详见 SystemBuilder 文档。
    let mut system = SystemBuilder::new(args)
        // 引擎驱动模式：同步 Iterator 模式
        .engine_feed_mode(EngineFeedMode::Iterator)
        // 启用审计反馈流（引擎将发送审计事件）
        .audit_mode(AuditMode::Enabled)
        // 初始交易状态：禁用
        .trading_state(TradingState::Disabled)
        // 构建系统
        .build()?
        // 初始化系统
        .init_with_runtime(tokio::runtime::Handle::current())
        .await?;

    // 7. 获取引擎审计快照及其更新流的所有权
    let audit = system.audit.take().unwrap();

    // 8. 启动一个模拟的异步审计流消费者任务
    // 注意：通常你会使用此流来复现引擎状态、持久化事件等。
    // 例如：参见 examples/engine_sync_with_audit_replica_engine_state
    let audit_task = tokio::spawn(async move {
        let mut audit_stream = audit.updates.into_stream();
        while let Some(audit) = audit_stream.next().await {
            debug!(?audit, "AuditStream consumed AuditTick");
            if audit.event.is_terminal() {
                break;
            }
        }
        audit_stream
    });

    // 9. 启用交易
    system.trading_state(TradingState::Enabled);

    // 让示例运行 5 秒...
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 10. 关机前清理：取消订单平掉仓位
    system.cancel_orders(InstrumentFilter::None);
    system.close_positions(InstrumentFilter::None);

    // 11. 系统关机
    let (engine, _shutdown_audit) = system.shutdown().await?;
    let _audit_stream = audit_task.await?;

    // 12. 生成日度交易汇总报告
    let trading_summary = engine
        .trading_summary_generator(RISK_FREE_RETURN)
        .generate(Daily);

    // 13. 打印报告
    trading_summary.print_summary();

    Ok(())
}

fn load_config() -> Result<SystemConfig, Box<dyn std::error::Error>> {
    let file = File::open(FILE_PATH_SYSTEM_CONFIG)?;
    let reader = BufReader::new(file);
    let config = serde_json::from_reader(reader)?;
    Ok(config)
}
