package auth

import (
	"time"
)

// Role represents user authorization roles in the system.
type Role string

const (
	RoleSuperAdmin Role = "SUPER_ADMIN"
	RoleAdmin      Role = "ADMIN"
	RoleMember     Role = "MEMBER"
	RoleReadOnly   Role = "READ_ONLY"
)

// User represents the database entity for an authenticated user account.
type User struct {
	ID           string    `json:"id" gorm:"primaryKey;type:varchar(64)"`
	Email        string    `json:"email" gorm:"uniqueIndex;type:varchar(255);not null"`
	Username     string    `json:"username" gorm:"uniqueIndex;type:varchar(100);not null"`
	PasswordHash string    `json:"-" gorm:"type:varchar(255);not null"`
	Role         Role      `json:"role" gorm:"type:varchar(32);default:'MEMBER'"`
	IsActive     bool      `json:"is_active" gorm:"default:true"`
	MFASecret    *string   `json:"-" gorm:"type:varchar(128)"`
	CreatedAt    time.Time `json:"created_at" gorm:"autoCreateTime"`
	UpdatedAt    time.Time `json:"updated_at" gorm:"autoUpdateTime"`
}

// Session represents an active login session tied to a refresh token.
type Session struct {
	ID           string    `json:"id" gorm:"primaryKey;type:varchar(64)"`
	UserID       string    `json:"user_id" gorm:"index;type:varchar(64);not null"`
	RefreshToken string    `json:"-" gorm:"uniqueIndex;type:varchar(512);not null"`
	UserAgent    string    `json:"user_agent" gorm:"type:varchar(512)"`
	ClientIP     string    `json:"client_ip" gorm:"type:varchar(45)"`
	IsRevoked    bool      `json:"is_revoked" gorm:"default:false"`
	ExpiresAt    time.Time `json:"expires_at" gorm:"not null"`
	CreatedAt    time.Time `json:"created_at" gorm:"autoCreateTime"`
}

// Claims represents the JWT token claims structure.
type Claims struct {
	UserID    string   `json:"uid"`
	Email     string   `json:"email"`
	Username  string   `json:"usr"`
	Role      Role     `json:"rol"`
	Scopes    []string `json:"scp"`
	Issuer    string   `json:"iss"`
	Subject   string   `json:"sub"`
	Audience  string   `json:"aud"`
	ExpiresAt int64    `json:"exp"`
	IssuedAt  int64    `json:"iat"`
}

// AuthResult holds the resulting authentication tokens and metadata.
type AuthResult struct {
	AccessToken  string    `json:"access_token"`
	RefreshToken string    `json:"refresh_token"`
	ExpiresAt    time.Time `json:"expires_at"`
	User         *User     `json:"user"`
}

// LoginCredentials specifies user authentication parameters.
type LoginCredentials struct {
	Username  string `json:"username"`
	Password  string `json:"password"`
	ClientIP  string `json:"client_ip"`
	UserAgent string `json:"user_agent"`
}

// RegistrationRequest specifies parameters for creating a new user account.
type RegistrationRequest struct {
	Email    string `json:"email"`
	Username string `json:"username"`
	Password string `json:"password"`
	Role     Role   `json:"role"`
}
