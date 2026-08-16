package auth

import (
	"context"
	"errors"
	"sync"
	"time"
)

var (
	ErrUserNotFound    = errors.New("user not found")
	ErrUserExists      = errors.New("user already exists with that email or username")
	ErrSessionNotFound = errors.New("session not found")
)

// UserRepository defines persistence operations for users.
type UserRepository interface {
	FindByID(ctx context.Context, id string) (*User, error)
	FindByEmail(ctx context.Context, email string) (*User, error)
	FindByUsername(ctx context.Context, username string) (*User, error)
	Create(ctx context.Context, user *User) error
	Update(ctx context.Context, user *User) error
}

// SessionRepository defines persistence operations for login sessions.
type SessionRepository interface {
	CreateSession(ctx context.Context, session *Session) error
	FindSessionByToken(ctx context.Context, refreshToken string) (*Session, error)
	RevokeSession(ctx context.Context, sessionID string) error
	RevokeAllUserSessions(ctx context.Context, userID string) error
}

// MemoryAuthRepository provides an in-memory thread-safe implementation of both repositories.
type MemoryAuthRepository struct {
	mu       sync.RWMutex
	users    map[string]*User
	byEmail  map[string]string
	byName   map[string]string
	sessions map[string]*Session
	byToken  map[string]string
}

// NewMemoryAuthRepository creates a new initialized MemoryAuthRepository instance.
func NewMemoryAuthRepository() *MemoryAuthRepository {
	return &MemoryAuthRepository{
		users:    make(map[string]*User),
		byEmail:  make(map[string]string),
		byName:   make(map[string]string),
		sessions: make(map[string]*Session),
		byToken:  make(map[string]string),
	}
}

func (r *MemoryAuthRepository) FindByID(ctx context.Context, id string) (*User, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	user, exists := r.users[id]
	if !exists {
		return nil, ErrUserNotFound
	}
	return user, nil
}

func (r *MemoryAuthRepository) FindByEmail(ctx context.Context, email string) (*User, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	id, exists := r.byEmail[email]
	if !exists {
		return nil, ErrUserNotFound
	}
	return r.users[id], nil
}

func (r *MemoryAuthRepository) FindByUsername(ctx context.Context, username string) (*User, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	id, exists := r.byName[username]
	if !exists {
		return nil, ErrUserNotFound
	}
	return r.users[id], nil
}

func (r *MemoryAuthRepository) Create(ctx context.Context, user *User) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if _, exists := r.byEmail[user.Email]; exists {
		return ErrUserExists
	}
	if _, exists := r.byName[user.Username]; exists {
		return ErrUserExists
	}
	r.users[user.ID] = user
	r.byEmail[user.Email] = user.ID
	r.byName[user.Username] = user.ID
	return nil
}

func (r *MemoryAuthRepository) Update(ctx context.Context, user *User) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if _, exists := r.users[user.ID]; !exists {
		return ErrUserNotFound
	}
	r.users[user.ID] = user
	return nil
}

func (r *MemoryAuthRepository) CreateSession(ctx context.Context, session *Session) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.sessions[session.ID] = session
	r.byToken[session.RefreshToken] = session.ID
	return nil
}

func (r *MemoryAuthRepository) FindSessionByToken(ctx context.Context, refreshToken string) (*Session, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	id, exists := r.byToken[refreshToken]
	if !exists {
		return nil, ErrSessionNotFound
	}
	session, exists := r.sessions[id]
	if !exists || session.IsRevoked || session.ExpiresAt.Before(time.Now()) {
		return nil, ErrSessionNotFound
	}
	return session, nil
}

func (r *MemoryAuthRepository) RevokeSession(ctx context.Context, sessionID string) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	session, exists := r.sessions[sessionID]
	if !exists {
		return ErrSessionNotFound
	}
	session.IsRevoked = true
	return nil
}

func (r *MemoryAuthRepository) RevokeAllUserSessions(ctx context.Context, userID string) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	for _, session := range r.sessions {
		if session.UserID == userID {
			session.IsRevoked = true
		}
	}
	return nil
}
