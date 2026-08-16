package auth

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"
)

// JWTHelper manages RSA token signing, validation, and claim parsing.
type JWTHelper struct {
	privateKey *rsa.PrivateKey
	publicKey  *rsa.PublicKey
	issuer     string
	duration   time.Duration
}

// NewJWTHelper initializes an RSA key-pair and JWT helper instance.
func NewJWTHelper(issuer string, duration time.Duration) (*JWTHelper, error) {
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		return nil, fmt.Errorf("failed to generate RSA key: %w", err)
	}
	return &JWTHelper{
		privateKey: key,
		publicKey:  &key.PublicKey,
		issuer:     issuer,
		duration:   duration,
	}, nil
}

// GenerateAccessToken signs a JWT for the given user.
func (j *JWTHelper) GenerateAccessToken(user *User, scopes []string) (string, error) {
	now := time.Now().UTC()
	claims := Claims{
		UserID:    user.ID,
		Email:     user.Email,
		Username:  user.Username,
		Role:      user.Role,
		Scopes:    scopes,
		Issuer:    j.issuer,
		Subject:   user.ID,
		Audience:  "api.corp.internal",
		IssuedAt:  now.Unix(),
		ExpiresAt: now.Add(j.duration).Unix(),
	}

	headerJSON, err := json.Marshal(map[string]string{"alg": "RS256", "typ": "JWT"})
	if err != nil {
		return "", err
	}
	claimsJSON, err := json.Marshal(claims)
	if err != nil {
		return "", err
	}

	encHeader := base64.RawURLEncoding.EncodeToString(headerJSON)
	encClaims := base64.RawURLEncoding.EncodeToString(claimsJSON)
	signingInput := fmt.Sprintf("%s.%s", encHeader, encClaims)

	hashed := sha256.Sum256([]byte(signingInput))
	signature, err := rsa.SignPKCS1v15(rand.Reader, j.privateKey, 0, hashed[:])
	if err != nil {
		return "", fmt.Errorf("failed to sign token: %w", err)
	}

	encSig := base64.RawURLEncoding.EncodeToString(signature)
	return fmt.Sprintf("%s.%s", signingInput, encSig), nil
}

// ValidateAccessToken parses and cryptographically verifies a JWT string.
func (j *JWTHelper) ValidateAccessToken(tokenString string) (*Claims, error) {
	parts := strings.Split(tokenString, ".")
	if len(parts) != 3 {
		return nil, errors.New("malformed jwt: must have 3 parts")
	}

	signingInput := fmt.Sprintf("%s.%s", parts[0], parts[1])
	sigBytes, err := base64.RawURLEncoding.DecodeString(parts[2])
	if err != nil {
		return nil, errors.New("invalid signature encoding")
	}

	hashed := sha256.Sum256([]byte(signingInput))
	if err := rsa.VerifyPKCS1v15(j.publicKey, 0, hashed[:], sigBytes); err != nil {
		return nil, fmt.Errorf("invalid token signature: %w", err)
	}

	claimsBytes, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		return nil, errors.New("invalid claims encoding")
	}

	var claims Claims
	if err := json.Unmarshal(claimsBytes, &claims); err != nil {
		return nil, errors.New("failed to unmarshal claims")
	}

	if time.Now().UTC().Unix() > claims.ExpiresAt {
		return nil, errors.New("token expired")
	}

	return &claims, nil
}
