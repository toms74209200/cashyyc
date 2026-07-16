//go:build small

package devcontainer

import (
	"reflect"
	"testing"
)

func TestParseConfigVariants(t *testing.T) {
	tests := []struct {
		name  string
		input string
		check func(t *testing.T, got Config)
	}{
		{
			name:  "when parse config with image then returns image variant",
			input: `{"name": "Rust", "image": "mcr.microsoft.com/devcontainers/rust:2-1-trixie"}`,
			check: func(t *testing.T, got Config) {
				c, ok := got.(*ImageConfig)
				if !ok {
					t.Fatalf("expected *ImageConfig, got %T", got)
				}
				if c.Image != "mcr.microsoft.com/devcontainers/rust:2-1-trixie" {
					t.Errorf("Image = %q", c.Image)
				}
				if c.Name == nil || *c.Name != "Rust" {
					t.Errorf("Name = %v, want Rust", c.Name)
				}
			},
		},
		{
			name:  "when parse config with docker file then returns dockerfile variant",
			input: `{"name": "Dev", "dockerFile": "Dockerfile"}`,
			check: func(t *testing.T, got Config) {
				c, ok := got.(*DockerfileConfig)
				if !ok {
					t.Fatalf("expected *DockerfileConfig, got %T", got)
				}
				if c.DockerFile != "Dockerfile" {
					t.Errorf("DockerFile = %q", c.DockerFile)
				}
				if c.Name == nil || *c.Name != "Dev" {
					t.Errorf("Name = %v, want Dev", c.Name)
				}
			},
		},
		{
			name:  "when parse config with build dockerfile then returns dockerfilebuild variant",
			input: `{"build": {"dockerfile": "Dockerfile"}}`,
			check: func(t *testing.T, got Config) {
				if _, ok := got.(*DockerfileBuildConfig); !ok {
					t.Fatalf("expected *DockerfileBuildConfig, got %T", got)
				}
			},
		},
		{
			name:  "when parse config with docker compose file string then returns dockercompose variant",
			input: `{"dockerComposeFile": "docker-compose.yml", "service": "app", "workspaceFolder": "/workspace"}`,
			check: func(t *testing.T, got Config) {
				c, ok := got.(*DockerComposeConfig)
				if !ok {
					t.Fatalf("expected *DockerComposeConfig, got %T", got)
				}
				if !reflect.DeepEqual(c.DockerComposeFile, StringOrList{"docker-compose.yml"}) {
					t.Errorf("DockerComposeFile = %v", c.DockerComposeFile)
				}
				if c.Service != "app" {
					t.Errorf("Service = %q", c.Service)
				}
			},
		},
		{
			name:  "when parse config with docker compose file array then returns dockercompose variant",
			input: `{"dockerComposeFile": ["docker-compose.yml", "docker-compose.override.yml"], "service": "app", "workspaceFolder": "/workspace"}`,
			check: func(t *testing.T, got Config) {
				c, ok := got.(*DockerComposeConfig)
				if !ok {
					t.Fatalf("expected *DockerComposeConfig, got %T", got)
				}
				want := StringOrList{"docker-compose.yml", "docker-compose.override.yml"}
				if !reflect.DeepEqual(c.DockerComposeFile, want) {
					t.Errorf("DockerComposeFile = %v, want %v", c.DockerComposeFile, want)
				}
			},
		},
		{
			name:  "when parse config with docker compose run services then parses correctly",
			input: `{"dockerComposeFile": "docker-compose.yml", "service": "app", "workspaceFolder": "/workspace", "runServices": ["db", "cache"], "shutdownAction": "stopCompose"}`,
			check: func(t *testing.T, got Config) {
				c, ok := got.(*DockerComposeConfig)
				if !ok {
					t.Fatalf("expected *DockerComposeConfig, got %T", got)
				}
				if !reflect.DeepEqual(c.RunServices, []string{"db", "cache"}) {
					t.Errorf("RunServices = %v", c.RunServices)
				}
				if c.ShutdownAction == nil || *c.ShutdownAction != "stopCompose" {
					t.Errorf("ShutdownAction = %v", c.ShutdownAction)
				}
				if c.WorkspaceFolder != "/workspace" {
					t.Errorf("WorkspaceFolder = %q", c.WorkspaceFolder)
				}
			},
		},
		{
			name: "when parse config with dockerfilebuild then parses all fields",
			input: `{
				"build": {
					"dockerfile": "Dockerfile",
					"context": ".",
					"target": "dev",
					"args": {"KEY": "value"},
					"options": ["--no-cache"]
				},
				"appPort": 3000,
				"runArgs": ["--rm"],
				"workspaceMount": "source=${localWorkspaceFolder},target=/workspace,type=bind",
				"shutdownAction": "none"
			}`,
			check: func(t *testing.T, got Config) {
				c, ok := got.(*DockerfileBuildConfig)
				if !ok {
					t.Fatalf("expected *DockerfileBuildConfig, got %T", got)
				}
				if c.Build.Dockerfile == nil || *c.Build.Dockerfile != "Dockerfile" {
					t.Errorf("Build.Dockerfile = %v", c.Build.Dockerfile)
				}
				if c.Build.Context == nil || *c.Build.Context != "." {
					t.Errorf("Build.Context = %v", c.Build.Context)
				}
				if c.Build.Target == nil || *c.Build.Target != "dev" {
					t.Errorf("Build.Target = %v", c.Build.Target)
				}
				if c.Build.Args["KEY"] != "value" {
					t.Errorf("Build.Args = %v", c.Build.Args)
				}
				if !reflect.DeepEqual(c.Build.Options, []string{"--no-cache"}) {
					t.Errorf("Build.Options = %v", c.Build.Options)
				}
				if !reflect.DeepEqual(c.AppPort, AppPorts{"127.0.0.1:3000:3000"}) {
					t.Errorf("AppPort = %v", c.AppPort)
				}
				if !reflect.DeepEqual(c.RunArgs, []string{"--rm"}) {
					t.Errorf("RunArgs = %v", c.RunArgs)
				}
				if c.WorkspaceMount == nil {
					t.Error("WorkspaceMount = nil")
				}
				if c.ShutdownAction == nil || *c.ShutdownAction != "none" {
					t.Errorf("ShutdownAction = %v", c.ShutdownAction)
				}
			},
		},
		{
			name:  "when parse config with common fields then parses correctly",
			input: `{"image": "rust:latest", "remoteUser": "vscode", "postCreateCommand": "cargo build", "features": {}}`,
			check: func(t *testing.T, got Config) {
				c, ok := got.(*ImageConfig)
				if !ok {
					t.Fatalf("expected *ImageConfig, got %T", got)
				}
				if c.RemoteUser == nil || *c.RemoteUser != "vscode" {
					t.Errorf("RemoteUser = %v", c.RemoteUser)
				}
				if c.PostCreateCommand == nil {
					t.Error("PostCreateCommand = nil")
				}
			},
		},
		{
			name:  "when parse config with container env then parses correctly",
			input: `{"image": "rust:latest", "containerEnv": {"RUST_LOG": "debug", "PORT": "8080"}}`,
			check: func(t *testing.T, got Config) {
				env := got.Common().ContainerEnv
				if env["RUST_LOG"] != "debug" || env["PORT"] != "8080" {
					t.Errorf("ContainerEnv = %v", env)
				}
			},
		},
		{
			name:  "when parse config with remote env null value then parses correctly",
			input: `{"image": "rust:latest", "remoteEnv": {"UNSET_VAR": null, "SET_VAR": "value"}}`,
			check: func(t *testing.T, got Config) {
				env := got.Common().RemoteEnv
				unset, ok := env["UNSET_VAR"]
				if !ok || unset != nil {
					t.Errorf("UNSET_VAR = %v, present=%v, want present null", unset, ok)
				}
				set := env["SET_VAR"]
				if set == nil || *set != "value" {
					t.Errorf("SET_VAR = %v, want value", set)
				}
			},
		},
		{
			name:  "when parse config with cap add and security opt then parses correctly",
			input: `{"image": "rust:latest", "capAdd": ["SYS_PTRACE"], "securityOpt": ["seccomp=unconfined"]}`,
			check: func(t *testing.T, got Config) {
				common := got.Common()
				if !reflect.DeepEqual(common.CapAdd, []string{"SYS_PTRACE"}) {
					t.Errorf("CapAdd = %v", common.CapAdd)
				}
				if !reflect.DeepEqual(common.SecurityOpt, []string{"seccomp=unconfined"}) {
					t.Errorf("SecurityOpt = %v", common.SecurityOpt)
				}
			},
		},
		{
			name: "when parse config with ports attributes then parses correctly",
			input: `{
				"image": "rust:latest",
				"portsAttributes": {
					"3000": {"label": "Application", "onAutoForward": "notify", "elevateIfNeeded": false}
				}
			}`,
			check: func(t *testing.T, got Config) {
				attrs, ok := got.Common().PortsAttributes["3000"]
				if !ok {
					t.Fatalf("PortsAttributes[3000] missing")
				}
				if attrs.Label == nil || *attrs.Label != "Application" {
					t.Errorf("Label = %v", attrs.Label)
				}
				if attrs.OnAutoForward == nil || *attrs.OnAutoForward != "notify" {
					t.Errorf("OnAutoForward = %v", attrs.OnAutoForward)
				}
				if attrs.ElevateIfNeeded == nil || *attrs.ElevateIfNeeded != false {
					t.Errorf("ElevateIfNeeded = %v", attrs.ElevateIfNeeded)
				}
			},
		},
		{
			name:  "when parse config with host requirements then parses correctly",
			input: `{"image": "rust:latest", "hostRequirements": {"cpus": 4, "memory": "8gb", "storage": "32gb"}}`,
			check: func(t *testing.T, got Config) {
				hr := got.Common().HostRequirements
				if hr == nil {
					t.Fatal("HostRequirements = nil")
				}
				if hr.Cpus == nil || *hr.Cpus != 4 {
					t.Errorf("Cpus = %v", hr.Cpus)
				}
				if hr.Memory == nil || *hr.Memory != "8gb" {
					t.Errorf("Memory = %v", hr.Memory)
				}
				if hr.Storage == nil || *hr.Storage != "32gb" {
					t.Errorf("Storage = %v", hr.Storage)
				}
			},
		},
		{
			name:  "when parse config with build cache from as string then returns single element list",
			input: `{"build": {"dockerfile": "Dockerfile", "cacheFrom": "myimage:latest"}}`,
			check: func(t *testing.T, got Config) {
				c := got.(*DockerfileBuildConfig)
				if !reflect.DeepEqual(c.Build.CacheFrom, StringOrList{"myimage:latest"}) {
					t.Errorf("CacheFrom = %v", c.Build.CacheFrom)
				}
			},
		},
		{
			name:  "when parse config with build cache from as array then parses all elements",
			input: `{"build": {"dockerfile": "Dockerfile", "cacheFrom": ["image1:latest", "image2:latest"]}}`,
			check: func(t *testing.T, got Config) {
				c := got.(*DockerfileBuildConfig)
				want := StringOrList{"image1:latest", "image2:latest"}
				if !reflect.DeepEqual(c.Build.CacheFrom, want) {
					t.Errorf("CacheFrom = %v, want %v", c.Build.CacheFrom, want)
				}
			},
		},
		{
			name:  "when parse config with build cache from null then is nil",
			input: `{"build": {"dockerfile": "Dockerfile", "cacheFrom": null}}`,
			check: func(t *testing.T, got Config) {
				c := got.(*DockerfileBuildConfig)
				if c.Build.CacheFrom != nil {
					t.Errorf("CacheFrom = %v, want nil", c.Build.CacheFrom)
				}
			},
		},
		{
			name:  "when parse config with build cache from absent then is nil",
			input: `{"build": {"dockerfile": "Dockerfile"}}`,
			check: func(t *testing.T, got Config) {
				c := got.(*DockerfileBuildConfig)
				if c.Build.CacheFrom != nil {
					t.Errorf("CacheFrom = %v, want nil", c.Build.CacheFrom)
				}
			},
		},
		{
			name:  "when parse config with user env probe then parses correctly",
			input: `{"image": "rust:latest", "userEnvProbe": "loginInteractiveShell"}`,
			check: func(t *testing.T, got Config) {
				p := got.Common().UserEnvProbe
				if p == nil || *p != UserEnvProbeLoginInteractiveShell {
					t.Errorf("UserEnvProbe = %v", p)
				}
			},
		},
		{
			name:  "when parse config with wait for then parses correctly",
			input: `{"image": "rust:latest", "waitFor": "onCreateCommand"}`,
			check: func(t *testing.T, got Config) {
				w := got.Common().WaitFor
				if w == nil || *w != WaitForOnCreateCommand {
					t.Errorf("WaitFor = %v", w)
				}
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ParseConfig(tt.input)
			if got == nil {
				t.Fatal("ParseConfig returned nil")
			}
			tt.check(t, got)
		})
	}
}

