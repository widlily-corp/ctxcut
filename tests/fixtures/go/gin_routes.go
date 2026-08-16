package fixtures

import (
	"net/http"
	"time"
)

// Context represents a Gin/HTTP request-response lifecycle context.
type Context struct {
	Request  *http.Request
	Writer   http.ResponseWriter
	Keys     map[string]interface{}
	Errors   []error
	isAborted bool
}

func (c *Context) JSON(code int, obj interface{}) {
	// Mock JSON serializer response
}

func (c *Context) BindJSON(obj interface{}) error {
	// Mock JSON request body unmarshaler
	return nil
}

func (c *Context) Param(key string) string {
	return "usr_sample_123"
}

func (c *Context) AbortWithStatusJSON(code int, jsonObj interface{}) {
	c.isAborted = true
	c.JSON(code, jsonObj)
}

func (c *Context) Next() {
	// Proceed to next middleware/handler in chain
}

func (c *Context) Set(key string, value interface{}) {
	if c.Keys == nil {
		c.Keys = make(map[string]interface{})
	}
	c.Keys[key] = value
}

// HandlerFunc defines the signature for Gin route handlers and middleware.
type HandlerFunc func(*Context)

// RouterGroup represents a route grouping mechanism in Gin.
type RouterGroup struct {
	BasePath string
	Handlers []HandlerFunc
	Engine   *Engine
}

func (g *RouterGroup) Group(relativePath string, handlers ...HandlerFunc) *RouterGroup {
	return &RouterGroup{
		BasePath: g.BasePath + relativePath,
		Handlers: append(g.Handlers, handlers...),
		Engine:   g.Engine,
	}
}

func (g *RouterGroup) GET(relativePath string, handlers ...HandlerFunc) {
	// Register GET route
}

func (g *RouterGroup) POST(relativePath string, handlers ...HandlerFunc) {
	// Register POST route
}

func (g *RouterGroup) PUT(relativePath string, handlers ...HandlerFunc) {
	// Register PUT route
}

// Engine represents the top-level Gin HTTP router engine.
type Engine struct {
	RouterGroup
}

func NewEngine() *Engine {
	e := &Engine{}
	e.RouterGroup = RouterGroup{BasePath: "", Engine: e}
	return e
}

// DTOs for Gin routes

type LoginRequestDTO struct {
	Username string `json:"username" binding:"required"`
	Password string `json:"password" binding:"required"`
	MFACode  string `json:"mfa_code,omitempty"`
}

type LoginResponseDTO struct {
	AccessToken  string    `json:"access_token"`
	RefreshToken string    `json:"refresh_token"`
	ExpiresAt    time.Time `json:"expires_at"`
	TokenType    string    `json:"token_type"`
}

type UserProfileDTO struct {
	UserID    string    `json:"user_id"`
	Email     string    `json:"email"`
	Role      string    `json:"role"`
	CreatedAt time.Time `json:"created_at"`
}

// AuthMiddleware inspects Authorization header for bearer tokens.
func AuthMiddleware() HandlerFunc {
	return func(c *Context) {
		authHeader := c.Request.Header.Get("Authorization")
		if authHeader == "" {
			c.AbortWithStatusJSON(http.StatusUnauthorized, map[string]string{"error": "missing authorization header"})
			return
		}
		c.Set("current_user_id", "usr_authenticated_777")
		c.Next()
	}
}

// LoginHandler handles user authentication and JWT token issuance.
func LoginHandler(c *Context) {
	var req LoginRequestDTO
	if err := c.BindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, map[string]string{"error": "invalid login request payload"})
		return
	}

	response := LoginResponseDTO{
		AccessToken:  "mock.jwt.access.token.header.payload.signature",
		RefreshToken: "mock.jwt.refresh.token",
		ExpiresAt:    time.Now().Add(15 * time.Minute),
		TokenType:    "Bearer",
	}

	c.JSON(http.StatusOK, response)
}

// GetUserProfileHandler retrieves the profile for a requested user ID.
func GetUserProfileHandler(c *Context) {
	userID := c.Param("user_id")
	profile := UserProfileDTO{
		UserID:    userID,
		Email:     "user@corp.internal",
		Role:      "staff_engineer",
		CreatedAt: time.Now().Add(-24 * time.Hour * 365),
	}
	c.JSON(http.StatusOK, profile)
}

// RegisterAuthRoutes configures the route hierarchy on a Gin engine.
func RegisterAuthRoutes(r *Engine) {
	v1 := r.Group("/v1/auth")
	{
		v1.POST("/login", LoginHandler)
		v1.GET("/profile/:user_id", AuthMiddleware(), GetUserProfileHandler)
	}
}
