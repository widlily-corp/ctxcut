"""
Pydantic model schemas, generic models, field validators, and user registration functions.
"""

from __future__ import annotations

from datetime import datetime
from enum import Enum
from typing import Any, Generic, Sequence, TypeVar
from pydantic import BaseModel, EmailStr, Field, field_validator


class UserRole(str, Enum):
    ADMIN = "admin"
    EDITOR = "editor"
    VIEWER = "viewer"
    GUEST = "guest"


class AuditMetadata(BaseModel):
    created_at: datetime = Field(default_factory=datetime.utcnow)
    created_by: str = "system"
    tags: list[str] = Field(default_factory=list)
    version: int = 1


TData = TypeVar("TData")


class APIEnvelope(BaseModel, Generic[TData]):
    success: bool
    data: TData | None = None
    error_code: str | None = None
    message: str | None = None
    timestamp: datetime = Field(default_factory=datetime.utcnow)


class UserPreferences(BaseModel):
    theme: str = Field(default="system", pattern="^(light|dark|system)$")
    locale: str = "en_US"
    receive_newsletter: bool = False
    items_per_page: int = Field(default=25, ge=5, le=100)


class UserCreate(BaseModel):
    username: str = Field(..., min_length=3, max_length=50)
    email: EmailStr
    full_name: str = Field(..., min_length=1)
    role: UserRole = UserRole.VIEWER
    preferences: UserPreferences | None = None

    @field_validator("username")
    @classmethod
    def validate_username_alphanumeric(cls, v: str) -> str:
        if not v.isalnum() and "_" not in v:
            raise ValueError("Username must contain only alphanumeric characters and underscores")
        return v.lower()


class UserResponse(BaseModel):
    id: str
    username: str
    email: str
    full_name: str
    role: UserRole
    is_active: bool
    preferences: UserPreferences
    metadata: AuditMetadata


def register_user(
    payload: UserCreate,
    tenant_id: str,
    auto_activate: bool = True
) -> APIEnvelope[UserResponse]:
    """
    Register a new user in the system with validation and auditing metadata.
    """
    user_id = f"usr_{tenant_id}_{payload.username}"
    user_prefs = payload.preferences or UserPreferences()
    
    response_dto = UserResponse(
        id=user_id,
        username=payload.username,
        email=str(payload.email),
        full_name=payload.full_name,
        role=payload.role,
        is_active=auto_activate,
        preferences=user_prefs,
        metadata=AuditMetadata(
            created_by=f"tenant_{tenant_id}",
            tags=["new_registration", payload.role.value],
        )
    )
    
    return APIEnvelope[UserResponse](
        success=True,
        data=response_dto,
        message="User successfully registered",
    )


def batch_register_users(
    user_list: Sequence[UserCreate],
    tenant_id: str
) -> list[APIEnvelope[UserResponse]]:
    """Register multiple users sequentially."""
    results: list[APIEnvelope[UserResponse]] = []
    for user_req in user_list:
        results.append(register_user(user_req, tenant_id=tenant_id))
    return results
