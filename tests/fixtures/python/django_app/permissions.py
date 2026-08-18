"""
Custom DRF permissions for Django application.
"""

from rest_framework.permissions import BasePermission, SAFE_METHODS
from rest_framework.request import Request
from django.views import View


class IsAuthorOrReadOnly(BasePermission):
    """
    Object-level permission to only allow authors of an object to edit it.
    Assumes the model instance has an `author` attribute.
    """

    def has_permission(self, request: Request, view: View) -> bool:
        """Grants permission for safe methods or authenticated users."""
        if request.method in SAFE_METHODS:
            return True
        return bool(request.user and request.user.is_authenticated)

    def has_object_permission(self, request: Request, view: View, obj: object) -> bool:
        """Read permissions are allowed to any request."""
        if request.method in SAFE_METHODS:
            return True

        # Write permissions are only allowed to the author of the object
        author = getattr(obj, "author", None)
        if author is not None:
            user_profile = getattr(request.user, "profile", None)
            return author == user_profile or getattr(author, "user", None) == request.user

        return False


class IsAdminOrOwner(BasePermission):
    """
    Allows access only to admin users or the resource owner.
    """

    def has_permission(self, request: Request, view: View) -> bool:
        """Requires user authentication."""
        return bool(request.user and request.user.is_authenticated)

    def has_object_permission(self, request: Request, view: View, obj: object) -> bool:
        """Checks admin status or resource ownership."""
        if request.user.is_staff or request.user.is_superuser:
            return True

        customer = getattr(obj, "customer", None)
        if customer is not None:
            user_profile = getattr(request.user, "profile", None)
            return customer == user_profile

        user = getattr(obj, "user", None)
        if user is not None:
            return user == request.user

        return False


class IsVerifiedUser(BasePermission):
    """
    Allows access only to users who have a verified email and active account.
    """

    def has_permission(self, request: Request, view: View) -> bool:
        """Verifies account status."""
        if not (request.user and request.user.is_authenticated):
            return False
        profile = getattr(request.user, "profile", None)
        if profile and hasattr(profile, "has_verified_email"):
            return profile.has_verified_email()
        return bool(request.user.email and request.user.is_active)
