"""
Generator script for large monolithic test fixtures (>2,000 LOC each)
for TypeScript, Python, Go, and Rust.
"""

import os
from pathlib import Path

FIXTURES_DIR = Path(r"C:\Users\Widlily\Documents\projects\ctxcut\tests\fixtures")

def generate_typescript_large_file():
    path = FIXTURES_DIR / "typescript" / "large_file.ts"
    lines = []
    lines.append("/**")
    lines.append(" * Large monolithic TypeScript module for benchmarking AST parsing and slicing performance.")
    lines.append(" * Contains >2,000 LOC, 120 functions, and 40 interfaces representing a complex data processing engine.")
    lines.append(" */")
    lines.append("")
    lines.append("// ==========================================")
    lines.append("// 1. DOMAIN INTERFACES (40 Interfaces)")
    lines.append("// ==========================================")
    lines.append("")

    interfaces = [
        ("Vector2D", ["x: number;", "y: number;"]),
        ("Vector3D", ["x: number;", "y: number;", "z: number;"]),
        ("Matrix4x4", ["data: number[][];"]),
        ("Quaternion", ["w: number;", "x: number;", "y: number;", "z: number;"]),
        ("BoundingBox3D", ["min: Vector3D;", "max: Vector3D;"]),
        ("Ray3D", ["origin: Vector3D;", "direction: Vector3D;"]),
        ("MeshGeometry", ["vertices: Vector3D[];", "indices: number[];", "normals: Vector3D[];", "uvs: Vector2D[];"]),
        ("MaterialProperties", ["diffuseColor: string;", "roughness: number;", "metalness: number;", "opacity: number;", "emissiveIntensity: number;"]),
        ("RenderSceneNode", ["id: string;", "name: string;", "geometry?: MeshGeometry;", "material?: MaterialProperties;", "transform: Matrix4x4;", "children: RenderSceneNode[];"]),
        ("CameraViewport", ["fieldOfView: number;", "aspectRatio: number;", "nearPlane: number;", "farPlane: number;", "position: Vector3D;", "target: Vector3D;"]),
        ("FinancialTimeSeriesPoint", ["timestamp: number;", "open: number;", "high: number;", "low: number;", "close: number;", "volume: number;"]),
        ("IndicatorConfig", ["period: number;", "smoothingFactor?: number;", "sourceField: 'close' | 'open' | 'high' | 'low' | 'hl2' | 'hlc3';"]),
        ("MovingAverageResult", ["period: number;", "values: number[];", "lastComputedIndex: number;"]),
        ("BollingerBandsResult", ["upper: number[];", "middle: number[];", "lower: number[];", "bandwidth: number[];"]),
        ("MACDResult", ["macdLine: number[];", "signalLine: number[];", "histogram: number[];"]),
        ("OrderBookEntry", ["price: number;", "quantity: number;", "orderCount: number;"]),
        ("DepthOfMarket", ["bids: OrderBookEntry[];", "asks: OrderBookEntry[];", "timestamp: number;", "spread: number;"]),
        ("TradeExecutionReport", ["tradeId: string;", "symbol: string;", "side: 'BUY' | 'SELL';", "price: number;", "quantity: number;", "fee: number;", "timestamp: number;"]),
        ("AccountBalanceSnapshot", ["currency: string;", "available: number;", "reserved: number;", "total: number;", "unrealizedPnl: number;"]),
        ("PortfolioRiskMetrics", ["valueAtRisk95: number;", "valueAtRisk99: number;", "expectedShortfall: number;", "sharpeRatio: number;", "sortinoRatio: number;", "beta: number;"]),
        ("ASTTokenPosition", ["line: number;", "column: number;", "offset: number;"]),
        ("ASTTokenSpan", ["start: ASTTokenPosition;", "end: ASTTokenPosition;"]),
        ("ASTBaseNode", ["type: string;", "span: ASTTokenSpan;"]),
        ("ASTIdentifierNode", ["type: 'Identifier';", "name: string;", "span: ASTTokenSpan;"]),
        ("ASTLiteralNode", ["type: 'Literal';", "raw: string;", "value: string | number | boolean | null;", "span: ASTTokenSpan;"]),
        ("ASTBinaryExpressionNode", ["type: 'BinaryExpression';", "operator: string;", "left: ASTBaseNode;", "right: ASTBaseNode;", "span: ASTTokenSpan;"]),
        ("ASTCallExpressionNode", ["type: 'CallExpression';", "callee: ASTBaseNode;", "arguments: ASTBaseNode[];", "span: ASTTokenSpan;"]),
        ("ASTBlockStatementNode", ["type: 'BlockStatement';", "body: ASTBaseNode[];", "span: ASTTokenSpan;"]),
        ("ASTFunctionDeclarationNode", ["type: 'FunctionDeclaration';", "name: ASTIdentifierNode;", "parameters: ASTIdentifierNode[];", "body: ASTBlockStatementNode;", "returnType?: string;", "span: ASTTokenSpan;"]),
        ("ASTModuleProgramNode", ["type: 'Program';", "statements: ASTBaseNode[];", "sourceFile: string;", "span: ASTTokenSpan;"]),
        ("NetworkPacketHeader", ["packetId: number;", "protocolVersion: number;", "flags: number;", "payloadLength: number;", "checksum: number;"]),
        ("NetworkPacket", ["header: NetworkPacketHeader;", "payload: Uint8Array;"]),
        ("RateLimitPolicy", ["maxRequests: number;", "windowSeconds: number;", "burstCapacity: number;"]),
        ("SessionTokenDescriptor", ["tokenId: string;", "subjectId: string;", "issuedAt: number;", "expiresAt: number;", "scopes: string[];"]),
        ("CacheEntry<T>", ["key: string;", "value: T;", "createdAt: number;", "timeToLiveMs: number;", "accessCount: number;"]),
        ("PipelineStepContext<TInput, TOutput>", ["stepName: string;", "input: TInput;", "output?: TOutput;", "durationMs: number;", "status: 'PENDING' | 'RUNNING' | 'COMPLETED' | 'FAILED';"]),
        ("TelemetryEvent", ["eventId: string;", "eventName: string;", "severity: 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';", "timestamp: number;", "attributes: Record<string, string | number | boolean>;"]),
        ("HealthCheckStatus", ["serviceName: string;", "status: 'UP' | 'DOWN' | 'DEGRADED';", "latencyMs: number;", "details: Record<string, unknown>;"]),
        ("OrderBatchProcessingRequest", ["batchId: string;", "merchantId: string;", "orders: Array<{ orderId: string; amount: number; currency: string; items: Array<{ sku: string; qty: number; price: number }> }>;", "priority: 'LOW' | 'NORMAL' | 'HIGH' | 'CRITICAL';"]),
        ("OrderBatchProcessingSummary", ["batchId: string;", "totalOrders: number;", "processedOrders: number;", "failedOrders: number;", "totalVolume: number;", "processingTimeMs: number;", "completedAt: string;"])
    ]

    for name, fields in interfaces:
        lines.append(f"export interface {name} {{")
        for f in fields:
            lines.append(f"    {f}")
        lines.append("}")
        lines.append("")

    lines.append("// ==========================================")
    lines.append("// 2. DOMAIN FUNCTIONS (120 Functions)")
    lines.append("// ==========================================")
    lines.append("")

    # Target function processBatchOrders
    lines.append("export async function processBatchOrders(")
    lines.append("    request: OrderBatchProcessingRequest,")
    lines.append("    validator?: (orderId: string) => Promise<boolean>")
    lines.append("): Promise<OrderBatchProcessingSummary> {")
    lines.append("    const startTime = Date.now();")
    lines.append("    let totalVolume = 0;")
    lines.append("    let processedCount = 0;")
    lines.append("    let failedCount = 0;")
    lines.append("")
    lines.append("    for (const order of request.orders) {")
    lines.append("        if (validator) {")
    lines.append("            const isValid = await validator(order.orderId);")
    lines.append("            if (!isValid) {")
    lines.append("                failedCount++;")
    lines.append("                continue;")
    lines.append("            }")
    lines.append("        }")
    lines.append("        if (order.amount <= 0 || order.items.length === 0) {")
    lines.append("            failedCount++;")
    lines.append("            continue;")
    lines.append("        }")
    lines.append("        let orderSum = 0;")
    lines.append("        for (const item of order.items) {")
    lines.append("            orderSum += item.qty * item.price;")
    lines.append("        }")
    lines.append("        totalVolume += orderSum;")
    lines.append("        processedCount++;")
    lines.append("    }")
    lines.append("")
    lines.append("    return {")
    lines.append("        batchId: request.batchId,")
    lines.append("        totalOrders: request.orders.length,")
    lines.append("        processedOrders: processedCount,")
    lines.append("        failedOrders: failedCount,")
    lines.append("        totalVolume,")
    lines.append("        processingTimeMs: Date.now() - startTime,")
    lines.append("        completedAt: new Date().toISOString(),")
    lines.append("    };")
    lines.append("}")
    lines.append("")

    for i in range(1, 120):
        mod = i % 6
        fn_name = f"computeEngineFunction_{i:03d}"
        if mod == 0:
            lines.append(f"export function {fn_name}(points: FinancialTimeSeriesPoint[], period: number): MovingAverageResult {{")
            lines.append(f"    const values: number[] = [];")
            lines.append(f"    if (points.length < period || period <= 0) {{")
            lines.append(f"        return {{ period, values, lastComputedIndex: -1 }};")
            lines.append(f"    }}")
            lines.append(f"    let currentSum = 0;")
            lines.append(f"    for (let idx = 0; idx < period; idx++) {{")
            lines.append(f"        currentSum += points[idx].close;")
            lines.append(f"    }}")
            lines.append(f"    values.push(currentSum / period);")
            lines.append(f"    for (let idx = period; idx < points.length; idx++) {{")
            lines.append(f"        currentSum += points[idx].close - points[idx - period].close;")
            lines.append(f"        values.push(currentSum / period);")
            lines.append(f"    }}")
            lines.append(f"    return {{")
            lines.append(f"        period,")
            lines.append(f"        values,")
            lines.append(f"        lastComputedIndex: points.length - 1,")
            lines.append(f"    }};")
            lines.append("}")
        elif mod == 1:
            lines.append(f"export function {fn_name}(vecA: Vector3D, vecB: Vector3D, weight: number): Vector3D {{")
            lines.append(f"    const clampedWeight = Math.max(0, Math.min(1, weight));")
            lines.append(f"    const inv = 1 - clampedWeight;")
            lines.append(f"    return {{")
            lines.append(f"        x: vecA.x * inv + vecB.x * clampedWeight,")
            lines.append(f"        y: vecA.y * inv + vecB.y * clampedWeight,")
            lines.append(f"        z: vecA.z * inv + vecB.z * clampedWeight,")
            lines.append(f"    }};")
            lines.append("}")
        elif mod == 2:
            lines.append(f"export function {fn_name}(depth: DepthOfMarket, threshold: number): {{ cumulativeBid: number; cumulativeAsk: number; imbalanceRatio: number }} {{")
            lines.append(f"    let cumulativeBid = 0;")
            lines.append(f"    let cumulativeAsk = 0;")
            lines.append(f"    for (const bid of depth.bids) {{")
            lines.append(f"        if (bid.price >= threshold) {{")
            lines.append(f"            cumulativeBid += bid.quantity * bid.price;")
            lines.append(f"        }}")
            lines.append(f"    }}")
            lines.append(f"    for (const ask of depth.asks) {{")
            lines.append(f"        if (ask.price <= threshold) {{")
            lines.append(f"            cumulativeAsk += ask.quantity * ask.price;")
            lines.append(f"        }}")
            lines.append(f"    }}")
            lines.append(f"    const total = cumulativeBid + cumulativeAsk;")
            lines.append(f"    const imbalanceRatio = total > 0 ? (cumulativeBid - cumulativeAsk) / total : 0;")
            lines.append(f"    return {{ cumulativeBid, cumulativeAsk, imbalanceRatio }};")
            lines.append("}")
        elif mod == 3:
            lines.append(f"export function {fn_name}(node: RenderSceneNode, scaleFactor: number): BoundingBox3D {{")
            lines.append(f"    let minX = Infinity, minY = Infinity, minZ = Infinity;")
            lines.append(f"    let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;")
            lines.append(f"    if (node.geometry) {{")
            lines.append(f"        for (const v of node.geometry.vertices) {{")
            lines.append(f"            minX = Math.min(minX, v.x * scaleFactor);")
            lines.append(f"            minY = Math.min(minY, v.y * scaleFactor);")
            lines.append(f"            minZ = Math.min(minZ, v.z * scaleFactor);")
            lines.append(f"            maxX = Math.max(maxX, v.x * scaleFactor);")
            lines.append(f"            maxY = Math.max(maxY, v.y * scaleFactor);")
            lines.append(f"            maxZ = Math.max(maxZ, v.z * scaleFactor);")
            lines.append(f"        }}")
            lines.append(f"    }} else {{")
            lines.append(f"        minX = minY = minZ = 0;")
            lines.append(f"        maxX = maxY = maxZ = 1;")
            lines.append(f"    }}")
            lines.append(f"    return {{")
            lines.append(f"        min: {{ x: minX, y: minY, z: minZ }},")
            lines.append(f"        max: {{ x: maxX, y: maxY, z: maxZ }},")
            lines.append(f"    }};")
            lines.append("}")
        elif mod == 4:
            lines.append(f"export function {fn_name}(events: TelemetryEvent[], filterSeverity: 'DEBUG' | 'INFO' | 'WARN' | 'ERROR'): Map<string, number> {{")
            lines.append(f"    const counts = new Map<string, number>();")
            lines.append(f"    for (const ev of events) {{")
            lines.append(f"        if (ev.severity === filterSeverity) {{")
            lines.append(f"            const current = counts.get(ev.eventName) ?? 0;")
            lines.append(f"            counts.set(ev.eventName, current + 1);")
            lines.append(f"        }}")
            lines.append(f"    }}")
            lines.append(f"    return counts;")
            lines.append("}")
        else:
            lines.append(f"export function {fn_name}(balances: AccountBalanceSnapshot[], baseCurrency: string): PortfolioRiskMetrics {{")
            lines.append(f"    let totalValue = 0;")
            lines.append(f"    let totalUnrealized = 0;")
            lines.append(f"    for (const bal of balances) {{")
            lines.append(f"        totalValue += bal.total;")
            lines.append(f"        totalUnrealized += bal.unrealizedPnl;")
            lines.append(f"    }}")
            lines.append(f"    const var95 = totalValue * 0.05 * 1.645;")
            lines.append(f"    const var99 = totalValue * 0.05 * 2.326;")
            lines.append(f"    const sharpe = totalValue > 0 ? totalUnrealized / (totalValue * 0.15) : 0;")
            lines.append(f"    return {{")
            lines.append(f"        valueAtRisk95: var95,")
            lines.append(f"        valueAtRisk99: var99,")
            lines.append(f"        expectedShortfall: var99 * 1.25,")
            lines.append(f"        sharpeRatio: sharpe,")
            lines.append(f"        sortinoRatio: sharpe * 1.1,")
            lines.append(f"        beta: 1.02,")
            lines.append(f"    }};")
            lines.append("}")
        lines.append("")

    content = "\n".join(lines)
    path.write_text(content, encoding="utf-8")
    print(f"Generated {path}: {len(lines)} lines")