func TestParseConfigJsonc(t *testing.T) {
	tests := []struct {
		name  string
		input string
		image string
	}{
		{
			name:  "when parse config with trailing comma in object then succeeds",
			input: `{"image": "rust:latest",}`,
			image: "rust:latest",
		},
		{
			name:  "when parse config with comma in string value then preserves value",
			input: `{"image": "registry.example.com,backup:latest"}`,
			image: "registry.example.com,backup:latest",
		},
		{
			name:  "when parse config with double slash in string value then preserves value",
			input: `{"image": "http://registry.example.com"}`,
			image: "http://registry.example.com",
		},
		{
			name:  "when parse config with block comment marker in string value then preserves value",
			input: `{"image": "value /* not a comment */ end"}`,
			image: "value /* not a comment */ end",
		},
		{
			name:  "when parse config with line comments then succeeds",
			input: "{\n// comment\n\"name\": \"Rust\",\n\"image\": \"rust:latest\"\n}",
			image: "rust:latest",
		},
		{
			name:  "when parse config with block comments then succeeds",
			input: `{ /* comment */ "name": "Rust", "image": "rust:latest" }`,
			image: "rust:latest",
		},
		{
			name:  "when parse config with multiline block comment then succeeds",
			input: "{\n/* line1\n   line2 */\n\"image\": \"rust:latest\"\n}",
			image: "rust:latest",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ParseConfig(tt.input)
			c, ok := got.(*ImageConfig)
			if !ok {
				t.Fatalf("expected *ImageConfig, got %T", got)
			}
			if c.Image != tt.image {
				t.Errorf("Image = %q, want %q", c.Image, tt.image)
			}
		})
	}
}

