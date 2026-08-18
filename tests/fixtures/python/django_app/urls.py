"""
URL configuration for the Django application.
"""

from django.urls import path, include
from rest_framework.routers import DefaultRouter
from .views import ArticleViewSet, OrderViewSet, UserProfileView

router = DefaultRouter()
router.register(r"articles", ArticleViewSet, basename="article")
router.register(r"orders", OrderViewSet, basename="order")

urlpatterns = [
    path("api/v1/", include(router.urls)),
    path("api/v1/profile/", UserProfileView.as_view(), name="user-profile"),
]
