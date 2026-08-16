"""
Pydantic schemas and data models for the realistic payment service.
"""

from __future__ import annotations

from datetime import datetime
from enum import Enum
from typing import Any, Optional
from pydantic import BaseModel, Field, field_validator


class Currency(str, Enum):
    USD = "USD"
    EUR = "EUR"
    GBP = "GBP"
    CAD = "CAD"
    AUD = "AUD"
    JPY = "JPY"


class PaymentMethodType(str, Enum):
    CREDIT_CARD = "credit_card"
    SEPA_DEBIT = "sepa_debit"
    WIRE_TRANSFER = "wire_transfer"
    CRYPTO = "crypto"


class TransactionStatus(str, Enum):
    PENDING = "pending"
    PROCESSING = "processing"
    SETTLED = "settled"
    FAILED = "failed"
    REFUNDED = "refunded"
    DISPUTED = "disputed"


class BillingAddress(BaseModel):
    line1: str = Field(..., min_length=1)
    line2: Optional[str] = None
    city: str = Field(..., min_length=1)
    state: str = Field(..., min_length=2)
    postal_code: str = Field(..., min_length=3)
    country: str = Field(..., min_length=2, max_length=2)


class CustomerBillingProfile(BaseModel):
    customer_id: str
    email: str
    full_name: str
    default_currency: Currency = Currency.USD
    default_payment_method: PaymentMethodType = PaymentMethodType.CREDIT_CARD
    billing_address: BillingAddress
    is_active: bool = True
    kyc_verified: bool = False


class ChargeRequest(BaseModel):
    account_id: str
    customer_id: str
    amount_cents: int = Field(..., gt=0)
    currency: Currency = Currency.USD
    payment_method_id: str
    idempotency_key: str = Field(..., min_length=16)
    description: Optional[str] = None
    metadata: dict[str, str] = Field(default_factory=dict)

    @field_validator("amount_cents")
    @classmethod
    def validate_max_amount(cls, v: int) -> int:
        if v > 100_000_000:
            raise ValueError("Amount exceeds maximum single-charge ceiling of $1,000,000")
        return v


class ChargeResult(BaseModel):
    transaction_id: str
    charge_id: str
    status: TransactionStatus
    amount_cents: int
    fee_cents: int
    currency: Currency
    settled_at: Optional[datetime] = None
    auth_code: str
    error_message: Optional[str] = None


class RefundRequest(BaseModel):
    charge_id: str
    amount_cents: Optional[int] = Field(None, gt=0)
    reason: str = Field(..., min_length=3)
    initiated_by: str


class RefundResponse(BaseModel):
    refund_id: str
    charge_id: str
    status: TransactionStatus
    amount_refunded_cents: int
    currency: Currency
    remaining_balance_cents: int
    created_at: datetime = Field(default_factory=datetime.utcnow)


class WebhookEventPayload(BaseModel):
    event_id: str
    event_type: str
    timestamp: datetime
    data: dict[str, Any]
    signature: str
