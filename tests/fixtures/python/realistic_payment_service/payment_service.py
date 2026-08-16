"""
Core PaymentProcessor microservice implementation.
"""

from __future__ import annotations

import logging
from datetime import datetime
from typing import Any, Optional

from .clients import BankingGatewayClient, FraudDetectionClient
from .schemas import (
    ChargeRequest,
    ChargeResult,
    RefundRequest,
    RefundResponse,
    TransactionStatus,
    WebhookEventPayload,
)

logger = logging.getLogger("payment_service")


class PaymentRepository:
    """Mock persistence layer for payment transactions."""

    def __init__(self) -> None:
        self._charges: dict[str, dict[str, Any]] = {}
        self._refunds: dict[str, dict[str, Any]] = {}

    async def save_charge(self, record: dict[str, Any]) -> None:
        self._charges[record["charge_id"]] = record

    async def get_charge(self, charge_id: str) -> Optional[dict[str, Any]]:
        return self._charges.get(charge_id)

    async def save_refund(self, record: dict[str, Any]) -> None:
        self._refunds[record["refund_id"]] = record


class PaymentProcessor:
    """
    Production-grade payment processing coordinator managing risk checks,
    gateway transactions, refunds, and webhook event reconciliation.
    """

    def __init__(
        self,
        gateway_client: BankingGatewayClient,
        fraud_client: FraudDetectionClient,
        repository: PaymentRepository
    ) -> None:
        self.gateway = gateway_client
        self.fraud = fraud_client
        self.repo = repository

    async def execute_charge(self, request: ChargeRequest) -> ChargeResult:
        """
        Execute payment authorization and capture against external gateway.
        """
        logger.info("Initiating charge for customer %s, amount %d %s", request.customer_id, request.amount_cents, request.currency)

        risk_score = await self.fraud.evaluate_risk(request.customer_id, request.amount_cents)
        if risk_score > 0.85:
            logger.warning("Charge rejected due to excessive fraud risk: %.2f", risk_score)
            return ChargeResult(
                transaction_id=f"tx_rej_{int(datetime.utcnow().timestamp())}",
                charge_id="none",
                status=TransactionStatus.FAILED,
                amount_cents=request.amount_cents,
                fee_cents=0,
                currency=request.currency,
                auth_code="FRAUD_REJECTED",
                error_message=f"Transaction exceeded risk threshold (score: {risk_score:.2f})",
            )

        gateway_resp = await self.gateway.authorize_charge(
            account_id=request.account_id,
            amount_cents=request.amount_cents,
            currency=request.currency,
            payment_token=request.payment_method_id,
            idempotency_key=request.idempotency_key,
        )

        if not gateway_resp.get("success", True) or gateway_resp.get("status") == "failed":
            error_msg = gateway_resp.get("error", "Charge authorization failed")
            logger.error("Gateway authorization failure: %s", error_msg)
            return ChargeResult(
                transaction_id=f"tx_failed_{int(datetime.utcnow().timestamp())}",
                charge_id=gateway_resp.get("id", "none"),
                status=TransactionStatus.FAILED,
                amount_cents=request.amount_cents,
                fee_cents=0,
                currency=request.currency,
                auth_code="DECLINED",
                error_message=error_msg,
            )

        charge_id = gateway_resp.get("id", f"ch_{int(datetime.utcnow().timestamp())}")
        fee_cents = int(request.amount_cents * 0.029) + 30
        now = datetime.utcnow()

        record = {
            "charge_id": charge_id,
            "account_id": request.account_id,
            "customer_id": request.customer_id,
            "amount_cents": request.amount_cents,
            "fee_cents": fee_cents,
            "currency": request.currency.value,
            "status": TransactionStatus.SETTLED.value,
            "created_at": now.isoformat(),
            "refunded_cents": 0,
        }
        await self.repo.save_charge(record)

        return ChargeResult(
            transaction_id=f"tx_{charge_id}",
            charge_id=charge_id,
            status=TransactionStatus.SETTLED,
            amount_cents=request.amount_cents,
            fee_cents=fee_cents,
            currency=request.currency,
            settled_at=now,
            auth_code=gateway_resp.get("auth_code", "AUTH_APPROVED_OK"),
        )

    async def issue_refund(self, request: RefundRequest) -> RefundResponse:
        """
        Process partial or full refund for a settled charge.
        """
        charge_record = await self.repo.get_charge(request.charge_id)
        if not charge_record:
            raise ValueError(f"Charge ID '{request.charge_id}' not found")

        current_refunded = charge_record.get("refunded_cents", 0)
        total_amount = charge_record["amount_cents"]
        refundable_remaining = total_amount - current_refunded

        refund_amount = request.amount_cents if request.amount_cents is not None else refundable_remaining
        if refund_amount <= 0 or refund_amount > refundable_remaining:
            raise ValueError(f"Invalid refund amount: {refund_amount}. Available: {refundable_remaining}")

        gateway_resp = await self.gateway.execute_refund(
            charge_id=request.charge_id,
            amount_cents=refund_amount,
            reason=request.reason,
        )

        refund_id = gateway_resp.get("refund_id", f"ref_{int(datetime.utcnow().timestamp())}")
        charge_record["refunded_cents"] = current_refunded + refund_amount
        if charge_record["refunded_cents"] >= total_amount:
            charge_record["status"] = TransactionStatus.REFUNDED.value
        await self.repo.save_charge(charge_record)

        refund_record = {
            "refund_id": refund_id,
            "charge_id": request.charge_id,
            "amount_cents": refund_amount,
            "reason": request.reason,
            "initiated_by": request.initiated_by,
            "created_at": datetime.utcnow().isoformat(),
        }
        await self.repo.save_refund(refund_record)

        return RefundResponse(
            refund_id=refund_id,
            charge_id=request.charge_id,
            status=TransactionStatus.REFUNDED if charge_record["refunded_cents"] >= total_amount else TransactionStatus.SETTLED,
            amount_refunded_cents=refund_amount,
            currency=charge_record["currency"],
            remaining_balance_cents=total_amount - charge_record["refunded_cents"],
        )

    async def handle_webhook(self, payload: WebhookEventPayload, raw_body: bytes) -> dict[str, str]:
        """
        Validate and process asynchronous banking network webhook events.
        """
        is_valid = await self.gateway.verify_webhook_signature(raw_body, payload.signature)
        if not is_valid:
            logger.error("Invalid webhook signature for event %s", payload.event_id)
            return {"status": "error", "message": "Invalid signature"}

        logger.info("Processing webhook event: %s (%s)", payload.event_id, payload.event_type)
        if payload.event_type == "charge.dispute.created":
            charge_id = payload.data.get("charge_id")
            if charge_id:
                charge = await self.repo.get_charge(charge_id)
                if charge:
                    charge["status"] = TransactionStatus.DISPUTED.value
                    await self.repo.save_charge(charge)

        return {"status": "processed", "event_id": payload.event_id}