def generate_python_large_file():
    path = FIXTURES_DIR / "python" / "large_file.py"
    lines = []
    lines.append('"""')
    lines.append('Large monolithic Python module for AST parser benchmarking and slicing tests.')
    lines.append('Contains >2,000 LOC, 100+ functions and classes modeling an algorithmic analytics engine.')
    lines.append('"""')
    lines.append('')
    lines.append('from __future__ import annotations')
    lines.append('import math')
    lines.append('from dataclasses import dataclass, field')
    lines.append('from datetime import datetime')
    lines.append('from enum import Enum')
    lines.append('from typing import Any, Callable, Dict, List, Optional, Sequence, Tuple, Union')
    lines.append('')
    lines.append('# ==========================================')
    lines.append('# 1. DOMAIN DATA CLASSES & ENUMS (30 Classes)')
    lines.append('# ==========================================')
    lines.append('')

    classes = [
        ("Vector2D", [("x", "float"), ("y", "float")]),
        ("Vector3D", [("x", "float"), ("y", "float"), ("z", "float")]),
        ("Quaternion", [("w", "float"), ("x", "float"), ("y", "float"), ("z", "float")]),
        ("BoundingBox", [("min_val", "Vector3D"), ("max_val", "Vector3D")]),
        ("MarketTick", [("symbol", "str"), ("price", "float"), ("volume", "float"), ("timestamp", "float")]),
        ("OHLCVCandle", [("timestamp", "int"), ("open", "float"), ("high", "float"), ("low", "float"), ("close", "float"), ("volume", "float")]),
        ("OrderBookLevel", [("price", "float"), ("size", "float"), ("order_count", "int")]),
        ("DepthSnapshot", [("bids", "list[OrderBookLevel]"), ("asks", "list[OrderBookLevel]"), ("timestamp", "int")]),
        ("TradeRecord", [("trade_id", "str"), ("symbol", "str"), ("side", "str"), ("price", "float"), ("qty", "float"), ("timestamp", "float")]),
        ("AccountPosition", [("symbol", "str"), ("quantity", "float"), ("entry_price", "float"), ("current_price", "float"), ("unrealized_pnl", "float")]),
        ("RiskAssessment", [("var_95", "float"), ("var_99", "float"), ("expected_shortfall", "float"), ("sharpe_ratio", "float"), ("sortino_ratio", "float")]),
        ("Matrix4x4", [("data", "list[list[float]]")]),
        ("ASTSpan", [("start_line", "int"), ("start_col", "int"), ("end_line", "int"), ("end_col", "int")]),
        ("ASTNodeBase", [("node_type", "str"), ("span", "ASTSpan")]),
        ("ASTIdentifier", [("node_type", "str"), ("name", "str"), ("span", "ASTSpan")]),
        ("ASTLiteral", [("node_type", "str"), ("value", "Any"), ("span", "ASTSpan")]),
        ("ASTBinaryOp", [("node_type", "str"), ("operator", "str"), ("left", "ASTNodeBase"), ("right", "ASTNodeBase"), ("span", "ASTSpan")]),
        ("ASTFuncDef", [("node_type", "str"), ("name", "str"), ("args", "list[str]"), ("body", "list[ASTNodeBase]"), ("span", "ASTSpan")]),
        ("TelemetryMetric", [("metric_name", "str"), ("metric_value", "float"), ("tags", "dict[str, str]"), ("timestamp", "float")]),
        ("NetworkPacket", [("packet_id", "int"), ("source_ip", "str"), ("dest_ip", "str"), ("payload_len", "int"), ("flags", "int")]),
        ("RateLimitWindow", [("key", "str"), ("max_tokens", "int"), ("current_tokens", "int"), ("last_refill", "float")]),
        ("SecurityToken", [("token_id", "str"), ("subject", "str"), ("issued_at", "float"), ("expires_at", "float"), ("claims", "dict[str, Any]")]),
        ("PipelineTask", [("task_id", "str"), ("task_name", "str"), ("priority", "int"), ("payload", "dict[str, Any]")]),
        ("AuditLogEntry", [("actor", "str"), ("action", "str"), ("resource_id", "str"), ("status", "str"), ("timestamp", "datetime")]),
        ("TransactionAnalysisRequest", [("account_id", "str"), ("transactions", "list[dict[str, Any]]"), ("time_window_days", "int"), ("threshold_multiplier", "float")]),
        ("TransactionAnalysisResult", [("account_id", "str"), ("total_count", "int"), ("total_volume", "float"), ("mean_amount", "float"), ("std_dev", "float"), ("anomalies_detected", "int"), ("risk_level", "str")]),
        ("CacheItem", [("key", "str"), ("val", "Any"), ("created_at", "float"), ("ttl", "float")]),
        ("GraphNode", [("node_id", "str"), ("attributes", "dict[str, Any]")]),
        ("GraphEdge", [("source_id", "str"), ("target_id", "str"), ("weight", "float")]),
        ("ExecutionPlan", [("plan_id", "str"), ("steps", "list[str]"), ("estimated_cost_ms", "float")])
    ]

    for name, fields in classes:
        lines.append("@dataclass")
        lines.append(f"class {name}:")
        for fname, ftype in fields:
            lines.append(f"    {fname}: {ftype}")
        lines.append("")

    lines.append("# ==========================================")
    lines.append("# 2. TARGET FUNCTION & DOMAIN FUNCTIONS (110 Functions)")
    lines.append("# ==========================================")
    lines.append("")

    # Target function analyze_transactions
    lines.append("def analyze_transactions(")
    lines.append("    request: TransactionAnalysisRequest,")
    lines.append("    filter_hook: Optional[Callable[[dict[str, Any]], bool]] = None")
    lines.append(") -> TransactionAnalysisResult:")
    lines.append('    """')
    lines.append('    Analyze financial transactions for an account, detecting volume spikes,')
    lines.append('    standard deviation deviations, and statistical anomalies.')
    lines.append('    """')
    lines.append("    valid_amounts: list[float] = []")
    lines.append("    for tx in request.transactions:")
    lines.append("        if filter_hook and not filter_hook(tx):")
    lines.append("            continue")
    lines.append("        amt = float(tx.get('amount', 0.0))")
    lines.append("        if amt > 0.0:")
    lines.append("            valid_amounts.append(amt)")
    lines.append("")
    lines.append("    total_count = len(valid_amounts)")
    lines.append("    if total_count == 0:")
    lines.append("        return TransactionAnalysisResult(")
    lines.append("            account_id=request.account_id,")
    lines.append("            total_count=0,")
    lines.append("            total_volume=0.0,")
    lines.append("            mean_amount=0.0,")
    lines.append("            std_dev=0.0,")
    lines.append("            anomalies_detected=0,")
    lines.append("            risk_level='LOW',")
    lines.append("        )")
    lines.append("")
    lines.append("    total_volume = sum(valid_amounts)")
    lines.append("    mean = total_volume / total_count")
    lines.append("    variance = sum((x - mean) ** 2 for x in valid_amounts) / total_count")
    lines.append("    std_dev = math.sqrt(variance)")
    lines.append("")
    lines.append("    threshold = mean + (request.threshold_multiplier * std_dev)")
    lines.append("    anomalies = sum(1 for x in valid_amounts if x > threshold)")
    lines.append("")
    lines.append("    risk = 'LOW'")
    lines.append("    if anomalies > 5 or (total_count > 0 and anomalies / total_count > 0.1):")
    lines.append("        risk = 'HIGH'")
    lines.append("    elif anomalies > 0:")
    lines.append("        risk = 'MEDIUM'")
    lines.append("")
    lines.append("    return TransactionAnalysisResult(")
    lines.append("        account_id=request.account_id,")
    lines.append("        total_count=total_count,")
    lines.append("        total_volume=round(total_volume, 2),")
    lines.append("        mean_amount=round(mean, 2),")
    lines.append("        std_dev=round(std_dev, 2),")
    lines.append("        anomalies_detected=anomalies,")
    lines.append("        risk_level=risk,")
    lines.append("    )")
    lines.append("")

    for i in range(1, 160):
        fn_name = f"analytics_module_fn_{i:03d}"
        mod = i % 6
        if mod == 0:
            lines.append(f"def {fn_name}(candles: Sequence[OHLCVCandle], period: int = 14) -> list[float]:")
            lines.append('    """')
            lines.append('    Calculate Exponential Moving Average (EMA) for a sequence of OHLCV candles.')
            lines.append('    Applies standard smoothing multiplier 2 / (period + 1).')
            lines.append('    """')
            lines.append("    if len(candles) < period or period <= 0:")
            lines.append("        return []")
            lines.append("    multiplier = 2.0 / (period + 1.0)")
            lines.append("    ema_values: list[float] = []")
            lines.append("    sma = sum(c.close for c in candles[:period]) / period")
            lines.append("    ema_values.append(sma)")
            lines.append("    for c in candles[period:]:")
            lines.append("        current_ema = (c.close - ema_values[-1]) * multiplier + ema_values[-1]")
            lines.append("        ema_values.append(current_ema)")
            lines.append("    return ema_values")
        elif mod == 1:
            lines.append(f"def {fn_name}(v1: Vector3D, v2: Vector3D, t: float) -> Vector3D:")
            lines.append('    """')
            lines.append('    Perform spherical or linear interpolation between two 3D vectors.')
            lines.append('    Clamps interpolation parameter t between 0.0 and 1.0.')
            lines.append('    """')
            lines.append("    clamped_t = max(0.0, min(1.0, t))")
            lines.append("    return Vector3D(")
            lines.append("        x=v1.x + (v2.x - v1.x) * clamped_t,")
            lines.append("        y=v1.y + (v2.y - v1.y) * clamped_t,")
            lines.append("        z=v1.z + (v2.z - v1.z) * clamped_t,")
            lines.append("    )")
        elif mod == 2:
            lines.append(f"def {fn_name}(depth: DepthSnapshot, depth_limit: int = 10) -> Tuple[float, float, float]:")
            lines.append('    """')
            lines.append('    Compute top-N orderbook depth volume and weighted average spread.')
            lines.append('    Returns a 3-tuple of (bid_volume, ask_volume, bid_ask_spread).')
            lines.append('    """')
            lines.append("    bid_vol = sum(b.size for b in depth.bids[:depth_limit])")
            lines.append("    ask_vol = sum(a.size for a in depth.asks[:depth_limit])")
            lines.append("    best_bid = depth.bids[0].price if depth.bids else 0.0")
            lines.append("    best_ask = depth.asks[0].price if depth.asks else 0.0")
            lines.append("    spread = max(0.0, best_ask - best_bid)")
            lines.append("    return (bid_vol, ask_vol, spread)")
        elif mod == 3:
            lines.append(f"def {fn_name}(positions: Sequence[AccountPosition], conf_level: float = 0.95) -> RiskAssessment:")
            lines.append('    """')
            lines.append('    Compute parametric Value at Risk (VaR) and Sortino/Sharpe risk metrics.')
            lines.append('    """')
            lines.append("    total_exposure = sum(p.quantity * p.current_price for p in positions)")
            lines.append("    total_unrealized = sum(p.unrealized_pnl for p in positions)")
            lines.append("    z_score = 1.645 if conf_level <= 0.95 else 2.326")
            lines.append("    var = total_exposure * 0.02 * z_score")
            lines.append("    sharpe = total_unrealized / max(1.0, var)")
            lines.append("    return RiskAssessment(")
            lines.append("        var_95=var,")
            lines.append("        var_99=var * 1.41,")
            lines.append("        expected_shortfall=var * 1.25,")
            lines.append("        sharpe_ratio=sharpe,")
            lines.append("        sortino_ratio=sharpe * 1.15,")
            lines.append("    )")
        elif mod == 4:
            lines.append(f"def {fn_name}(metrics: Sequence[TelemetryMetric], tag_key: str, tag_val: str) -> dict[str, float]:")
            lines.append('    """')
            lines.append('    Aggregate telemetry metrics matching specific tag key-value pairs.')
            lines.append('    """')
            lines.append("    aggregated: dict[str, float] = {}")
            lines.append("    for m in metrics:")
            lines.append("        if m.tags.get(tag_key) == tag_val:")
            lines.append("            aggregated[m.metric_name] = aggregated.get(m.metric_name, 0.0) + m.metric_value")
            lines.append("    return aggregated")
        else:
            lines.append(f"def {fn_name}(matrix: Matrix4x4, vector: Vector3D) -> Vector3D:")
            lines.append('    """')
            lines.append('    Transform a 3D vector by a 4x4 affine transformation matrix.')
            lines.append('    """')
            lines.append("    d = matrix.data")
            lines.append("    if len(d) < 4 or any(len(row) < 4 for row in d):")
            lines.append("        return vector")
            lines.append("    nx = d[0][0] * vector.x + d[0][1] * vector.y + d[0][2] * vector.z + d[0][3]")
            lines.append("    ny = d[1][0] * vector.x + d[1][1] * vector.y + d[1][2] * vector.z + d[1][3]")
            lines.append("    nz = d[2][0] * vector.x + d[2][1] * vector.y + d[2][2] * vector.z + d[2][3]")
            lines.append("    nw = d[3][0] * vector.x + d[3][1] * vector.y + d[3][2] * vector.z + d[3][3]")
            lines.append("    w = nw if abs(nw) > 1e-6 else 1.0")
            lines.append("    return Vector3D(x=nx / w, y=ny / w, z=nz / w)")
        lines.append("")

    content = "\n".join(lines)
    path.write_text(content, encoding="utf-8")
    print(f"Generated {path}: {len(lines)} lines")

