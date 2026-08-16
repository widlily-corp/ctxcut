"""
Asynchronous HTTP clients calling external banking and card processor APIs.
"""

from __future__ import annotations

import hashlib
import hmac
import json
import time
from typing import Any, Optional
import httpx

from .schemas import Currency, TransactionStatus


class BankingGatewayClient:
    """
    Client for interacting with clearinghouse banking networks and ACH processors.
    """

    def __init__(self, base_url: str, api_key: str, api_secret: str, timeout_seconds: float = 15.0):
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.api_secret = api_secret
        self.timeout = timeout_seconds

    def _generate_signature(self, payload: str, timestamp: str) -> str:
        message = f"{timestamp}.{payload}".encode("utf-8")
        return hmac.new(self.api_secret.encode("utf-8"), message, hashlib.sha256).hexdigest()

    async def authorize_charge(
        self,
        account_id: str,
        amount_cents: int,
        currency: Currency,
        payment_token: str,
        idempotency_key: str
    ) -> dict[str, Any]:
        """Send charge authorization request to upstream banking gateway."""
        endpoint = f"{self.base_url}/v1/charges/authorize"
        timestamp = str(int(time.time()))
        payload_data = {
            "account_id": account_id,
            "amount": amount_cents,
            "currency": currency.value,
            "token": payment_token,
            "idempotency_key": idempotency_key,
        }
        raw_payload = json.dumps(payload_data)
        signature = self._generate_signature(raw_payload, timestamp)

        headers = {
            "X-API-Key": self.api_key,
            "X-Signature": signature,
            "X-Timestamp": timestamp,
            "Content-Type": "application/json",
        }

        async with httpx.AsyncClient(timeout=self.timeout) as client:
            try:
                response = await client.post(endpoint, content=raw_payload, headers=headers)
                response.raise_for_status()
                return response.json()
            except httpx.HTTPStatusError as exc:
                return {
                    "success": False,
                    "status": "failed",
                    "error": f"Gateway error: {exc.response.status_code}",
                    "raw_body": exc.response.text,
                }
            except httpx.RequestError as exc:
                return {
                    "success": False,
                    "status": "failed",
                    "error": f"Transport failure: {str(exc)}",
                }

    async def execute_refund(
        self,
        charge_id: str,
        amount_cents: int,
        reason: str
    ) -> dict[str, Any]:
        """Execute a refund against a previously settled charge."""
        endpoint = f"{self.base_url}/v1/charges/{charge_id}/refund"
        payload_data = {
            "amount": amount_cents,
            "reason": reason,
        }

        async with httpx.AsyncClient(timeout=self.timeout) as client:
            response = await client.post(
                endpoint,
                json=payload_data,
                headers={"X-API-Key": self.api_key, "Content-Type": "application/json"}
            )
            response.raise_for_status()
            return response.json()

    async def verify_webhook_signature(self, body_bytes: bytes, signature_header: str) -> bool:
        """Verify the integrity and authenticity of inbound webhooks."""
        expected = hmac.new(self.api_secret.encode("utf-8"), body_bytes, hashlib.sha256).hexdigest()
        return hmac.compare_digest(expected, signature_header)


class FraudDetectionClient:
    """Client for querying real-time transaction risk scores."""

    def __init__(self, endpoint_url: str):
        self.endpoint_url = endpoint_url

    async def evaluate_risk(
        self,
        customer_id: str,
        amount_cents: int,
        ip_address: Optional[str] = None
    ) -> float:
        """Query fraud detection ML engine for risk score (0.0 - 1.0)."""
        async with httpx.AsyncClient(timeout=5.0) as client:
            try:
                resp = await client.post(
                    f"{self.endpoint_url}/evaluate",
                    json={"customer_id": customer_id, "amount": amount_cents, "ip": ip_address}
                )
                if resp.status_code == 200:
                    return float(resp.json().get("risk_score", 0.05))
            except Exception:
                pass
        return 0.10
