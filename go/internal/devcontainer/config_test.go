//go:build small

package devcontainer

import (
	"encoding/json"
	"reflect"
	"testing"
)

func ptr[T any](v T) *T {
	return &v
}

func TestAppPortsUnmarshalJSON(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		want    AppPorts
		wantErr bool
	}{
		{
			name:  "when unmarshal app port with number then normalizes to loopback mapping",
			input: "8080",
			want:  AppPorts{"127.0.0.1:8080:8080"},
		},
		{
			name:  "when unmarshal app port with string then uses as is",
			input: `"8080:80"`,
			want:  AppPorts{"8080:80"},
		},
		{
			name:  "when unmarshal app port with array of numbers then normalizes each",
			input: "[3000, 4000]",
			want:  AppPorts{"127.0.0.1:3000:3000", "127.0.0.1:4000:4000"},
		},
		{
			name:  "when unmarshal app port with array of strings then uses each as is",
			input: `["3000:3000", "4000:80"]`,
			want:  AppPorts{"3000:3000", "4000:80"},
		},
		{
			name:  "when unmarshal app port with null then returns empty",
			input: "null",
			want:  AppPorts{},
		},
		{
			name:    "when unmarshal app port with fractional number then returns error",
			input:   "8080.5",
			wantErr: true,
		},
		{
			name:    "when unmarshal app port with negative number then returns error",
			input:   "-1",
			wantErr: true,
		},
		{
			name:    "when unmarshal app port with object then returns error",
			input:   "{}",
			wantErr: true,
		},
		{
			name:    "when unmarshal app port with array containing boolean then returns error",
			input:   "[true]",
			wantErr: true,
		},
		{
			name:    "when unmarshal app port with invalid json then returns error",
			input:   "{",
			wantErr: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var got AppPorts
			err := got.UnmarshalJSON([]byte(tt.input))
			if tt.wantErr {
				if err == nil {
					t.Errorf("Unmarshal(%q) expected error, got %v", tt.input, got)
				}
				return
			}
			if err != nil {
				t.Fatalf("Unmarshal(%q) error: %v", tt.input, err)
			}
			if !reflect.DeepEqual(got, tt.want) {
				t.Errorf("Unmarshal(%q) = %v, want %v", tt.input, got, tt.want)
			}
		})
	}
}

func TestStringOrListUnmarshalJSON(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		want    StringOrList
		wantErr bool
	}{
		{
			name:  "when unmarshal string or list with string then returns single element list",
			input: `"hello"`,
			want:  StringOrList{"hello"},
		},
		{
			name:  "when unmarshal string or list with array then returns list",
			input: `["hello", "world"]`,
			want:  StringOrList{"hello", "world"},
		},
		{
			name:  "when unmarshal string or list with null then returns nil",
			input: "null",
			want:  nil,
		},
		{
			name:    "when unmarshal string or list with array containing number then returns error",
			input:   `["hello", 42]`,
			wantErr: true,
		},
		{
			name:    "when unmarshal string or list with number then returns error",
			input:   "42",
			wantErr: true,
		},
		{
			name:    "when unmarshal string or list with invalid json then returns error",
			input:   "{",
			wantErr: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var got StringOrList
			err := got.UnmarshalJSON([]byte(tt.input))
			if tt.wantErr {
				if err == nil {
					t.Errorf("Unmarshal(%q) expected error, got %v", tt.input, got)
				}
				return
			}
			if err != nil {
				t.Fatalf("Unmarshal(%q) error: %v", tt.input, err)
			}
			if !reflect.DeepEqual(got, tt.want) {
				t.Errorf("Unmarshal(%q) = %v, want %v", tt.input, got, tt.want)
			}
		})
	}
}