def generate_go_large_file():
    path = FIXTURES_DIR / "go" / "large_file.go"
    lines = []
    lines.append('package fixtures')
    lines.append('')
    lines.append('import (')
    lines.append('\t"context"')
    lines.append('\t"fmt"')
    lines.append('\t"math"')
    lines.append('\t"sync"')
    lines.append('\t"time"')
    lines.append(')')
    lines.append('')
    lines.append('// ==========================================')
    lines.append('// 1. DOMAIN STRUCTS & TYPES (35 Structs)')
    lines.append('// ==========================================')
    lines.append('')

    structs = [
        ("ClusterNode", [("ID", "string"), ("Address", "string"), ("Port", "int"), ("IsLeader", "bool"), ("Heartbeat", "time.Time")]),
        ("ClusterEvent", [("EventID", "string"), ("Type", "string"), ("NodeID", "string"), ("Payload", "[]byte"), ("Timestamp", "time.Time")]),
        ("ReconciliationResult", [("ProcessedCount", "int"), ("ErrorCount", "int"), ("LeaderNodeID", "string"), ("ClusterHealth", "string"), ("Duration", "time.Duration")]),
        ("Vector3D", [("X", "float64"), ("Y", "float64"), ("Z", "float64")]),
        ("BoundingBox3D", [("Min", "Vector3D"), ("Max", "Vector3D")]),
        ("Matrix4x4", [("M", "[4][4]float64")]),
        ("MarketTick", [("Symbol", "string"), ("Price", "float64"), ("Volume", "float64"), ("Timestamp", "int64")]),
        ("OrderBookLevel", [("Price", "float64"), ("Quantity", "float64"), ("OrderCount", "int")]),
        ("DepthSnapshot", [("Bids", "[]OrderBookLevel"), ("Asks", "[]OrderBookLevel"), ("Timestamp", "int64")]),
        ("PositionReport", [("Symbol", "string"), ("Quantity", "float64"), ("AvgCost", "float64"), ("UnrealizedPnL", "float64")]),
        ("RiskMetrics", [("VaR95", "float64"), ("VaR99", "float64"), ("Sharpe", "float64"), ("Beta", "float64")]),
        ("ASTNode", [("Type", "string"), ("StartPos", "int"), ("EndPos", "int"), ("Source", "string")]),
        ("ASTIdentifier", [("ASTNode", "ASTNode"), ("Name", "string")]),
        ("ASTBinaryExpr", [("ASTNode", "ASTNode"), ("Op", "string"), ("Left", "*ASTNode"), ("Right", "*ASTNode")]),
        ("TelemetryPoint", [("Name", "string"), ("Value", "float64"), ("Tags", "map[string]string"), ("RecordedAt", "time.Time")]),
        ("NetworkPacket", [("Seq", "uint64"), ("Flags", "uint32"), ("Payload", "[]byte"), ("Checksum", "uint32")]),
        ("TokenBucket", [("Rate", "float64"), ("Capacity", "float64"), ("Tokens", "float64"), ("LastRefill", "time.Time")]),
        ("RingBuffer", [("Data", "[]interface{}"), ("Head", "int"), ("Tail", "int"), ("Capacity", "int")]),
        ("BloomFilter", [("Bitset", "[]uint64"), ("KHashes", "int"), ("Size", "uint64")]),
        ("SessionMeta", [("SessionID", "string"), ("UserID", "string"), ("ExpiresAt", "time.Time"), ("IsActive", "bool")]),
        ("AuditEntry", [("Action", "string"), ("Actor", "string"), ("Target", "string"), ("Timestamp", "time.Time")]),
        ("CronSchedule", [("Expr", "string"), ("NextRun", "time.Time"), ("JobName", "string")]),
        ("HTTPRouteMatch", [("Method", "string"), ("Pattern", "string"), ("HandlerName", "string")]),
        ("CacheItem", [("Key", "string"), ("Value", "[]byte"), ("TTL", "time.Duration"), ("CreatedAt", "time.Time")]),
        ("WorkerTask", [("TaskID", "string"), ("Priority", "int"), ("Payload", "[]byte"), ("Retries", "int")]),
        ("WorkerPoolStatus", [("ActiveWorkers", "int"), ("IdleWorkers", "int"), ("QueuedTasks", "int")]),
        ("KVMutation", [("Key", "string"), ("Value", "[]byte"), ("OpType", "string"), ("Version", "uint64")]),
        ("RaftLogEntry", [("Term", "uint64"), ("Index", "uint64"), ("Data", "[]byte")]),
        ("RaftState", [("CurrentTerm", "uint64"), ("VotedFor", "string"), ("CommitIndex", "uint64"), ("LastApplied", "uint64")]),
        ("MetricsSummary", [("TotalCount", "int64"), ("Sum", "float64"), ("Min", "float64"), ("Max", "float64"), ("Mean", "float64")]),
    ]

    for name, fields in structs:
        lines.append(f"type {name} struct {{")
        for fname, ftype in fields:
            lines.append(f"\t{fname} {ftype}")
        lines.append("}")
        lines.append("")

    lines.append('// ==========================================')
    lines.append('// 2. TARGET FUNCTION & DOMAIN METHODS (130 Functions)')
    lines.append('// ==========================================')
    lines.append('')

    # Target function HandleClusterEvents
    lines.append('// HandleClusterEvents reconciles inbound events across a distributed cluster.')
    lines.append('func HandleClusterEvents(ctx context.Context, nodes []ClusterNode, events []ClusterEvent) (*ReconciliationResult, error) {')
    lines.append('\tstart := time.Now()')
    lines.append('\tif len(nodes) == 0 {')
    lines.append('\t\treturn nil, fmt.Errorf("cannot reconcile empty cluster node list")')
    lines.append('\t}')
    lines.append('\tleaderID := ""')
    lines.append('\tfor _, node := range nodes {')
    lines.append('\t\tif node.IsLeader {')
    lines.append('\t\t\tleaderID = node.ID')
    lines.append('\t\t\tbreak')
    lines.append('\t\t}')
    lines.append('\t}')
    lines.append('\tif leaderID == "" && len(nodes) > 0 {')
    lines.append('\t\tleaderID = nodes[0].ID')
    lines.append('\t}')
    lines.append('')
    lines.append('\tprocessed := 0')
    lines.append('\terrs := 0')
    lines.append('\tfor _, ev := range events {')
    lines.append('\t\tselect {')
    lines.append('\t\tcase <-ctx.Done():')
    lines.append('\t\t\treturn nil, ctx.Err()')
    lines.append('\t\tdefault:')
    lines.append('\t\t\tif len(ev.Payload) > 0 {')
    lines.append('\t\t\t\tprocessed++')
    lines.append('\t\t\t} else {')
    lines.append('\t\t\t\terrs++')
    lines.append('\t\t\t}')
    lines.append('\t\t}')
    lines.append('\t}')
    lines.append('')
    lines.append('\thealth := "HEALTHY"')
    lines.append('\tif errs > processed {')
    lines.append('\t\thealth = "DEGRADED"')
    lines.append('\t}')
    lines.append('')
    lines.append('\treturn &ReconciliationResult{')
    lines.append('\t\tProcessedCount: processed,')
    lines.append('\t\tErrorCount:     errs,')
    lines.append('\t\tLeaderNodeID:   leaderID,')
    lines.append('\t\tClusterHealth:  health,')
    lines.append('\t\tDuration:       time.Since(start),')
    lines.append('\t}, nil')
    lines.append('}')
    lines.append('')

    # Generate 130 additional functions
    for i in range(1, 131):
        fn_name = f"ComputeGoClusterMetric_{i:03d}"
        mod = i % 5
        if mod == 0:
            lines.append(f"// {fn_name} computes moving average for financial ticks.")
            lines.append(f"func {fn_name}(ticks []MarketTick, window int) []float64 {{")
            lines.append("\tif len(ticks) < window || window <= 0 {")
            lines.append("\t\treturn nil")
            lines.append("\t}")
            lines.append("\tresult := make([]float64, 0, len(ticks)-window+1)")
            lines.append("\tsum := 0.0")
            lines.append("\tfor i := 0; i < window; i++ {")
            lines.append("\t\tsum += ticks[i].Price")
            lines.append("\t}")
            lines.append("\tresult = append(result, sum/float64(window))")
            lines.append("\tfor i := window; i < len(ticks); i++ {")
            lines.append("\t\tsum += ticks[i].Price - ticks[i-window].Price")
            lines.append("\t\tresult = append(result, sum/float64(window))")
            lines.append("\t}")
            lines.append("\treturn result")
            lines.append("}")
        elif mod == 1:
            lines.append(f"// {fn_name} performs vector transformation.")
            lines.append(f"func {fn_name}(v1, v2 Vector3D, alpha float64) Vector3D {{")
            lines.append("\talpha = math.Max(0.0, math.Min(1.0, alpha))")
            lines.append("\tinv := 1.0 - alpha")
            lines.append("\treturn Vector3D{")
            lines.append("\t\tX: v1.X*inv + v2.X*alpha,")
            lines.append("\t\tY: v1.Y*inv + v2.Y*alpha,")
            lines.append("\t\tZ: v1.Z*inv + v2.Z*alpha,")
            lines.append("\t}")
            lines.append("}")
        elif mod == 2:
            lines.append(f"// {fn_name} computes depth spread and liquidity imbalance.")
            lines.append(f"func {fn_name}(depth DepthSnapshot) (float64, float64, float64) {{")
            lines.append("\tbidVol := 0.0")
            lines.append("\tfor _, b := range depth.Bids {")
            lines.append("\t\tbidVol += b.Price * b.Quantity")
            lines.append("\t}")
            lines.append("\taskVol := 0.0")
            lines.append("\tfor _, a := range depth.Asks {")
            lines.append("\t\taskVol += a.Price * a.Quantity")
            lines.append("\t}")
            lines.append("\tspread := 0.0")
            lines.append("\tif len(depth.Asks) > 0 && len(depth.Bids) > 0 {")
            lines.append("\t\tspread = depth.Asks[0].Price - depth.Bids[0].Price")
            lines.append("\t}")
            lines.append("\treturn bidVol, askVol, spread")
            lines.append("}")
        elif mod == 3:
            lines.append(f"// {fn_name} calculates portfolio risk indicators.")
            lines.append(f"func {fn_name}(positions []PositionReport) RiskMetrics {{")
            lines.append("\ttotalExp := 0.0")
            lines.append("\ttotalPnL := 0.0")
            lines.append("\tfor _, p := range positions {")
            lines.append("\t\ttotalExp += p.Quantity * p.AvgCost")
            lines.append("\t\ttotalPnL += p.UnrealizedPnL")
            lines.append("\t}")
            lines.append("\tvar95 := totalExp * 0.02 * 1.645")
            lines.append("\tsharpe := 0.0")
            lines.append("\tif var95 > 0 {")
            lines.append("\t\tsharpe = totalPnL / var95")
            lines.append("\t}")
            lines.append("\treturn RiskMetrics{")
            lines.append("\t\tVaR95:  var95,")
            lines.append("\t\tVaR99:  var95 * 1.41,")
            lines.append("\t\tSharpe: sharpe,")
            lines.append("\t\tBeta:   1.05,")
            lines.append("\t}")
            lines.append("}")
        else:
            lines.append(f"// {fn_name} computes metric statistics summary.")
            lines.append(f"func {fn_name}(pts []TelemetryPoint) MetricsSummary {{")
            lines.append("\tif len(pts) == 0 {")
            lines.append("\t\treturn MetricsSummary{}")
            lines.append("\t}")
            lines.append("\tsum := 0.0")
            lines.append("\tminVal := math.MaxFloat64")
            lines.append("\tmaxVal := -math.MaxFloat64")
            lines.append("\tfor _, p := range pts {")
            lines.append("\t\tsum += p.Value")
            lines.append("\t\tif p.Value < minVal {")
            lines.append("\t\t\tminVal = p.Value")
            lines.append("\t\t}")
            lines.append("\t\tif p.Value > maxVal {")
            lines.append("\t\t\tmaxVal = p.Value")
            lines.append("\t\t}")
            lines.append("\t}")
            lines.append("\treturn MetricsSummary{")
            lines.append("\t\tTotalCount: int64(len(pts)),")
            lines.append("\t\tSum:        sum,")
            lines.append("\t\tMin:        minVal,")
            lines.append("\t\tMax:        maxVal,")
            lines.append("\t\tMean:       sum / float64(len(pts)),")
            lines.append("\t}")
            lines.append("}")
        lines.append("")

    content = "\n".join(lines)
    path.write_text(content, encoding="utf-8")
    print(f"Generated {path}: {len(lines)} lines")

