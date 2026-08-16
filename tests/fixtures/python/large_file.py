"""
Large monolithic Python module for AST parser benchmarking and slicing tests.
Contains >2,000 LOC, 100+ functions and classes modeling an algorithmic analytics engine.
"""

from __future__ import annotations
import math
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Any, Callable, Dict, List, Optional, Sequence, Tuple, Union

# ==========================================
# 1. DOMAIN DATA CLASSES & ENUMS (30 Classes)
# ==========================================

@dataclass
class Vector2D:
    x: float
    y: float

@dataclass
class Vector3D:
    x: float
    y: float
    z: float

@dataclass
class Quaternion:
    w: float
    x: float
    y: float
    z: float

@dataclass
class BoundingBox:
    min_val: Vector3D
    max_val: Vector3D

@dataclass
class MarketTick:
    symbol: str
    price: float
    volume: float
    timestamp: float

@dataclass
class OHLCVCandle:
    timestamp: int
    open: float
    high: float
    low: float
    close: float
    volume: float

@dataclass
class OrderBookLevel:
    price: float
    size: float
    order_count: int

@dataclass
class DepthSnapshot:
    bids: list[OrderBookLevel]
    asks: list[OrderBookLevel]
    timestamp: int

@dataclass
class TradeRecord:
    trade_id: str
    symbol: str
    side: str
    price: float
    qty: float
    timestamp: float

@dataclass
class AccountPosition:
    symbol: str
    quantity: float
    entry_price: float
    current_price: float
    unrealized_pnl: float

@dataclass
class RiskAssessment:
    var_95: float
    var_99: float
    expected_shortfall: float
    sharpe_ratio: float
    sortino_ratio: float

@dataclass
class Matrix4x4:
    data: list[list[float]]

@dataclass
class ASTSpan:
    start_line: int
    start_col: int
    end_line: int
    end_col: int

@dataclass
class ASTNodeBase:
    node_type: str
    span: ASTSpan

@dataclass
class ASTIdentifier:
    node_type: str
    name: str
    span: ASTSpan

@dataclass
class ASTLiteral:
    node_type: str
    value: Any
    span: ASTSpan

@dataclass
class ASTBinaryOp:
    node_type: str
    operator: str
    left: ASTNodeBase
    right: ASTNodeBase
    span: ASTSpan

@dataclass
class ASTFuncDef:
    node_type: str
    name: str
    args: list[str]
    body: list[ASTNodeBase]
    span: ASTSpan

@dataclass
class TelemetryMetric:
    metric_name: str
    metric_value: float
    tags: dict[str, str]
    timestamp: float

@dataclass
class NetworkPacket:
    packet_id: int
    source_ip: str
    dest_ip: str
    payload_len: int
    flags: int

@dataclass
class RateLimitWindow:
    key: str
    max_tokens: int
    current_tokens: int
    last_refill: float

@dataclass
class SecurityToken:
    token_id: str
    subject: str
    issued_at: float
    expires_at: float
    claims: dict[str, Any]

@dataclass
class PipelineTask:
    task_id: str
    task_name: str
    priority: int
    payload: dict[str, Any]

@dataclass
class AuditLogEntry:
    actor: str
    action: str
    resource_id: str
    status: str
    timestamp: datetime

@dataclass
class TransactionAnalysisRequest:
    account_id: str
    transactions: list[dict[str, Any]]
    time_window_days: int
    threshold_multiplier: float

@dataclass
class TransactionAnalysisResult:
    account_id: str
    total_count: int
    total_volume: float
    mean_amount: float
    std_dev: float
    anomalies_detected: int
    risk_level: str

@dataclass
class CacheItem:
    key: str
    val: Any
    created_at: float
    ttl: float

@dataclass
class GraphNode:
    node_id: str
    attributes: dict[str, Any]

@dataclass
class GraphEdge:
    source_id: str
    target_id: str
    weight: float

@dataclass
class ExecutionPlan:
    plan_id: str
    steps: list[str]
    estimated_cost_ms: float

# ==========================================
# 2. TARGET FUNCTION & DOMAIN FUNCTIONS (110 Functions)
# ==========================================

def analyze_transactions(
    request: TransactionAnalysisRequest,
    filter_hook: Optional[Callable[[dict[str, Any]], bool]] = None
) -> TransactionAnalysisResult:
    """
    Analyze financial transactions for an account, detecting volume spikes,
    standard deviation deviations, and statistical anomalies.
    """
    valid_amounts: list[float] = []
    for tx in request.transactions:
        if filter_hook and not filter_hook(tx):
            continue
        amt = float(tx.get('amount', 0.0))
        if amt > 0.0:
            valid_amounts.append(amt)

    total_count = len(valid_amounts)
    if total_count == 0:
        return TransactionAnalysisResult(
            account_id=request.account_id,
            total_count=0,
            total_volume=0.0,
            mean_amount=0.0,
            std_dev=0.0,
            anomalies_detected=0,
            risk_level='LOW',
        )

    total_volume = sum(valid_amounts)
    mean = total_volume / total_count
    variance = sum((x - mean) ** 2 for x in valid_amounts) / total_count
    std_dev = math.sqrt(variance)

    threshold = mean + (request.threshold_multiplier * std_dev)
    anomalies = sum(1 for x in valid_amounts if x > threshold)

    risk = 'LOW'
    if anomalies > 5 or (total_count > 0 and anomalies / total_count > 0.1):
        risk = 'HIGH'
    elif anomalies > 0:
        risk = 'MEDIUM'

    return TransactionAnalysisResult(
        account_id=request.account_id,
        total_count=total_count,
        total_volume=round(total_volume, 2),
        mean_amount=round(mean, 2),
        std_dev=round(std_dev, 2),
        anomalies_detected=anomalies,
        risk_level=risk,
    )

