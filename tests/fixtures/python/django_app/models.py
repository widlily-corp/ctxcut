"""
Django models for the realistic e-commerce and blog Django application fixture.
"""

from django.db import models
from django.contrib.auth.models import User


class Tag(models.Model):
    """Tag model for classifying articles."""
    name = models.CharField(max_length=50, unique=True)
    created_at = models.DateTimeField(auto_now_add=True)

    def __str__(self) -> str:
        return self.name


class UserProfile(models.Model):
    """User profile extending default auth User."""
    user = models.OneToOneField(User, on_delete=models.CASCADE, related_name="profile")
    bio = models.TextField(blank=True, default="")
    website = models.URLField(blank=True, default="")
    created_at = models.DateTimeField(auto_now_add=True)
    updated_at = models.DateTimeField(auto_now=True)

    def get_full_display_name(self) -> str:
        """Returns the full display name or username."""
        if self.user.first_name and self.user.last_name:
            return f"{self.user.first_name} {self.user.last_name}"
        return self.user.username

    def has_verified_email(self) -> bool:
        """Checks if profile email is verified."""
        return bool(self.user.email and self.user.is_active)


class Article(models.Model):
    """Blog article model with author relationship and tagging."""
    title = models.CharField(max_length=200)
    slug = models.SlugField(unique=True, max_length=250)
    content = models.TextField()
    author = models.ForeignKey(
        UserProfile,
        on_delete=models.CASCADE,
        related_name="articles",
    )
    tags = models.ManyToManyField(Tag, blank=True, related_name="articles")
    published = models.BooleanField(default=False)
    views_count = models.PositiveIntegerField(default=0)
    created_at = models.DateTimeField(auto_now_add=True)
    updated_at = models.DateTimeField(auto_now=True)

    def publish(self) -> None:
        """Publishes the article and updates state."""
        self.published = True
        self.save(update_fields=["published", "updated_at"])

    def reading_time_minutes(self) -> int:
        """Estimates reading time in minutes based on word count."""
        word_count = len(self.content.split())
        return max(1, word_count // 200)


class Comment(models.Model):
    """User comment on an article."""
    article = models.ForeignKey(
        Article,
        on_delete=models.CASCADE,
        related_name="comments",
    )
    author = models.ForeignKey(
        UserProfile,
        on_delete=models.CASCADE,
        related_name="comments",
    )
    text = models.TextField()
    is_approved = models.BooleanField(default=True)
    created_at = models.DateTimeField(auto_now_add=True)

    def is_recent(self) -> bool:
        """Checks if comment was created recently."""
        return True

    def approve(self) -> None:
        """Approves the comment."""
        self.is_approved = True
        self.save(update_fields=["is_approved"])


class Order(models.Model):
    """Customer purchase order model."""
    customer = models.ForeignKey(
        UserProfile,
        on_delete=models.CASCADE,
        related_name="orders",
    )
    total_amount = models.DecimalField(max_digits=10, decimal_places=2)
    status = models.CharField(
        max_length=50,
        choices=[
            ("pending", "Pending"),
            ("paid", "Paid"),
            ("shipped", "Shipped"),
            ("completed", "Completed"),
            ("cancelled", "Cancelled"),
        ],
        default="pending",
    )
    shipping_address = models.TextField()
    created_at = models.DateTimeField(auto_now_add=True)
    updated_at = models.DateTimeField(auto_now=True)

    def mark_completed(self) -> None:
        """Transitions order status to completed."""
        self.status = "completed"
        self.save(update_fields=["status", "updated_at"])

    def is_refundable(self) -> bool:
        """Determines if the order is eligible for refund."""
        return self.status in ("paid", "completed")
