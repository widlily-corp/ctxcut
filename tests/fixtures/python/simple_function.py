"""
Standalone typed functions for Python with modern 3.10+ union syntax.
"""

from typing import Sequence


def add_numbers(a: int | float, b: int | float) -> int | float:
    """Add two numbers supporting ints and floats."""
    return a + b


def format_user_name(first_name: str, last_name: str, prefix: str | None = None) -> str:
    """Format a user's full name with an optional honorific prefix."""
    full = f"{first_name.strip()} {last_name.strip()}".strip()
    if prefix and prefix.strip():
        return f"{prefix.strip()} {full}"
    return full


def parse_identifier(raw: str | int | bytes) -> str:
    """Normalize various identifier input formats into a canonical string."""
    if isinstance(raw, bytes):
        return raw.decode("utf-8", errors="replace").strip()
    if isinstance(raw, int):
        return f"id_{raw:08d}"
    return str(raw).strip()


def calculate_discount(price: float, percentage: float) -> float:
    """Calculate discounted price with input validation."""
    if price < 0.0:
        raise ValueError("Price cannot be negative")
    if not (0.0 <= percentage <= 100.0):
        raise ValueError("Percentage must be between 0 and 100")
    discount = (price * percentage) / 100.0
    return round(price - discount, 2)


def find_first_matching(items: Sequence[str | int], query: str | int) -> int | None:
    """Find the index of the first matching item in a sequence."""
    for idx, item in enumerate(items):
        if item == query:
            return idx
    return None
