package auth

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"time"
)

var (
	ErrInvalidCredentials = errors.New("invalid username or password")
	ErrInactiveAccount    = errors.New("user account is inactive")
	ErrInvalidToken       = errors.New("invalid or expired refresh token")
)

// AuthService coordinates user registration, authentication, session tokens, and revocation.
type AuthService struct {
	userRepo    UserRepository
	sessionRepo SessionRepository
	jwtHelper   *JWTHelper
}

// NewAuthService creates a new AuthService with the provided repositories and JWT helper.
func NewAuthService(uRepo UserRepository, sRepo SessionRepository, jwt *JWTHelper) *AuthService {
	return &AuthService{
		userRepo:    uRepo,
		sessionRepo: sRepo,
		jwtHelper:   jwt,
	}
}

// HashPassword generates a SHA-256 password hash.
func (s *AuthService) hashPassword(password string) string {
	sum := sha256.Sum256([]byte(password))
	return hex.EncodeToString(sum[:])
}

// GenerateRandomToken generates a cryptographically secure random token string.
func (s *AuthService) generateRandomToken(byteLen int) (string, error) {
	b := make([]byte, byteLen)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return hex.EncodeToString(b), nil
}

// Register creates a new user account in the repository.
func (s *AuthService) Register(ctx context.Context, req RegistrationRequest) (*User, error) {
	if req.Email == "" || req.Username == "" || req.Password == "" {
		return nil, errors.New("email, username, and password are required")
	}

	role := req.Role
	if role == "" {
		role = RoleMember
	}

	user := &User{
		ID:           fmt.Sprintf("usr_%d", time.Now().UnixNano()),
		Email:        req.Email,
		Username:     req.Username,
		PasswordHash: s.hashPassword(req.Password),
		Role:         role,
		IsActive:     true,
		CreatedAt:    time.Now().UTC(),
		UpdatedAt:    time.Now().UTC(),
	}

	if err := s.userRepo.Create(ctx, user); err != nil {
		return nil, fmt.Errorf("failed to create user: %w", err)
	}

	return user, nil
}

// AuthenticateUser verifies user credentials, generates JWT access tokens, and creates an active session.
func (s *AuthService) AuthenticateUser(ctx context.Context, creds LoginCredentials) (*AuthResult, error) {
	user, err := s.userRepo.FindByUsername(ctx, creds.Username)
	if err != nil {
		// Fallback lookup by email
		user, err = s.userRepo.FindByEmail(ctx, creds.Username)
		if err != nil {
			return nil, ErrInvalidCredentials
		}
	}

	if !user.IsActive {
		return nil, ErrInactiveAccount
	}

	expectedHash := s.hashPassword(creds.Password)
	if user.PasswordHash != expectedHash {
		return nil, ErrInvalidCredentials
	}

	scopes := []string{"read:profile", "write:orders"}
	if user.Role == RoleSuperAdmin || user.Role == RoleAdmin {
		scopes = append(scopes, "admin:all")
	}

	accessToken, err := s.jwtHelper.GenerateAccessToken(user, scopes)
	if err != nil {
		return nil, fmt.Errorf("failed to generate access token: %w", err)
	}

	refreshToken, err := s.generateRandomToken(32)
	if err != nil {
		return nil, fmt.Errorf("failed to generate refresh token: %w", err)
	}

	now := time.Now().UTC()
	session := &Session{
		ID:           fmt.Sprintf("sess_%d", now.UnixNano()),
		UserID:       user.ID,
		RefreshToken: refreshToken,
		UserAgent:    creds.UserAgent,
		ClientIP:     creds.ClientIP,
		IsRevoked:    false,
		ExpiresAt:    now.Add(30 * 24 * time.Hour),
		CreatedAt:    now,
	}

	if err := s.sessionRepo.CreateSession(ctx, session); err != nil {
		return nil, fmt.Errorf("failed to persist user session: %w", err)
	}

	return &AuthResult{
		AccessToken:  accessToken,
		RefreshToken: refreshToken,
		ExpiresAt:    now.Add(15 * time.Minute),
		User:         user,
	}, nil
}

// Authenticate is an alias for AuthenticateUser satisfying generic auth interfaces.
func (s *AuthService) Authenticate(ctx context.Context, creds LoginCredentials) (*AuthResult, error) {
	return s.AuthenticateUser(ctx, creds)
}

// RefreshToken rotates the session tokens given a valid refresh token.
func (s *AuthService) RefreshToken(ctx context.Context, oldRefreshToken string) (*AuthResult, error) {
	session, err := s.sessionRepo.FindSessionByToken(ctx, oldRefreshToken)
	if err != nil {
		return nil, ErrInvalidToken
	}

	user, err := s.userRepo.FindByID(ctx, session.UserID)
	if err != nil || !user.IsActive {
		return nil, ErrInvalidCredentials
	}

	// Revoke old session and issue new session token
	if err := s.sessionRepo.RevokeSession(ctx, session.ID); err != nil {
		return nil, err
	}

	scopes := []string{"read:profile"}
	accessToken, err := s.jwtHelper.GenerateAccessToken(user, scopes)
	if err != nil {
		return nil, err
	}

	newRefreshToken, err := s.generateRandomToken(32)
	if err != nil {
		return nil, err
	}

	now := time.Now().UTC()
	newSession := &Session{
		ID:           fmt.Sprintf("sess_%d", now.UnixNano()),
		UserID:       user.ID,
		RefreshToken: newRefreshToken,
		UserAgent:    session.UserAgent,
		ClientIP:     session.ClientIP,
		IsRevoked:    false,
		ExpiresAt:    now.Add(30 * 24 * time.Hour),
		CreatedAt:    now,
	}

	if err := s.sessionRepo.CreateSession(ctx, newSession); err != nil {
		return nil, err
	}

	return &AuthResult{
		AccessToken:  accessToken,
		RefreshToken: newRefreshToken,
		ExpiresAt:    now.Add(15 * time.Minute),
		User:         user,
	}, nil
}

// RevokeSession terminates a user session.
func (s *AuthService) RevokeSession(ctx context.Context, sessionID string) error {
	return s.sessionRepo.RevokeSession(ctx, sessionID)
}
