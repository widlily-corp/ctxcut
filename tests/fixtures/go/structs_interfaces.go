package fixtures

import (
	"context"
	"fmt"
	"time"
)

// Executor defines the core operational interface.
type Executor interface {
	Execute(ctx context.Context, req ExecutionRequest) (*ExecutionResponse, error)
	Status() string
}

// BaseEntity represents embedded common audit fields.
type BaseEntity struct {
	ID        string    `json:"id"`
	CreatedAt time.Time `json:"created_at"`
	UpdatedAt time.Time `json:"updated_at"`
}

// ExecutionRequest specifies input parameters for a task execution.
type ExecutionRequest struct {
	TraceID string                 `json:"trace_id"`
	Payload map[string]interface{} `json:"payload"`
	Timeout time.Duration          `json:"timeout"`
}

// ExecutionResponse specifies output results from a task execution.
type ExecutionResponse struct {
	TraceID      string        `json:"trace_id"`
	Success      bool          `json:"success"`
	Duration     time.Duration `json:"duration"`
	OutputData   []byte        `json:"output_data"`
	ErrorMessage string        `json:"error_message,omitempty"`
}

// Service embeds BaseEntity and implements the Executor interface.
type Service struct {
	BaseEntity
	Name     string
	Version  string
	isActive bool
}

// NewService constructs a new initialized Service instance.
func NewService(id, name, version string) *Service {
	now := time.Now().UTC()
	return &Service{
		BaseEntity: BaseEntity{
			ID:        id,
			CreatedAt: now,
			UpdatedAt: now,
		},
		Name:     name,
		Version:  version,
		isActive: true,
	}
}

// Execute performs the requested operation within the given context deadline.
func (s *Service) Execute(ctx context.Context, req ExecutionRequest) (*ExecutionResponse, error) {
	start := time.Now()
	if !s.isActive {
		return nil, fmt.Errorf("service %s (ID: %s) is currently inactive", s.Name, s.ID)
	}

	select {
	case <-ctx.Done():
		return &ExecutionResponse{
			TraceID:      req.TraceID,
			Success:      false,
			Duration:     time.Since(start),
			ErrorMessage: ctx.Err().Error(),
		}, ctx.Err()
	default:
		// Simulated successful execution
		return &ExecutionResponse{
			TraceID:    req.TraceID,
			Success:    true,
			Duration:   time.Since(start),
			OutputData: []byte(`{"status":"completed"}`),
		}, nil
	}
}

// Status returns the current running status string of the service.
func (s *Service) Status() string {
	if s.isActive {
		return "RUNNING"
	}
	return "STOPPED"
}
