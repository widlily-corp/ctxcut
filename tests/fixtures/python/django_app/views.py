"""
Django REST Framework views and viewsets.
"""

from rest_framework import viewsets, status
from rest_framework.views import APIView
from rest_framework.response import Response
from rest_framework.decorators import action
from rest_framework.permissions import IsAuthenticated

from .models import Article, Order, UserProfile, Comment
from .serializers import (
    ArticleSerializer,
    OrderSerializer,
    OrderCreateSerializer,
    UserProfileSerializer,
    CommentSerializer,
)
from .permissions import IsAuthorOrReadOnly, IsAdminOrOwner, IsVerifiedUser


class ArticleViewSet(viewsets.ModelViewSet):
    """
    ViewSet for managing articles, comments, and publishing actions.
    """
    queryset = Article.objects.all().select_related("author").prefetch_related("tags", "comments")
    serializer_class = ArticleSerializer
    permission_classes = [IsAuthorOrReadOnly]

    def perform_create(self, serializer: ArticleSerializer) -> None:
        """Associates the authenticated user's profile as the author."""
        serializer.save(author=self.request.user.profile)

    @action(detail=True, methods=["post"], permission_classes=[IsAuthorOrReadOnly])
    def publish(self, request, pk=None) -> Response:
        """Custom endpoint to publish an article."""
        article = self.get_object()
        article.publish()
        return Response(
            {"status": "article published", "slug": article.slug},
            status=status.HTTP_200_OK,
        )

    @action(detail=True, methods=["post"], serializer_class=CommentSerializer, permission_classes=[IsAuthenticated])
    def add_comment(self, request, pk=None) -> Response:
        """Adds a comment to the specified article."""
        article = self.get_object()
        serializer = CommentSerializer(data=request.data)
        if serializer.is_valid():
            serializer.save(article=article, author=request.user.profile)
            return Response(serializer.data, status=status.HTTP_201_CREATED)
        return Response(serializer.errors, status=status.HTTP_400_BAD_REQUEST)


class OrderViewSet(viewsets.ModelViewSet):
    """
    ViewSet for managing customer orders and status transitions.
    """
    queryset = Order.objects.all().select_related("customer")
    serializer_class = OrderSerializer
    permission_classes = [IsAdminOrOwner]

    def get_queryset(self):
        """Filters orders by customer unless user is staff."""
        if self.request.user.is_staff:
            return Order.objects.all()
        return Order.objects.filter(customer=self.request.user.profile)

    @action(detail=True, methods=["post"], permission_classes=[IsAdminOrOwner])
    def complete(self, request, pk=None) -> Response:
        """Marks an order as completed."""
        order = self.get_object()
        order.mark_completed()
        return Response({"status": "order completed", "id": order.id})


class UserProfileView(APIView):
    """
    APIView for retrieving and updating current user's profile.
    """
    permission_classes = [IsAuthenticated, IsVerifiedUser]

    def get(self, request) -> Response:
        """Retrieves authenticated user profile."""
        profile = request.user.profile
        serializer = UserProfileSerializer(profile)
        return Response(serializer.data)

    def put(self, request) -> Response:
        """Updates authenticated user profile."""
        profile = request.user.profile
        serializer = UserProfileSerializer(profile, data=request.data, partial=True)
        if serializer.is_valid():
            serializer.save()
            return Response(serializer.data)
        return Response(serializer.errors, status=status.HTTP_400_BAD_REQUEST)
