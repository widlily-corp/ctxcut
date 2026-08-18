"""
Unit and integration tests for PaymentProcessor service using pytest and unittest.mock.
"""

import pytest
from unittest.mock import AsyncMock, MagicMock
from decimal import Decimal

from .schemas import (
    ChargeRequest,
    ChargeResult,
    RefundRequest,
    RefundResponse,
    TransactionStatus,
    PaymentMethodType,
)
from .payment_service import PaymentProcessor, PaymentRepository
from .clients import BankingGatewayClient, FraudDetectionClient


@pytest.fixture
def mock_gateway():
    gateway = MagicMock(spec=BankingGatewayClient)
    gateway.process_transaction = AsyncMock(
        return_value={
            "gateway_ref": "gw_tx_9988",
            "status": "APPROVED",
            "fee_cents": 35,
        }
    )
    gateway.issue_refund = AsyncMock(
        return_value={
            "gateway_refund_ref": "gw_rf_1122",
            "status": "REFUNDED",
        }
    )
    return gateway


@pytest.fixture
def mock_fraud():
    fraud = MagicMock(spec=FraudDetectionClient)
    fraud.evaluate_risk = AsyncMock(
        return_value={
            "risk_score": 0.05,
            "decision": "ACCEPT",
            "flagged_signals": [],
        }
    )
    return fraud


@pytest.fixture
def mock_repo():
    return PaymentRepository()


@pytest.fixture
def payment_processor(mock_gateway, mock_fraud, mock_repo):
    return PaymentProcessor(
        gateway_client=mock_gateway,
        fraud_client=mock_fraud,
        repository=mock_repo,
    )


@pytest.mark.asyncio
async def test_execute_charge_success(payment_processor, mock_gateway, mock_fraud):
    request = ChargeRequest(
        customer_id="cust_999",
        amount_cents=10500,
        currency="USD",
        payment_method=PaymentMethodType.CREDIT_CARD,
        payment_token="tok_secure_123",
        idempotency_key="idemp_abc_001",
    )

    result = await payment_processor.execute_charge(request)

    assert result.status == TransactionStatus.SUCCESS
    assert result.amount_cents == 10500
    mock_fraud.evaluate_risk.assert_called_once()
    mock_gateway.process_transaction.assert_called_once()


@pytest.mark.asyncio
async def test_execute_charge_fraud_rejection(payment_processor, mock_fraud, mock_gateway):
    mock_fraud.evaluate_risk.return_value = {
        "risk_score": 0.95,
        "decision": "REJECT",
        "flagged_signals": ["SUSPICIOUS_IP", "VELOCITY_EXCEEDED"],
    }

    request = ChargeRequest(
        customer_id="cust_bad",
        amount_cents=50000,
        currency="USD",
        payment_method=PaymentMethodType.CREDIT_CARD,
        payment_token="tok_fraud",
        idempotency_key="idemp_bad_002",
    )

    result = await payment_processor.execute_charge(request)

    assert result.status == TransactionStatus.DECLINED_FRAUD
    mock_gateway.process_transaction.assert_not_called()