def analytics_module_fn_001(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_002(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_003(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_004(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_005(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_006(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_007(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_008(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_009(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_010(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_011(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_012(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_013(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_014(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_015(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_016(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_017(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_018(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_019(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_020(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_021(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_022(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_023(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_024(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_025(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_026(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_027(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_028(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_029(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_030(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_031(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_032(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_033(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_034(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_035(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_036(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_037(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_038(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_039(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_040(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_041(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_042(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_043(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_044(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_045(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_046(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_047(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_048(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_049(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_050(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_051(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_052(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_053(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_054(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_055(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_056(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_057(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_058(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_059(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_060(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_061(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_062(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_063(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_064(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_065(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_066(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_067(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_068(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_069(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_070(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_071(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_072(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_073(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_074(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_075(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_076(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_077(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_078(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_079(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_080(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_081(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_082(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_083(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_084(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_085(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_086(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_087(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_088(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_089(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_090(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_091(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_092(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_093(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_094(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_095(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_096(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_097(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_098(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_099(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_100(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_101(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_102(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_103(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_104(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_105(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_106(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_107(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_108(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_109(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_110(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_111(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_112(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_113(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_114(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_115(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_116(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_117(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_118(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_119(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_120(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_121(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_122(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_123(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_124(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_125(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_126(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_127(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_128(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_129(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_130(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_131(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_132(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_133(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_134(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_135(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_136(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_137(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_138(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_139(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_140(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_141(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_142(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_143(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_144(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_145(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_146(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_147(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_148(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_149(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_150(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_151(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_152(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_153(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )

def analytics_module_fn_154(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:
    """
    Aggregate telemetry metrics matching specific tag key-value pairs.
    """
    aggregated: dict[str, float] = {}
    for m in metrics:
        if m.tags.get(tag_key) == tag_val:
            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value
    return aggregated

def analytics_module_fn_155(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:
    """
    Transform a 3D vector by a 4x4 affine transformation matrix.
    """
    d = matrix.data
    if len(d) < 4 or any(len(row) < 4 for row in d):
        return vector
    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]
    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]
    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]
    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]
    w = nw if abs(nw) > 1e-6 else 1.0
    return Vector3D(x=nx / w, y=ny / w, z=nz / w)

def analytics_module_fn_156(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:
    """
    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.
    Applies standard smoothing multiplier 2 / (period + 1).
    """
    if len(candles) < period or period <= 0:
        return []
    multiplier = 2.0 / (period + 1.0)
    ema_values: list[float] = []
    sma = sum(c.close for c in candles[:period]) / period
    ema_values.append(sma)
    for c in candles[period:]:
        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]
        ema_values.append(current_ema)
    return ema_values

def analytics_module_fn_157(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:
    """
    Perform spherical or linear interpolation between two 3D vectors.
    Clamps interpolation parameter t between 0.0 and 1.0.
    """
    clamped_t = max(0.0, min(1.0, t))
    return Vector3D(
        x=v1.x + (v2.x - v1.x) * clamped_t,
        y=v1.y + (v2.y - v1.y) * clamped_t,
        z=v1.z + (v2.z - v1.z) * clamped_t,
    )

def analytics_module_fn_158(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:
    """
    Compute top-N orderbook depth volume and weighted average spread.
    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).
    """
    bid_vol = sum(b.size for b in depth.bids[:depth_limit])
    ask_vol = sum(a.size for a in depth.asks[:depth_limit])
    best_bid = depth.bids[0].price if depth.bids else 0.0
    best_ask = depth.asks[0].price if depth.asks else 0.0
    spread = max(0.0, best_ask - best_bid)
    return (bid_vol, ask_vol, spread)

def analytics_module_fn_159(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:
    """
    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.
    """
    total_exposure = sum(p.quantity * p.current_price for p in positions)
    total_unrealized = sum(p.unrealized_pnl for p in positions)
    z_score = 1.645 if conf_level <= 0.95 else 2.326
    var = total_exposure * 0.02 * z_score
    sharpe = total_unrealized / max(1.0, var)
    return RiskAssessment(
        var_95=var,
        var_99=var * 1.41,
        expected_shortfall=var * 1.25,
        sharpe_ratio=sharpe,
        sortino_ratio=sharpe * 1.15,
    )