func TestUserEnvProbeUnmarshalJSON(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		want    UserEnvProbe
		wantErr bool
	}{
		{
			name:  "when unmarshal user env probe with none then returns none",
			input: `"none"`,
			want:  UserEnvProbeNone,
		},
		{
			name:  "when unmarshal user env probe with login interactive shell then returns login interactive shell",
			input: `"loginInteractiveShell"`,
			want:  UserEnvProbeLoginInteractiveShell,
		},
		{
			name:  "when unmarshal user env probe with interactive shell then returns interactive shell",
			input: `"interactiveShell"`,
			want:  UserEnvProbeInteractiveShell,
		},
		{
			name:  "when unmarshal user env probe with login shell then returns login shell",
			input: `"loginShell"`,
			want:  UserEnvProbeLoginShell,
		},
		{
			name:    "when unmarshal user env probe with unknown value then returns error",
			input:   `"unknownProbe"`,
			wantErr: true,
		},
		{
			name:    "when unmarshal user env probe with number then returns error",
			input:   "42",
			wantErr: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var got UserEnvProbe
			err := json.Unmarshal([]byte(tt.input), &got)
			if tt.wantErr {
				if err == nil {
					t.Errorf("Unmarshal(%q) expected error, got %v", tt.input, got)
				}
				return
			}
			if err != nil {
				t.Fatalf("Unmarshal(%q) error: %v", tt.input, err)
			}
			if got != tt.want {
				t.Errorf("Unmarshal(%q) = %v, want %v", tt.input, got, tt.want)
			}
		})
	}
}

func TestWaitForUnmarshalJSON(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		want    WaitFor
		wantErr bool
	}{
		{
			name:  "when unmarshal wait for with initialize command then returns initialize command",
			input: `"initializeCommand"`,
			want:  WaitForInitializeCommand,
		},
		{
			name:  "when unmarshal wait for with on create command then returns on create command",
			input: `"onCreateCommand"`,
			want:  WaitForOnCreateCommand,
		},
		{
			name:  "when unmarshal wait for with update content command then returns update content command",
			input: `"updateContentCommand"`,
			want:  WaitForUpdateContentCommand,
		},
		{
			name:  "when unmarshal wait for with post create command then returns post create command",
			input: `"postCreateCommand"`,
			want:  WaitForPostCreateCommand,
		},
		{
			name:  "when unmarshal wait for with post start command then returns post start command",
			input: `"postStartCommand"`,
			want:  WaitForPostStartCommand,
		},
		{
			name:  "when unmarshal wait for with post attach command then returns post attach command",
			input: `"postAttachCommand"`,
			want:  WaitForPostAttachCommand,
		},
		{
			name:    "when unmarshal wait for with unknown value then returns error",
			input:   `"unknownCommand"`,
			wantErr: true,
		},
		{
			name:    "when unmarshal wait for with number then returns error",
			input:   "42",
			wantErr: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var got WaitFor
			err := json.Unmarshal([]byte(tt.input), &got)
			if tt.wantErr {
				if err == nil {
					t.Errorf("Unmarshal(%q) expected error, got %v", tt.input, got)
				}
				return
			}
			if err != nil {
				t.Fatalf("Unmarshal(%q) error: %v", tt.input, err)
			}
			if got != tt.want {
				t.Errorf("Unmarshal(%q) = %v, want %v", tt.input, got, tt.want)
			}
		})
	}
}

func TestWaitForRequires(t *testing.T) {
	tests := []struct {
		name string
		self WaitFor
		cmd  WaitFor
		want bool
	}{
		{
			name: "when wait for post create requires on create then returns true",
			self: WaitForPostCreateCommand,
			cmd:  WaitForOnCreateCommand,
			want: true,
		},
		{
			name: "when wait for on create requires post create then returns false",
			self: WaitForOnCreateCommand,
			cmd:  WaitForPostCreateCommand,
			want: false,
		},
		{
			name: "when wait for on create requires on create then returns true",
			self: WaitForOnCreateCommand,
			cmd:  WaitForOnCreateCommand,
			want: true,
		},
		{
			name: "when wait for post attach requires initialize then returns true",
			self: WaitForPostAttachCommand,
			cmd:  WaitForInitializeCommand,
			want: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.self.Requires(tt.cmd); got != tt.want {
				t.Errorf("(%s).Requires(%s) = %v, want %v", tt.self, tt.cmd, got, tt.want)
			}
		})
	}
}

