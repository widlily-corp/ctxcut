package auth

import (
	"context"
	"testing"
	"time"
)

func TestAuthService_RegisterAndLogin(t *testing.T) {
	uRepo := NewInMemoryUserRepository()
	sRepo := NewInMemorySessionRepository()
	jwt := NewJWTHelper("test-secret-key-1234567890", 15*time.Minute, 24*time.Hour)

	service := NewAuthService(uRepo, sRepo, jwt)
	ctx := context.Background()

	// Register
	regReq := RegistrationRequest{
		Email:    "testuser@acme.corp",
		Username: "testuser",
		Password: "SecurePassword123!",
		Role:     RoleAdmin,
	}

	user, err := service.Register(ctx, regReq)
	if err != nil {
		t.Fatalf("Failed to register user: %v", err)
	}
	if user.Username != "testuser" {
		t.Errorf("Expected username 'testuser', got %s", user.Username)
	}

	// Login
	loginReq := LoginRequest{
		UsernameOrEmail: "testuser",
		Password:        "SecurePassword123!",
		UserAgent:       "Go-Test/1.0",
		ClientIP:        "127.0.0.1",
	}

	resp, err := service.Login(ctx, loginReq)
	if err != nil {
		t.Fatalf("Failed to login: %v", err)
	}
	if resp.AccessToken == "" {
		t.Error("Expected non-empty access token")
	}
	if resp.User.ID != user.ID {
		t.Errorf("Expected user ID %s, got %s", user.ID, resp.User.ID)
	}
}

func TestAuthService_LoginInvalidCredentials(t *testing.T) {
	uRepo := NewInMemoryUserRepository()
	sRepo := NewInMemorySessionRepository()
	jwt := NewJWTHelper("test-secret-key-1234567890", 15*time.Minute, 24*time.Hour)

	service := NewAuthService(uRepo, sRepo, jwt)
	ctx := context.Background()

	loginReq := LoginRequest{
		UsernameOrEmail: "nonexistent",
		Password:        "wrongpassword",
		UserAgent:       "Go-Test/1.0",
		ClientIP:        "127.0.0.1",
	}

	_, err := service.Login(ctx, loginReq)
	if err == nil {
		t.Fatal("Expected error for invalid login, got nil")
	}
}
