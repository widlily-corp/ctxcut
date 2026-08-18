"""
Django REST Framework serializers for models.
"""

from decimal import Decimal
from rest_framework import serializers
from .models import UserProfile, Article, Comment, Order, Tag


class TagSerializer(serializers.ModelSerializer):
    """Serializer for tag taxonomy."""

    class Meta:
        model = Tag
        fields = ["id", "name", "created_at"]


class UserProfileSerializer(serializers.ModelSerializer):
    """Serializer for user profile entity."""

    username = serializers.CharField(source="user.username", read_only=True)
    email = serializers.EmailField(source="user.email", read_only=True)

    class Meta:
        model = UserProfile
        fields = ["id", "username", "email", "bio", "website", "created_at"]

    def validate_bio(self, value: str) -> str:
        """Validates that the bio does not exceed maximum length."""
        if len(value) > 1000:
            raise serializers.ValidationError("Bio must not exceed 1000 characters.")
        return value.strip()


class CommentSerializer(serializers.ModelSerializer):
    """Serializer for article comments."""

    author_name = serializers.CharField(
        source="author.get_full_display_name",
        read_only=True,
    )

    class Meta:
        model = Comment
        fields = ["id", "article", "author", "author_name", "text", "is_approved", "created_at"]
        read_only_fields = ["id", "is_approved", "created_at"]

    def validate_text(self, value: str) -> str:
        """Ensures comment body is non-empty and stripped."""
        cleaned = value.strip()
        if not cleaned:
            raise serializers.ValidationError("Comment text cannot be empty.")
        return cleaned


class ArticleSerializer(serializers.ModelSerializer):
    """Serializer for blog articles including comments and tags."""

    author = UserProfileSerializer(read_only=True)
    tags = TagSerializer(many=True, read_only=True)
    comments = CommentSerializer(many=True, read_only=True)
    reading_time = serializers.IntegerField(source="reading_time_minutes", read_only=True)

    class Meta:
        model = Article
        fields = [
            "id",
            "title",
            "slug",
            "content",
            "author",
            "tags",
            "comments",
            "published",
            "views_count",
            "reading_time",
            "created_at",
            "updated_at",
        ]
        read_only_fields = ["id", "views_count", "created_at", "updated_at"]

    def validate_title(self, value: str) -> str:
        """Validates that the article title is descriptive."""
        if len(value.strip()) < 5:
            raise serializers.ValidationError("Title must be at least 5 characters long.")
        return value.strip()

    def validate_slug(self, value: str) -> str:
        """Ensures slug contains valid characters."""
        if " " in value:
            raise serializers.ValidationError("Slug must not contain spaces.")
        return value.lower()


class OrderSerializer(serializers.ModelSerializer):
    """Serializer for customer orders."""

    customer_name = serializers.CharField(
        source="customer.get_full_display_name",
        read_only=True,
    )

    class Meta:
        model = Order
        fields = [
            "id",
            "customer",
            "customer_name",
            "total_amount",
            "status",
            "shipping_address",
            "created_at",
            "updated_at",
        ]
        read_only_fields = ["id", "status", "created_at", "updated_at"]

    def validate_total_amount(self, value: Decimal) -> Decimal:
        """Validates order total is positive."""
        if value <= Decimal("0.00"):
            raise serializers.ValidationError("Order total amount must be strictly positive.")
        return value


class OrderCreateSerializer(serializers.Serializer):
    """Non-model serializer for checkout request payload."""

    customer_id = serializers.IntegerField()
    items = serializers.ListField(
        child=serializers.DictField(),
        min_length=1,
    )
    shipping_address = serializers.CharField(max_length=500)
    payment_token = serializers.CharField(max_length=256)

    def validate_shipping_address(self, value: str) -> str:
        """Validates shipping address length."""
        if len(value.strip()) < 10:
            raise serializers.ValidationError("Please provide a complete shipping address.")
        return value.strip()

    def validate(self, attrs: dict) -> dict:
        """Validates the overall checkout payload."""
        if not attrs.get("items"):
            raise serializers.ValidationError({"items": "At least one item is required."})
        return attrs