func TestParseConfigStringEscapes(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{
			name:  "when parse config with escaped backslash in string then succeeds",
			input: `{"image": "rust:latest", "name": "test\\value"}`,
			want:  `test\value`,
		},
		{
			name:  "when parse config with escaped quote in string then preserves name",
			input: `{"image": "rust:latest", "name": "say \"hello\""}`,
			want:  `say "hello"`,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ParseConfig(tt.input)
			c, ok := got.(*ImageConfig)
			if !ok {
				t.Fatalf("expected *ImageConfig, got %T", got)
			}
			if c.Name == nil || *c.Name != tt.want {
				t.Errorf("Name = %v, want %q", c.Name, tt.want)
			}
		})
	}
}

func TestParseConfigTrailingCommaInArray(t *testing.T) {
	got := ParseConfig(`{"image": "rust:latest", "capAdd": ["SYS_PTRACE",]}`)
	c, ok := got.(*ImageConfig)
	if !ok {
		t.Fatalf("expected *ImageConfig, got %T", got)
	}
	if !reflect.DeepEqual(c.CapAdd, []string{"SYS_PTRACE"}) {
		t.Errorf("CapAdd = %v", c.CapAdd)
	}
}

func TestParseConfigInvalid(t *testing.T) {
	tests := []struct {
		name  string
		input string
	}{
		{
			name:  "when parse config with empty string then returns nil",
			input: "",
		},
		{
			name:  "when parse config with lone slash outside string then returns nil",
			input: `{"image": "rust:latest"} /`,
		},
		{
			name:  "when parse config with no discriminating field then returns nil",
			input: `{"name": "x"}`,
		},
		{
			name:  "when parse config with invalid json then returns nil",
			input: "{ invalid }",
		},
		{
			name:  "when parse config with unknown user env probe then returns nil",
			input: `{"image": "rust:latest", "userEnvProbe": "badProbe"}`,
		},
		{
			name:  "when parse config with unknown wait for then returns nil",
			input: `{"image": "rust:latest", "waitFor": "badCommand"}`,
		},
		{
			name:  "when parse config with compose missing service then returns nil",
			input: `{"dockerComposeFile": "docker-compose.yml", "workspaceFolder": "/workspace"}`,
		},
		{
			name:  "when parse config with compose missing workspace folder then returns nil",
			input: `{"dockerComposeFile": "docker-compose.yml", "service": "app"}`,
		},
		{
			name:  "when parse config with compose service as number then returns nil",
			input: `{"dockerComposeFile": "docker-compose.yml", "service": 42, "workspaceFolder": "/workspace"}`,
		},
		{
			name:  "when parse config with docker file as number then returns nil",
			input: `{"dockerFile": 42}`,
		},
		{
			name:  "when parse config with build as number then returns nil",
			input: `{"build": 42}`,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := ParseConfig(tt.input); got != nil {
				t.Errorf("ParseConfig(%q) = %#v, want nil", tt.input, got)
			}
		})
	}
}