func TestConfigCommon(t *testing.T) {
	tests := []struct {
		name   string
		config Config
		want   string
	}{
		{
			name: "when common for image then returns image common",
			config: &ImageConfig{
				Image:        "img",
				CommonConfig: CommonConfig{Name: ptr("my-image")},
			},
			want: "my-image",
		},
		{
			name: "when common for dockerfile then returns dockerfile common",
			config: &DockerfileConfig{
				DockerFile:   "Dockerfile",
				CommonConfig: CommonConfig{Name: ptr("my-dockerfile")},
			},
			want: "my-dockerfile",
		},
		{
			name: "when common for dockerfile build then returns dockerfile build common",
			config: &DockerfileBuildConfig{
				CommonConfig: CommonConfig{Name: ptr("my-build")},
			},
			want: "my-build",
		},
		{
			name: "when common for compose then returns compose common",
			config: &DockerComposeConfig{
				DockerComposeFile: StringOrList{"docker-compose.yml"},
				Service:           "app",
				WorkspaceFolder:   "/workspace",
				CommonConfig:      CommonConfig{Name: ptr("my-compose")},
			},
			want: "my-compose",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.config.Common().Name
			if got == nil || *got != tt.want {
				t.Errorf("Common().Name = %v, want %q", got, tt.want)
			}
		})
	}
}

func TestResolveWorkspaceFolder(t *testing.T) {
	tests := []struct {
		name   string
		config Config
		cwd    string
		want   string
	}{
		{
			name: "when resolve workspace folder with explicit path then returns it",
			config: &ImageConfig{
				Image:        "img",
				CommonConfig: CommonConfig{WorkspaceFolder: ptr("/workspace")},
			},
			cwd:  "/home/user/myproject",
			want: "/workspace",
		},
		{
			name:   "when resolve workspace folder without explicit path then uses cwd basename",
			config: &ImageConfig{Image: "img"},
			cwd:    "/home/user/myproject",
			want:   "/workspaces/myproject",
		},
		{
			name: "when resolve workspace folder for dockerfile variant with explicit path then returns it",
			config: &DockerfileConfig{
				DockerFile:   "Dockerfile",
				CommonConfig: CommonConfig{WorkspaceFolder: ptr("/workspace")},
			},
			cwd:  "/home/user/myproject",
			want: "/workspace",
		},
		{
			name: "when resolve workspace folder for dockerfile build variant with explicit path then returns it",
			config: &DockerfileBuildConfig{
				CommonConfig: CommonConfig{WorkspaceFolder: ptr("/workspace")},
			},
			cwd:  "/home/user/myproject",
			want: "/workspace",
		},
		{
			name: "when resolve workspace folder with local workspace folder basename variable then expands it",
			config: &ImageConfig{
				Image:        "img",
				CommonConfig: CommonConfig{WorkspaceFolder: ptr("/workspaces/${localWorkspaceFolderBasename}")},
			},
			cwd:  "/home/user/myproject",
			want: "/workspaces/myproject",
		},
		{
			name: "when resolve workspace folder for compose then returns compose field",
			config: &DockerComposeConfig{
				DockerComposeFile: StringOrList{"docker-compose.yml"},
				Service:           "app",
				WorkspaceFolder:   "/workspace",
			},
			cwd:  "/home/user/myproject",
			want: "/workspace",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.config.ResolveWorkspaceFolder(tt.cwd); got != tt.want {
				t.Errorf("ResolveWorkspaceFolder(%q) = %q, want %q", tt.cwd, got, tt.want)
			}
		})
	}
}
