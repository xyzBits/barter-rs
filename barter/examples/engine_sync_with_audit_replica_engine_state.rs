//! # 审计副本状态管理器示例 (Audit Replica Engine State)
//!
//! 该示例演示了如何使用 `StateReplicaManager`，通过引擎的审计流 (Audit Stream) 在本地维护一个引擎状态的实时副本。
//! 这在需要实时监控引擎内部状态（如持仓、订单）但又不想干扰引擎主循环的场景下非常有用。

use barter::{
    EngineEvent,
    engine::{
        audit::state_replica::StateReplicaManager,
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
use barter_integration::snapshot::SnapUpdates;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::{fs::File, io::BufReader, time::Duration};

const FILE_PATH_SYSTEM_CONFIG: &str = "barter/examples/config/system_config.json";

// Risk-free rate of 5% (configure as needed)
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

    // 4. 初始化多交易所市场数据流
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
        .build::<EngineEvent, _>()?
        // 初始化系统
        .init_with_runtime(tokio::runtime::Handle::current())
        .await?;

    // 7. 获取引擎审计快照 (Audit Snapshot) 及其更新流的所有权
    // 这允许我们监听引擎内部状态的所有变更。
    let SnapUpdates {
        snapshot: audit_snapshot,
        updates: audit_updates,
    } = system.audit.take().unwrap();

    // 8. 构造状态副本管理器 (StateReplicaManager)
    // 它会通过审计更新流实时同步并维护一个与引擎一致的 EngineState 副本。
    let mut state_replica_manager = StateReplicaManager::new(audit_snapshot, audit_updates);

    // 9. 在阻塞任务中运行同步的状态副本管理器
    let state_replica_task = tokio::task::spawn_blocking(move || {
        state_replica_manager.run().unwrap();
        state_replica_manager
    });

    // 10. 启用交易
    system.trading_state(TradingState::Enabled);

    // 让示例运行 5 秒...
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 11. 关机前清理：取消订单平掉仓位
    system.cancel_orders(InstrumentFilter::None);
    system.close_positions(InstrumentFilter::None);

    // 12. 系统关机
    let (engine, _shutdown_audit) = system.shutdown().await?;
    state_replica_task.await?;

    // 13. 生成日度交易汇总报告
    let trading_summary = engine
        .trading_summary_generator(RISK_FREE_RETURN)
        .generate(Daily);

    // 14. 打印报告
    trading_summary.print_summary();

    Ok(())
}

fn load_config() -> Result<SystemConfig, Box<dyn std::error::Error>> {
    let file = File::open(FILE_PATH_SYSTEM_CONFIG)?;
    let reader = BufReader::new(file);
    let config = serde_json::from_reader(reader)?;
    Ok(config)
}
