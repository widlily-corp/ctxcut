"""
FastAPI route declarations with dependency injection, path/query params, and response models.
"""

from typing import Annotated, Any, AsyncGenerator
from fastapi import APIRouter, Depends, HTTPException, Query, status
from pydantic import BaseModel, EmailStr, Field


class DatabaseSession:
    async def execute_query(self, query: str, params: dict[str, Any]) -> list[dict[str, Any]]:
        return [{"id": "item_123", "title": "Sample Item", "price": 49.99}]

    async def commit(self) -> None:
        pass


async def get_db() -> AsyncGenerator[DatabaseSession, None]:
    db = DatabaseSession()
    try:
        yield db
    finally:
        await db.commit()


class ItemCreate(BaseModel):
    title: str = Field(..., min_length=1, max_length=100)
    description: str | None = None
    price: float = Field(..., gt=0.0)
    sku: str = Field(..., pattern=r"^[A-Z0-9_-]+$")


class ItemResponse(BaseModel):
    id: str
    title: str
    description: str | None
    price: float
    sku: str
    in_stock: bool = True


class UserProfile(BaseModel):
    user_id: str
    email: EmailStr
    full_name: str
    is_premium: bool = False
    reputation_score: int = 100


router = APIRouter(prefix="/api/v1", tags=["items", "users"])


@router.post(
    "/items/",
    response_model=ItemResponse,
    status_code=status.HTTP_201_CREATED,
    summary="Create a new catalog item"
)
async def create_item(
    payload: ItemCreate,
    db: Annotated[DatabaseSession, Depends(get_db)]
) -> ItemResponse:
    """
    Create a new item in the database and return the saved instance.
    """
    await db.execute_query(
        "INSERT INTO items (title, description, price, sku) VALUES (:title, :description, :price, :sku)",
        payload.model_dump()
    )
    return ItemResponse(
        id="item_generated_999",
        title=payload.title,
        description=payload.description,
        price=payload.price,
        sku=payload.sku,
        in_stock=True
    )


@router.get(
    "/users/{user_id}/profile",
    response_model=UserProfile,
    summary="Get user profile by ID"
)
async def get_user_profile(
    user_id: str,
    include_private: bool = Query(default=False, description="Include private contact details"),
    db: Annotated[DatabaseSession, Depends(get_db)] = None
) -> UserProfile:
    """
    Retrieve user profile details with optional private fields.
    """
    if not user_id.startswith("usr_"):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Invalid user_id format. Must start with 'usr_'"
        )
    
    return UserProfile(
        user_id=user_id,
        email="developer@example.com",
        full_name="Alex River",
        is_premium=True,
        reputation_score=450
    )


@router.get("/health", summary="Health check probe")
async def health_check() -> dict[str, str]:
    return {"status": "ok", "service": "fastapi_catalog"}