def generate_rust_large_file():
    path = FIXTURES_DIR / "rust" / "large_file.rs"
    lines = []
    lines.append('//! Monolithic Rust module for AST parser benchmarking and slicing tests.')
    lines.append('//! Contains >2,000 LOC, 100+ functions and structs modeling an analytics engine.')
    lines.append('')
    lines.append('use std::collections::HashMap;')
    lines.append('use std::fmt::Debug;')
    lines.append('')
    lines.append('// ==========================================')
    lines.append('// 1. DOMAIN STRUCTS & ENUMS (30 Types)')
    lines.append('// ==========================================')
    lines.append('')

    structs = [
        ("Vector2D", [("pub x", "f64"), ("pub y", "f64")]),
        ("Vector3D", [("pub x", "f64"), ("pub y", "f64"), ("pub z", "f64")]),
        ("Quaternion", [("pub w", "f64"), ("pub x", "f64"), ("pub y", "f64"), ("pub z", "f64")]),
        ("BoundingBox3D", [("pub min", "Vector3D"), ("pub max", "Vector3D")]),
        ("Ray3D", [("pub origin", "Vector3D"), ("pub direction", "Vector3D")]),
        ("Matrix4x4", [("pub data", "[[f64; 4]; 4]")]),
        ("MarketTick", [("pub symbol", "String"), ("pub price", "f64"), ("pub volume", "f64"), ("pub timestamp", "i64")]),
        ("OHLCVCandle", [("pub timestamp", "i64"), ("pub open", "f64"), ("pub high", "f64"), ("pub low", "f64"), ("pub close", "f64"), ("pub volume", "f64")]),
        ("OrderBookLevel", [("pub price", "f64"), ("pub size", "f64"), ("pub order_count", "u32")]),
        ("DepthSnapshot", [("pub bids", "Vec<OrderBookLevel>"), ("pub asks", "Vec<OrderBookLevel>"), ("pub timestamp", "i64")]),
        ("PositionReport", [("pub symbol", "String"), ("pub qty", "f64"), ("pub entry_price", "f64"), ("pub current_price", "f64"), ("pub unrealized_pnl", "f64")]),
        ("RiskReport", [("pub var_95", "f64"), ("pub var_99", "f64"), ("pub expected_shortfall", "f64"), ("pub sharpe", "f64"), ("pub sortino", "f64")]),
        ("ASTSpan", [("pub start_line", "usize"), ("pub start_col", "usize"), ("pub end_line", "usize"), ("pub end_col", "usize")]),
        ("TelemetryEvent", [("pub name", "String"), ("pub value", "f64"), ("pub tags", "HashMap<String, String>"), ("pub timestamp", "i64")]),
        ("NetworkPacket", [("pub seq", "u64"), ("pub flags", "u32"), ("pub payload", "Vec<u8>"), ("pub checksum", "u32")]),
        ("TokenBucket", [("pub rate", "f64"), ("pub capacity", "f64"), ("pub tokens", "f64"), ("pub last_refill", "i64")]),
        ("SessionMetadata", [("pub session_id", "String"), ("pub user_id", "String"), ("pub expires_at", "i64"), ("pub is_active", "bool")]),
        ("AuditRecord", [("pub actor", "String"), ("pub action", "String"), ("pub target_id", "String"), ("pub timestamp", "i64")]),
        ("ClusterState", [("pub cluster_id", "String"), ("pub leader_node", "String"), ("pub active_nodes", "Vec<String>"), ("pub term", "u64")]),
        ("ReconciliationSummary", [("pub cluster_id", "String"), ("pub converged", "bool"), ("pub nodes_updated", "usize"), ("pub duration_ms", "u64"), ("pub error_count", "usize")]),
        ("KVEntry", [("pub key", "String"), ("pub value", "Vec<u8>"), ("pub version", "u64")]),
        ("WorkerTask", [("pub task_id", "String"), ("pub priority", "u8"), ("pub payload", "Vec<u8>")]),
        ("PipelineMetrics", [("pub processed_count", "u64"), ("pub total_latency_ns", "u64"), ("pub error_count", "u64")]),
        ("CacheItem", [("pub key", "String"), ("pub data", "Vec<u8>"), ("pub ttl_secs", "u32")]),
        ("BloomFilterConfig", [("pub size", "usize"), ("pub num_hashes", "u8")]),
    ]

    for name, fields in structs:
        lines.append("#[derive(Debug, Clone, PartialEq)]")
        lines.append(f"pub struct {name} {{")
        for fname, ftype in fields:
            lines.append(f"    {fname}: {ftype},")
        lines.append("}")
        lines.append("")

    lines.append('// ==========================================')
    lines.append('// 2. TARGET FUNCTION & DOMAIN METHODS (130 Functions)')
    lines.append('// ==========================================')
    lines.append('')

    # Target function reconcile_state
    lines.append('/// Reconciles state across cluster nodes and applies consensus transitions.')
    lines.append('pub fn reconcile_state(')
    lines.append('    current_state: &ClusterState,')
    lines.append('    inbound_nodes: &[String],')
    lines.append('    max_term: u64,')
    lines.append(') -> Result<ReconciliationSummary, String> {')
    lines.append('    if inbound_nodes.is_empty() {')
    lines.append('        return Err("Cannot reconcile with empty node list".to_string());')
    lines.append('    }')
    lines.append('')
    lines.append('    let mut updated_nodes = 0;')
    lines.append('    for node in inbound_nodes {')
    lines.append('        if !current_state.active_nodes.contains(node) {')
    lines.append('            updated_nodes += 1;')
    lines.append('        }')
    lines.append('    }')
    lines.append('')
    lines.append('    let converged = current_state.term >= max_term;')
    lines.append('    Ok(ReconciliationSummary {')
    lines.append('        cluster_id: current_state.cluster_id.clone(),')
    lines.append('        converged,')
    lines.append('        nodes_updated: updated_nodes,')
    lines.append('        duration_ms: 42,')
    lines.append('        error_count: 0,')
    lines.append('    })')
    lines.append('}')
    lines.append('')

    # Generate 160 additional functions
    for i in range(1, 161):
        fn_name = f"compute_rust_engine_fn_{i:03d}"
        mod = i % 5
        if mod == 0:
            lines.append(f"/// Computes simple moving average for candles.")
            lines.append(f"pub fn {fn_name}(candles: &[OHLCVCandle], period: usize) -> Vec<f64> {{")
            lines.append("    if candles.len() < period || period == 0 {")
            lines.append("        return Vec::new();")
            lines.append("    }")
            lines.append("    let mut result = Vec::with_capacity(candles.len() - period + 1);")
            lines.append("    let mut sum: f64 = candles[..period].iter().map(|c| c.close).sum();")
            lines.append("    result.push(sum / period as f64);")
            lines.append("    for i in period..candles.len() {")
            lines.append("        sum += candles[i].close - candles[i - period].close;")
            lines.append("        result.push(sum / period as f64);")
            lines.append("    }")
            lines.append("    result")
            lines.append("}")
        elif mod == 1:
            lines.append(f"/// Computes 3D vector interpolation.")
            lines.append(f"pub fn {fn_name}(v1: &Vector3D, v2: &Vector3D, t: f64) -> Vector3D {{")
            lines.append("    let t_clamped = t.clamp(0.0, 1.0);")
            lines.append("    let inv = 1.0 - t_clamped;")
            lines.append("    Vector3D {")
            lines.append("        x: v1.x * inv + v2.x * t_clamped,")
            lines.append("        y: v1.y * inv + v2.y * t_clamped,")
            lines.append("        z: v1.z * inv + v2.z * t_clamped,")
            lines.append("    }")
            lines.append("}")
        elif mod == 2:
            lines.append(f"/// Computes orderbook liquidity metrics.")
            lines.append(f"pub fn {fn_name}(depth: &DepthSnapshot) -> (f64, f64, f64) {{")
            lines.append("    let bid_vol: f64 = depth.bids.iter().map(|b| b.price * b.size).sum();")
            lines.append("    let ask_vol: f64 = depth.asks.iter().map(|a| a.price * a.size).sum();")
            lines.append("    let spread = if !depth.bids.is_empty() && !depth.asks.is_empty() {")
            lines.append("        (depth.asks[0].price - depth.bids[0].price).max(0.0)")
            lines.append("    } else {")
            lines.append("        0.0")
            lines.append("    };")
            lines.append("    (bid_vol, ask_vol, spread)")
            lines.append("}")
        elif mod == 3:
            lines.append(f"/// Computes risk indicators for positions.")
            lines.append(f"pub fn {fn_name}(positions: &[PositionReport]) -> RiskReport {{")
            lines.append("    let total_exposure: f64 = positions.iter().map(|p| p.qty * p.entry_price).sum();")
            lines.append("    let total_pnl: f64 = positions.iter().map(|p| p.unrealized_pnl).sum();")
            lines.append("    let var95 = total_exposure * 0.02 * 1.645;")
            lines.append("    let sharpe = if var95 > 0.0 { total_pnl / var95 } else { 0.0 };")
            lines.append("    RiskReport {")
            lines.append("        var_95: var95,")
            lines.append("        var_99: var95 * 1.41,")
            lines.append("        expected_shortfall: var95 * 1.25,")
            lines.append("        sharpe,")
            lines.append("        sortino: sharpe * 1.1,")
            lines.append("    }")
            lines.append("}")
        else:
            lines.append(f"/// Transforms a 3D vector by 4x4 matrix.")
            lines.append(f"pub fn {fn_name}(m: &Matrix4x4, v: &Vector3D) -> Vector3D {{")
            lines.append("    let d = &m.data;")
            lines.append("    let nx = d[0][0] * v.x + d[0][1] * v.y + d[0][2] * v.z + d[0][3];")
            lines.append("    let ny = d[1][0] * v.x + d[1][1] * v.y + d[1][2] * v.z + d[1][3];")
            lines.append("    let nz = d[2][0] * v.x + d[2][1] * v.y + d[2][2] * v.z + d[2][3];")
            lines.append("    let nw = d[3][0] * v.x + d[3][1] * v.y + d[3][2] * v.z + d[3][3];")
            lines.append("    let w = if nw.abs() > 1e-6 { nw } else { 1.0 };")
            lines.append("    Vector3D { x: nx / w, y: ny / w, z: nz / w }")
            lines.append("}")
        lines.append("")

    content = "\n".join(lines)
    path.write_text(content, encoding="utf-8")
    print(f"Generated {path}: {len(lines)} lines")

if __name__ == "__main__":
    generate_typescript_large_file()
    generate_python_large_file()
    generate_go_large_file()
    generate_rust_large_file()


